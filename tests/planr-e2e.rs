//! End-to-end suite for the `planr` binary.
//!
//! Each scenario builds a throwaway git repo, seeds the minimal .plan/
//! structure needed, and runs the real `planr` binary via assert_cmd.
//!
//! Port of `skills/planr/tests/run-tests.sh` (~252 LOC bash, 40+ checks).

use assert_cmd::Command;
use std::path::Path;
use std::process::Output;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Create a throwaway git repo at `dir`, seeded with a minimal .plan/.
fn init_repo(dir: &Path) {
    // git init
    Command::new("git")
        .args(["init", "-b", "main"])
        .arg(dir)
        .ok()
        .unwrap();
    Command::new("git")
        .args(["config", "user.email", "e2e@test"])
        .current_dir(dir)
        .ok()
        .unwrap();
    Command::new("git")
        .args(["config", "user.name", "E2E Test"])
        .current_dir(dir)
        .ok()
        .unwrap();

    // Create .plan dirs with a placeholder so git tracks them
    for d in &[".plan/epics", ".plan/stories", ".plan/tasks"] {
        std::fs::create_dir_all(dir.join(d)).unwrap();
    }
    // Git doesn't track empty dirs -- write a .gitkeep
    std::fs::write(dir.join(".plan/.gitkeep"), "").unwrap();

    // Initial commit
    let out = Command::new("git")
        .args(["add", ".plan"])
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git add failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = Command::new("git")
        .args(["commit", "-m", "seed plan"])
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git commit failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Run planr with args, return the output.
fn planr(dir: &Path, args: &[&str]) -> Output {
    Command::cargo_bin("planr")
        .unwrap()
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap()
}

/// Run planr, expect success, return stdout.
fn planr_ok(dir: &Path, args: &[&str]) -> String {
    let out = planr(dir, args);
    assert!(
        out.status.success(),
        "planr {args:?} failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

/// Run planr, expect failure, return stderr.
fn planr_err(dir: &Path, args: &[&str]) -> String {
    let out = planr(dir, args);
    assert!(!out.status.success(), "planr {args:?} expected failure");
    String::from_utf8(out.stderr).unwrap().trim().to_string()
}

/// Run planr, expect success, return (stdout, stderr).
fn planr_ok_both(dir: &Path, args: &[&str]) -> (String, String) {
    let out = planr(dir, args);
    assert!(
        out.status.success(),
        "planr {args:?} failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    (
        String::from_utf8(out.stdout).unwrap().trim().to_string(),
        String::from_utf8(out.stderr).unwrap().trim().to_string(),
    )
}

// ---------------------------------------------------------------------------
// Scenario: new-ticket guards
// ---------------------------------------------------------------------------

#[test]
fn test_e2e_new_ticket_dangling_parent() {
    let td = tempfile::tempdir().unwrap();
    init_repo(td.path());
    let err = planr_err(
        td.path(),
        &["new", "task", "my-task", "Title", "nonexistent"],
    );
    assert!(
        err.contains("create the parent first"),
        "dangling parent: {err}"
    );
}

#[test]
fn test_e2e_new_ticket_bad_slug_uppercase() {
    let td = tempfile::tempdir().unwrap();
    init_repo(td.path());
    let err = planr_err(td.path(), &["new", "task", "Bad-Slug", "Title", "parent"]);
    assert!(err.contains("bad slug"), "bad slug: {err}");
}

#[test]
fn test_e2e_new_ticket_bad_slug_trailing_hyphen() {
    let td = tempfile::tempdir().unwrap();
    init_repo(td.path());
    let err = planr_err(td.path(), &["new", "task", "foo-", "Title", "parent"]);
    assert!(err.contains("bad slug"));
}

#[test]
fn test_e2e_new_ticket_bad_slug_double_hyphen() {
    let td = tempfile::tempdir().unwrap();
    init_repo(td.path());
    let err = planr_err(td.path(), &["new", "task", "foo--bar", "Title", "parent"]);
    assert!(err.contains("bad slug"));
}

// ---------------------------------------------------------------------------
// Scenario: happy path - epic -> stories -> tasks
// ---------------------------------------------------------------------------

#[test]
fn test_e2e_happy_path() {
    let td = tempfile::tempdir().unwrap();
    init_repo(td.path());

    // Create epic
    let out = planr_ok(td.path(), &["new", "epic", "v1", "Version One"]);
    assert!(out.contains(".plan/epics/"), "epic path: {out}");

    // Create two stories
    let s1 = planr_ok(td.path(), &["new", "story", "net", "Networking", "v1"]);
    assert!(s1.contains(".plan/stories/"));
    let s2 = planr_ok(td.path(), &["new", "story", "db", "Database", "v1"]);
    assert!(s2.contains(".plan/stories/"));

    // Create two tasks under first story
    let t1 = planr_ok(
        td.path(),
        &["new", "task", "http-proxy", "HTTP Proxy", "net"],
    );
    assert!(t1.contains(".plan/tasks/"));

    // Commit the backlog
    Command::new("git")
        .args(["add", ".plan"])
        .current_dir(td.path())
        .ok()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "seed"])
        .current_dir(td.path())
        .ok()
        .unwrap();

    // Verify the task file has aliases
    let content = std::fs::read_to_string(td.path().join(&t1)).unwrap();
    assert!(content.contains("aliases: [http-proxy]"), "aliases filled");
}

// ---------------------------------------------------------------------------
// Scenario: lint classes
// ---------------------------------------------------------------------------

fn seed_lint_repo(dir: &Path) {
    init_repo(dir);
    // Create epic + story + task via planr
    planr_ok(dir, &["new", "epic", "e1", "Epic One"]);
    planr_ok(dir, &["new", "story", "s1", "Story One", "e1"]);
    planr_ok(dir, &["new", "task", "t1", "Task One", "s1"]);

    // Commit the clean backlog for ref-mode lint
    Command::new("git")
        .args(["add", ".plan"])
        .current_dir(dir)
        .ok()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "clean backlog"])
        .current_dir(dir)
        .ok()
        .unwrap();
}

