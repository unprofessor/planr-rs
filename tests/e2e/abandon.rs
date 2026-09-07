//! Scenarios for `planr abandon`: the reason and branch guards, what
//! an abandoned ticket does to its dependents, and how it reads on the
//! board.

use crate::common::*;
use assert_cmd::Command;

// ---------------------------------------------------------------------------
// Scenario: abandon without review
// ---------------------------------------------------------------------------

#[test]
fn test_e2e_abandon_task_skips_review_and_blocks_dependents() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());

    // Add a dependent task and commit the dependency before abandoning t1.
    let dependent_path = planr_ok(td.path(), &["new", "task", "dependent", "Dependent", "s1"]);
    let dependent = std::fs::read_to_string(td.path().join(&dependent_path)).unwrap();
    std::fs::write(
        td.path().join(&dependent_path),
        dependent.replace("depends_on: []", "depends_on: [t1]"),
    )
    .unwrap();
    git_must(td.path(), &["add", ".plan"]);
    git_must(td.path(), &["commit", "-m", "add dependent"]);

    let msg = "OBE — no longer needed";
    let out = planr_ok(td.path(), &["abandon", "task", "t1", msg]);
    assert!(out.contains("abandoned task t1"), "abandon output: {out}");

    let t1_path = format!(".plan/tasks/{}", find_task_slug(td.path(), "t1"));
    let content = std::fs::read_to_string(td.path().join(&t1_path)).unwrap();
    assert!(content.contains("status: abandoned"));
    assert!(content.contains("## Reason Abandoned"));
    assert!(content.contains("OBE — no longer needed"));
    assert!(!content.contains("verdict: approved"));
    // Frontmatter should NOT contain a reason field
    assert!(!content.contains("\nreason:"));

    // Abandoned is deliberately not a satisfied dependency.
    let err = planr_err(td.path(), &["claim", "dependent"]);
    assert!(err.contains("t1(abandoned)"), "dependency blocker: {err}");

    // The normal close path still requires a branch and review.
    let err = planr_err(td.path(), &["close", "task", "t1"]);
    assert!(err.contains("no such branch"), "close gate: {err}");

    // A second abandon cannot overwrite; message is ignored.
    let err = planr_err(td.path(), &["abandon", "task", "t1", "wont-do even now"]);
    assert!(err.contains("already abandoned"), "repeat abandon: {err}");
    let content = std::fs::read_to_string(td.path().join(&t1_path)).unwrap();
    assert!(!content.contains("wont-do"));
}

#[test]
fn test_e2e_abandon_story_and_epic_are_visible_on_board() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());

    planr_ok(
        td.path(),
        &["abandon", "story", "s1", "wont-do — spec changed"],
    );
    planr_ok(td.path(), &["abandon", "epic", "e1", "OBE"]);

    let story_path = format!(
        ".plan/stories/{}",
        find_ticket_filename(td.path(), "stories", "s1")
    );
    let epic_path = format!(
        ".plan/epics/{}",
        find_ticket_filename(td.path(), "epics", "e1")
    );
    let story = std::fs::read_to_string(td.path().join(story_path)).unwrap();
    let epic = std::fs::read_to_string(td.path().join(epic_path)).unwrap();
    assert!(story.contains("status: abandoned"));
    assert!(story.contains("## Reason Abandoned"));
    assert!(story.contains("wont-do — spec changed"));
    assert!(epic.contains("status: abandoned"));
    assert!(epic.contains("## Reason Abandoned"));
    assert!(epic.contains("OBE"));

    let board = planr_ok(td.path(), &["board"]);
    assert!(
        board.lines().any(|line| {
            let fields: Vec<_> = line.split_whitespace().collect();
            fields.first() == Some(&"s1") && fields.get(1) == Some(&"abandoned")
        }),
        "story board row: {board}"
    );
    assert!(
        board.lines().any(|line| {
            let fields: Vec<_> = line.split_whitespace().collect();
            fields.first() == Some(&"e1") && fields.get(1) == Some(&"abandoned")
        }),
        "epic board row: {board}"
    );
    assert!(
        board.lines().any(|line| {
            line.starts_with("abandoned") && line.split_whitespace().last() == Some("2")
        }),
        "board summary: {board}"
    );
    assert!(planr_ok(td.path(), &["lint"]).is_empty());
}

#[test]
fn test_e2e_abandon_rejects_invalid_reason_and_active_branch() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());

    let wt_abs = td.path().join("wt-t1");
    planr_ok(
        td.path(),
        &["claim", "t1", "--worktree", &wt_abs.to_string_lossy()],
    );
    let err = planr_err(td.path(), &["abandon", "task", "t1", "some reason"]);
    assert!(
        err.contains("active branch plan/t1"),
        "active branch: {err}"
    );
    assert!(wt_abs.exists(), "active worktree must remain");

    let branches = Command::new("git")
        .args(["branch", "--list"])
        .current_dir(td.path())
        .output()
        .unwrap();
    assert!(
        String::from_utf8_lossy(&branches.stdout).contains("plan/t1"),
        "active branch must remain: {}",
        String::from_utf8_lossy(&branches.stdout)
    );
}
