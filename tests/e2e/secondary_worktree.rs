//! Trunk-writing subcommands run from a secondary worktree.
//!
//! The leader may invoke planr from a git worktree that is checked out on a
//! branch other than trunk. Trunk-local operations must not `git checkout
//! <trunk>` in that worktree (trunk is already used by the main worktree) --
//! they must write and commit in whichever worktree holds trunk.

use crate::common::*;
use assert_cmd::Command;

#[test]
fn test_e2e_abandon_from_secondary_worktree() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());

    // Leader operates from a worktree parked on an unrelated branch.
    let lead = td.path().join("wt-lead");
    git_worktree_add(td.path(), &lead, "lead/x");

    let out = planr_ok(&lead, &["abandon", "epic", "e1", "OBE — pivoted"]);
    assert!(out.contains("abandoned epic e1"), "abandon output: {out}");

    // The commit must land on trunk: the main worktree (still on main)
    // reflects the abandoned status.
    let epic_path = format!(
        ".plan/epics/{}",
        find_ticket_filename(td.path(), "epics", "e1")
    );
    let content = std::fs::read_to_string(td.path().join(&epic_path)).unwrap();
    assert!(content.contains("status: abandoned"), "trunk not updated");
    assert!(content.contains("OBE — pivoted"));

    // The operating worktree must remain on its own branch (never switched).
    let head = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(&lead)
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&head.stdout).trim(), "lead/x");
}

#[test]
fn test_e2e_close_lifecycle_from_secondary_worktree() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());

    // Claim the task with its own worktree, self-validate + approve on branch.
    let wt_abs = td.path().join("wt-t1");
    planr_ok(
        td.path(),
        &["claim", "t1", "--worktree", &wt_abs.to_string_lossy()],
    );
    let task_file = format!(".plan/tasks/{}", find_task_slug(td.path(), "t1"));
    let content = std::fs::read_to_string(wt_abs.join(&task_file)).unwrap();
    let review_content = content.replace("status: in_progress", "status: review")
        + "\n\n## Review\n\nverdict: approved\nreviewer: test\ndate: 2026-09-05\n";
    std::fs::write(wt_abs.join(&task_file), review_content).unwrap();
    git_must(&wt_abs, &["add", &task_file]);
    git_must(&wt_abs, &["commit", "-m", "review: t1"]);

    // Leader drives the whole close chain from a separate parked worktree.
    let lead = td.path().join("wt-lead");
    git_worktree_add(td.path(), &lead, "lead/x");

    let close_task = planr_ok(&lead, &["close", "task", "t1"]);
    assert!(close_task.contains("merged"), "close task: {close_task}");
    planr_ok(&lead, &["close", "story", "s1"]);
    planr_ok(&lead, &["close", "epic", "e1"]);

    // All three land as done on trunk (visible in the main worktree).
    for (kind, slug) in [("epics", "e1"), ("stories", "s1"), ("tasks", "t1")] {
        let p = format!(
            ".plan/{kind}/{}",
            find_ticket_filename(td.path(), kind, slug)
        );
        let c = std::fs::read_to_string(td.path().join(&p)).unwrap();
        assert!(c.contains("status: done"), "{slug} not done on trunk: {p}");
    }

    // The merged task branch is cleaned up despite the merge landing in a
    // worktree other than the one planr ran from.
    let branches = Command::new("git")
        .args(["branch", "--list"])
        .current_dir(td.path())
        .output()
        .unwrap();
    let branches_str = String::from_utf8(branches.stdout).unwrap();
    assert!(
        !branches_str.contains("plan/t1"),
        "branch should be deleted: {branches_str}"
    );
}