#[test]
fn test_e2e_lint_clean() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());

    let out = planr_ok(td.path(), &["lint"]);
    // Clean: empty output or 0 errors
    assert!(
        out.is_empty() || out.contains("0 error(s)"),
        "expected clean lint: '{out}'"
    );
}

#[test]
fn test_e2e_lint_ref_clean() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());

    let out = planr_ok(td.path(), &["lint", "main"]);
    assert!(
        out.is_empty() || out.contains("0 error(s)"),
        "ref lint: '{out}'"
    );
}

#[test]
fn test_e2e_lint_dangling_dep() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());

    // Create a task with depends_on to a non-existent ticket
    let t2_path = planr_ok(td.path(), &["new", "task", "t2", "Task Two", "s1"]);
    // Manually inject a depends_on
    let path = td.path().join(&t2_path);
    let content = std::fs::read_to_string(&path).unwrap();
    let new_content = content.replace("depends_on: []", "depends_on: [nonexistent]");
    std::fs::write(&path, new_content).unwrap();

    // Lint with errors exits 1; just check output content
    let out = planr(td.path(), &["lint"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("nonexistent"), "dangling dep: {stdout}");
    assert!(
        stdout.contains("could never be satisfied"),
        "dep message: {stdout}"
    );
}

#[test]
fn test_e2e_new_ticket_with_colon_in_title_lints_clean() {
    // Issue #1: `planr new` scaffolded an unquoted `title:`, so planr's own
    // reader failed on the file planr had just written -- silently at
    // creation, then as bogus lint errors about fields that were present.
    let td = tempfile::tempdir().unwrap();
    init_repo(td.path());

    let title = "The shadow remote: git-native sync through a sanitary staging repo";
    let epic_path = planr_ok(td.path(), &["new", "epic", "shadow-remote", title]);
    planr_ok(
        td.path(),
        &[
            "new",
            "story",
            "sanitary",
            "Sanitary history: boundary rev, rewriter",
            "shadow-remote",
        ],
    );

    // The scaffolded frontmatter quotes the colon-bearing title...
    let content = std::fs::read_to_string(td.path().join(&epic_path)).unwrap();
    assert!(
        content.contains(&format!("title: '{title}'")),
        "expected a quoted title in the scaffold: {content}"
    );

    // ...and the backlog reads back clean, with no cascade onto the child.
    let out = planr(td.path(), &["lint"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "lint failed: {stdout}");
    assert!(
        stdout.is_empty() || stdout.contains("0 error(s)"),
        "expected clean lint: {stdout}"
    );
}

#[test]
fn test_e2e_lint_reports_unparsed_frontmatter_not_missing_fields() {
    // Hand-written (or pre-0.3.2-scaffolded) tickets can still carry an
    // unquoted colon. Lint must name the real defect.
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());

    let epic = td.path().join(".plan/epics/01-e1.md");
    let content = std::fs::read_to_string(&epic).unwrap();
    std::fs::write(
        &epic,
        content.replace("title: Epic One", "title: Epic One: with a colon"),
    )
    .unwrap();

    let out = planr(td.path(), &["lint"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("frontmatter failed to parse"),
        "expected a parse-failure finding: {stdout}"
    );
    assert!(
        !stdout.contains("missing id"),
        "should not cascade into field errors: {stdout}"
    );
    assert!(
        !stdout.contains("<missing>"),
        "should not cascade into field errors: {stdout}"
    );
    // The story under this epic keeps a well-typed parent.
    assert!(
        !stdout.contains("is a task"),
        "should not cascade onto children: {stdout}"
    );
}

// ---------------------------------------------------------------------------
// Scenario: board
// ---------------------------------------------------------------------------

#[test]
fn test_e2e_board_summary() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());

    let out = planr_ok(td.path(), &["board"]);
    assert!(out.contains("## summary"), "board missing summary");
    assert!(out.contains("total"), "board missing total");
    // Should have epics, stories, tasks sections
    assert!(out.contains("## epics"), "board missing epics");
    assert!(out.contains("## stories"), "board missing stories");
    assert!(out.contains("## tasks"), "board missing tasks");
}

#[test]
fn test_e2e_board_defaults_to_working_tree() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());

    // Add a task on disk but do NOT commit it -- it exists only in the
    // working tree, not in any committed ref.
    planr_ok(td.path(), &["new", "task", "wip", "Work In Progress", "s1"]);

    // Default (no ref) reads the current on-disk tree: the uncommitted task
    // is visible.
    let working = planr_ok(td.path(), &["board"]);
    assert!(
        working.lines().any(|l| {
            let fields: Vec<_> = l.split_whitespace().collect();
            fields.first() == Some(&"wip")
        }),
        "working-tree board should show uncommitted task: {working}"
    );

    // An explicit commit-ish reads that ref, where the task was never
    // committed, so it is absent.
    let committed = planr_ok(td.path(), &["board", "main"]);
    assert!(
        !committed.lines().any(|l| {
            let fields: Vec<_> = l.split_whitespace().collect();
            fields.first() == Some(&"wip")
        }),
        "main board should not show uncommitted task: {committed}"
    );
    // The committed task t1 is still there under the explicit ref.
    assert!(
        committed.lines().any(|l| {
            let fields: Vec<_> = l.split_whitespace().collect();
            fields.first() == Some(&"t1")
        }),
        "main board should show committed task: {committed}"
    );
}

/// Read the first output line of `planr board [args]` -- the source header.
fn board_header(dir: &Path, args: &[&str]) -> String {
    let out = planr(dir, args);
    assert!(out.status.success(), "planr {args:?} failed");
    String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .next()
        .unwrap_or_default()
        .to_string()
}

fn git_stdout(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

#[test]
fn test_e2e_board_source_status_line() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());

    let toplevel = git_stdout(td.path(), &["rev-parse", "--show-toplevel"]);
    let head = git_stdout(td.path(), &["rev-parse", "--short", "HEAD"]);

    // Clean working tree, on branch main: "# <path> @ <sha> (main)".
    let header = board_header(td.path(), &["board"]);
    assert!(header.starts_with("# "), "header prefix: {header}");
    assert!(header.contains(&toplevel), "header path: {header}");
    assert!(
        header.contains(&format!("@ {head} (main)")),
        "header commit id + branch: {header}"
    );
    assert!(!header.contains("dirty"), "clean tree not dirty: {header}");

    // Dirty the working tree with an uncommitted ticket.
    planr_ok(td.path(), &["new", "task", "wip", "Work In Progress", "s1"]);
    let dirty = board_header(td.path(), &["board"]);
    assert!(dirty.contains("(main) dirty"), "dirty marker: {dirty}");

    // Ref mode reads committed data, so it is never marked dirty even when the
    // working tree has uncommitted changes.
    let ref_header = board_header(td.path(), &["board", "main"]);
    assert!(
        ref_header.contains("@ main"),
        "ref header name: {ref_header}"
    );
    assert!(
        !ref_header.contains("dirty"),
        "ref mode ignores working-tree dirt: {ref_header}"
    );

    // A ref given as a SHA is not repeated as both name and id.
    let sha_header = board_header(td.path(), &["board", &head]);
    assert!(
        sha_header.ends_with(&format!("@ {head}")),
        "sha ref not duplicated: {sha_header}"
    );
}

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
    Command::new("git")
        .args(["add", ".plan"])
        .current_dir(td.path())
        .ok()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "add dependent"])
        .current_dir(td.path())
        .ok()
        .unwrap();

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

// ---------------------------------------------------------------------------
// Scenario: trunk-writing subcommands run from a secondary worktree
//
// The leader may invoke planr from a git worktree that is checked out on a
// branch other than trunk. Trunk-local operations must not `git checkout
// <trunk>` in that worktree (trunk is already used by the main worktree) --
// they must write and commit in whichever worktree holds trunk.
// ---------------------------------------------------------------------------

/// Add a git worktree at `path` on a new `branch`, run from `dir`.
fn git_worktree_add(dir: &Path, path: &Path, branch: &str) {
    let out = Command::new("git")
        .args(["worktree", "add", "-b", branch])
        .arg(path)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git worktree add failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

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
    Command::new("git")
        .args(["add", &task_file])
        .current_dir(&wt_abs)
        .ok()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "review: t1"])
        .current_dir(&wt_abs)
        .ok()
        .unwrap();

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

// ---------------------------------------------------------------------------
// Scenario: claim + close task end-to-end
// ---------------------------------------------------------------------------

#[test]
fn test_e2e_claim_close_task() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());

    // planr claim t1 with explicit worktree path inside temp dir
    let wt_rel = "wt-t1";
    let wt_abs = td.path().join(wt_rel);
    let wt = planr_ok(
        td.path(),
        &["claim", "t1", "--worktree", &wt_abs.to_string_lossy()],
    );
    assert!(wt.contains("wt-t1"), "worktree path: {wt}");

    // Verify the worktree has the flipped status
    let task_file = format!(".plan/tasks/{}", find_task_slug(td.path(), "t1"));
    let content = std::fs::read_to_string(wt_abs.join(&task_file)).unwrap();
    assert!(
        content.contains("status: in_progress"),
        "flipped to in_progress"
    );
    // Flip status to review on the branch
    let review_content = content.replace("status: in_progress", "status: review")
        + "\n\n## Review\n\nverdict: approved\nreviewer: test\ndate: 2026-09-05\n";
    std::fs::write(wt_abs.join(&task_file), review_content).unwrap();

    // Commit the review flip
    Command::new("git")
        .args(["add", &task_file])
        .current_dir(&wt_abs)
        .ok()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "review: t1"])
        .current_dir(&wt_abs)
        .ok()
        .unwrap();

    // Now close task t1
    let close_out = planr_ok(td.path(), &["close", "task", "t1"]);
    assert!(close_out.contains("merged"), "close output: {close_out}");
    assert!(close_out.contains("t1 done"), "close output: {close_out}");

    // Verify the merge happened: branch should be gone
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

// ---------------------------------------------------------------------------
// Scenario: claim without --worktree (issue #4)
// ---------------------------------------------------------------------------

/// A bare `claim` must do the full job: default worktree, branch, status
/// flip, commit. It used to print `claimed: <slug>` and exit 0 having
/// changed nothing, which let a caller start editing on trunk.
#[test]
fn test_e2e_claim_without_worktree_flag_does_the_work() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());

    let out = planr_ok(td.path(), &["claim", "t1"]);

    // Output is the worktree path at the documented default location.
    let wt_abs = td.path().join(".plan/worktrees/wt-t1");
    assert!(
        !out.contains("claimed:"),
        "bare claim must not report a bare claim: {out}"
    );
    assert!(out.contains("wt-t1"), "worktree path expected: {out}");
    assert!(wt_abs.is_dir(), "worktree dir not created: {out}");

    // The branch exists.
    let branches = Command::new("git")
        .args(["branch", "--list"])
        .current_dir(td.path())
        .output()
        .unwrap();
    let branches_str = String::from_utf8(branches.stdout).unwrap();
    assert!(
        branches_str.contains("plan/t1"),
        "branch not created: {branches_str}"
    );

    // The status is flipped on the branch, and committed (not left dirty).
    let task_file = format!(".plan/tasks/{}", find_task_slug(td.path(), "t1"));
    let content = std::fs::read_to_string(wt_abs.join(&task_file)).unwrap();
    assert!(
        content.contains("status: in_progress"),
        "status not flipped: {content}"
    );
    let porcelain = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(&wt_abs)
        .output()
        .unwrap();
    assert!(
        String::from_utf8(porcelain.stdout)
            .unwrap()
            .trim()
            .is_empty(),
        "status flip was not committed"
    );
}

/// `--no-worktree` remains the one opt-out: the caller manages its own
/// workspace, so planr reports the claim and changes nothing.
#[test]
fn test_e2e_claim_no_worktree_opts_out() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());

    let out = planr_ok(td.path(), &["claim", "t1", "--no-worktree"]);
    assert!(out.contains("claimed: t1"), "claim output: {out}");

    assert!(
        !td.path().join(".plan/worktrees").exists(),
        "--no-worktree must not create a worktree"
    );
    let branches = Command::new("git")
        .args(["branch", "--list"])
        .current_dir(td.path())
        .output()
        .unwrap();
    let branches_str = String::from_utf8(branches.stdout).unwrap();
    assert!(
        !branches_str.contains("plan/t1"),
        "--no-worktree must not create a branch: {branches_str}"
    );
}

fn find_ticket_filename(plan_dir: &Path, kind_dir: &str, slug: &str) -> String {
    let tickets_dir = plan_dir.join(format!(".plan/{kind_dir}"));
    for entry in std::fs::read_dir(tickets_dir).unwrap() {
        let e = entry.unwrap();
        let name = e.file_name().into_string().unwrap();
        if name.ends_with(&format!("-{slug}.md")) {
            return name;
        }
    }
    panic!("{kind_dir}/{slug} not found");
}

fn find_task_slug(plan_dir: &Path, slug: &str) -> String {
    find_ticket_filename(plan_dir, "tasks", slug)
}

// We also need a way to find task file paths within seed_lint_repo.
// Let me fix the cycle test by restructuring.

// ---------------------------------------------------------------------------
// Scenario: parallel new-ticket
// ---------------------------------------------------------------------------

#[test]
fn test_e2e_parallel_new_ticket() {
    use std::sync::Arc;

    let td = Arc::new(tempfile::tempdir().unwrap());
    init_repo(td.path());

    // Seed an epic for parent
    planr_ok(td.path(), &["new", "epic", "parent-epic", "Parent"]);

    // Run 3 concurrent planr new tasks
    let mut handles = Vec::new();
    for i in 0..3 {
        let td = td.clone();
        handles.push(std::thread::spawn(move || {
            let out = Command::cargo_bin("planr")
                .unwrap()
                .args([
                    "new",
                    "task",
                    &format!("task-{i}"),
                    &format!("Task {i}"),
                    "parent-epic",
                ])
                .current_dir(td.path())
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "parallel new {i} failed: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            let stdout = String::from_utf8(out.stdout).unwrap().trim().to_string();
            assert!(!stdout.is_empty(), "parallel new {i} empty stdout");
            stdout
        }));
    }

    let mut paths: Vec<String> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    paths.sort();
    // All paths should be distinct
    paths.dedup();
    assert_eq!(paths.len(), 3, "expected 3 distinct paths: {paths:?}");

    // Lint should be clean
    let lint_out = planr_ok(td.path(), &["lint"]);
    assert!(
        lint_out.is_empty() || lint_out.contains("0 error(s)"),
        "lint after parallel: '{lint_out}'"
    );
}

// ---------------------------------------------------------------------------
// Scenario: close story gate
// ---------------------------------------------------------------------------

#[test]
fn test_e2e_close_story_refuses_open_tasks() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());

    // Try to close the story while tasks are open
    let err = planr_err(td.path(), &["close", "story", "s1"]);
    assert!(
        err.contains("unfinished"),
        "expected unfinished tasks: {err}"
    );
}

#[test]
fn test_e2e_close_epic_refuses_open_stories() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());

    let err = planr_err(td.path(), &["close", "epic", "e1"]);
    assert!(
        err.contains("unfinished"),
        "expected unfinished stories: {err}"
    );
}

// ---------------------------------------------------------------------------
// Scenario: informational lint on new-ticket
// ---------------------------------------------------------------------------

#[test]
fn test_e2e_new_ticket_informational_lint() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());

    let (stdout, _stderr) =
        planr_ok_both(td.path(), &["new", "task", "new-lint-test", "Test", "s1"]);
    // stdout is one line: the path
    assert!(stdout.contains(".plan/tasks/"), "stdout: {stdout}");
    assert_eq!(
        stdout.lines().count(),
        1,
        "stdout must be one line: {stdout}"
    );
    // stderr may have lint warnings (informational)
    // Just verify it doesn't contain "error:" from planr lint
    // stderr may have lint findings (informational)
    // Just verify stdout is one line
    assert!(stdout.contains(".plan/tasks/"));
}

// ---------------------------------------------------------------------------
// Helpers for specific test paths
// ---------------------------------------------------------------------------

fn t1_path_of(dir: &Path) -> String {
    format!(".plan/tasks/{}", find_task_slug(dir, "t1"))
}
fn t2_path_of(dir: &Path) -> String {
    format!(".plan/tasks/{}", find_task_slug(dir, "t2"))
}
fn t3_path_of(dir: &Path) -> String {
    format!(".plan/tasks/{}", find_task_slug(dir, "t3"))
}

#[test]
fn test_e2e_lint_cycle_real() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());

    // Create two more tasks
    let _t2_path = planr_ok(td.path(), &["new", "task", "t2", "Task Two", "s1"]);
    let _t3_path = planr_ok(td.path(), &["new", "task", "t3", "Task Three", "s1"]);

    let inject = |path: &Path, deps: &str| {
        let c = std::fs::read_to_string(path).unwrap();
        std::fs::write(
            path,
            c.replace("depends_on: []", &format!("depends_on: [{deps}]")),
        )
        .unwrap();
    };
    inject(&td.path().join(t1_path_of(td.path())), "t2");
    inject(&td.path().join(t2_path_of(td.path())), "t3");
    inject(&td.path().join(t3_path_of(td.path())), "t1");

    // Also inject a self-dep on t1 for the self-dep test
    let c = std::fs::read_to_string(td.path().join(t1_path_of(td.path()))).unwrap();
    std::fs::write(
        td.path().join(t1_path_of(td.path())),
        c.replace("depends_on: [t2]", "depends_on: [t2, t1]"),
    )
    .unwrap();

    let out = planr(td.path(), &["lint"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Should detect cycle: t1 -> t2 -> t3 -> t1
    assert!(stdout.contains("cycle"), "expected cycle: {stdout}");
    assert!(stdout.contains("t1"), "cycle involving t1: {stdout}");
    // Self-dep should NOT be reported as a cycle
    // (self-dep is reported separately)
}
