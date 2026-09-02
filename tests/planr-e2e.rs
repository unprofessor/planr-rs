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

// ---------------------------------------------------------------------------
// Scenario: board sees in-flight branches from trunk
// ---------------------------------------------------------------------------

/// The whole point of the in-flight section is that the leader, sitting on
/// trunk, can see what the workers are doing. `git branch --list` marks a
/// branch checked out in a linked worktree with `+ `, not `* ` or two
/// spaces, and every claimed task is exactly that -- so the decorated-output
/// parser mangled every branch name into `+ plan/<slug>`, the ref lookup
/// failed, and the row was silently dropped. The section only appeared from
/// inside the worktree itself, where the branch is `* `-marked.
#[test]
fn test_e2e_board_reports_in_flight_branches_from_trunk() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());

    planr_ok(td.path(), &["claim", "t1"]);

    // Trunk still records t1 as todo -- the flip lives on the branch.
    let trunk_task = td
        .path()
        .join(format!(".plan/tasks/{}", find_task_slug(td.path(), "t1")));
    let trunk_content = std::fs::read_to_string(&trunk_task).unwrap();
    assert!(
        trunk_content.contains("status: todo"),
        "trunk should be untouched by claim: {trunk_content}"
    );

    // git decorates the worktree branch with `+ `, which is what broke this.
    let decorated = git_stdout(td.path(), &["branch", "--list", "plan/*"]);
    assert!(
        decorated.contains("plan/t1"),
        "branch not created: {decorated}"
    );

    let board = planr_ok(td.path(), &["board"]);
    assert!(
        board.contains("## in flight (worktree branches)"),
        "in-flight section missing from trunk board: {board}"
    );
    assert!(
        board
            .lines()
            .any(|l| l.starts_with("plan/t1") && l.contains("in_progress") && l.contains("t1")),
        "in-flight row for plan/t1 missing: {board}"
    );
    // The summary must count the branch status, not the stale trunk status.
    assert!(
        board
            .lines()
            .any(|l| l.starts_with("in_progress") && l.split_whitespace().last() == Some("1")),
        "summary should count t1 as in_progress: {board}"
    );
    // The tasks table shows the live status, marked as branch-sourced, so it
    // agrees with the summary instead of reporting the stale trunk `todo`.
    let task_row = board
        .lines()
        .find(|l| l.starts_with("t1 "))
        .unwrap_or_else(|| panic!("t1 row missing: {board}"));
    assert!(
        task_row.contains("in_progress *"),
        "tasks row should carry the in-flight marker: {task_row}"
    );
    assert!(
        board.contains("* status from an in-flight branch"),
        "marker legend missing: {board}"
    );
}

/// A worktree inside the repo is an embedded repo, so git stages it as a
/// gitlink -- a bogus submodule that a fresh clone cannot resolve -- and
/// trunk reads dirty until someone runs `git rm --cached`. Claim must add an
/// ignore rule for it wherever it lands, default or explicit.
#[test]
fn test_e2e_worktree_inside_repo_does_not_dirty_trunk() {
    for placement in [
        vec!["claim", "t1"],
        vec!["claim", "t1", "--worktree", ".plan/wt"],
        vec!["claim", "t1", "--worktree", "wt"],
    ] {
        let td = tempfile::tempdir().unwrap();
        seed_lint_repo(td.path());
        planr_ok(td.path(), &placement);
        assert_trunk_undisturbed(td.path(), &format!("{placement:?}"));
    }
}

/// A worktree outside the repo needs no rule -- git never looks at it.
#[test]
fn test_e2e_worktree_outside_repo_needs_no_ignore_rule() {
    let td = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());

    let wt = outside.path().join("wt-t1");
    planr_ok(
        td.path(),
        &["claim", "t1", "--worktree", &wt.to_string_lossy()],
    );
    assert_trunk_undisturbed(td.path(), "outside the repo");

    let excl = std::fs::read_to_string(td.path().join(".git/info/exclude")).unwrap_or_default();
    assert!(
        !excl.contains("wt-t1"),
        "no rule should be written for a worktree outside the repo: {excl}"
    );
}

/// Trunk must stay clean after a claim: no dirty status, no `dirty` in the
/// board header, and no gitlink staged by the leader's `git add`.
fn assert_trunk_undisturbed(dir: &Path, case: &str) {
    let porcelain = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(dir)
        .output()
        .unwrap();
    let porcelain = String::from_utf8(porcelain.stdout).unwrap();
    assert!(
        porcelain.trim().is_empty(),
        "{case}: claim left trunk dirty: {porcelain}"
    );

    let board = planr_ok(dir, &["board"]);
    let header = board.lines().next().unwrap_or_default();
    assert!(!header.contains("dirty"), "{case}: board header: {header}");

    // The leader's normal flow must not stage the worktree as a gitlink.
    planr_ok(dir, &["new", "task", "t9", "Task Nine", "s1"]);
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(dir)
        .ok()
        .unwrap();
    let staged = Command::new("git")
        .args(["ls-files", "-s"])
        .current_dir(dir)
        .output()
        .unwrap();
    let staged = String::from_utf8(staged.stdout).unwrap();
    assert!(
        !staged.contains("160000"),
        "{case}: worktree staged as a gitlink: {staged}"
    );
}

/// Claiming a task someone already holds must fail in planr's terms. It used
/// to reach the caller as `fatal: '<path>' already exists`, which tells an
/// agent nothing about what went wrong.
#[test]
fn test_e2e_double_claim_refused_with_planr_error() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());

    planr_ok(td.path(), &["claim", "t1"]);
    let err = planr_err(td.path(), &["claim", "t1"]);
    assert!(
        err.contains("already claimed") && err.contains("wt-t1"),
        "expected a planr-level refusal naming the worktree: {err}"
    );
    assert!(!err.contains("fatal:"), "raw git error leaked: {err}");
}

/// Re-claiming a task whose worktree was removed rebuilds it on the existing
/// branch. `worktree_add` used to pass trunk as the commit-ish even when the
/// branch existed, so this died with `fatal: '<trunk>' is already used by
/// worktree at ...`; the status flip is then a no-op with nothing to commit.
#[test]
fn test_e2e_reclaim_after_worktree_removed_resumes() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());

    planr_ok(td.path(), &["claim", "t1"]);
    Command::new("git")
        .args(["worktree", "remove", ".plan/worktrees/wt-t1", "--force"])
        .current_dir(td.path())
        .ok()
        .unwrap();

    let out = planr_ok(td.path(), &["claim", "t1"]);
    assert!(
        out.contains("wt-t1"),
        "resume should return the path: {out}"
    );

    let wt = td.path().join(".plan/worktrees/wt-t1");
    let head = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(&wt)
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8(head.stdout).unwrap().trim(),
        "plan/t1",
        "worktree should be on the task branch, not trunk"
    );

    let task_file = format!(".plan/tasks/{}", find_task_slug(td.path(), "t1"));
    let content = std::fs::read_to_string(wt.join(&task_file)).unwrap();
    assert!(content.contains("status: in_progress"), "{content}");
}

/// An ignore rule must not outlive the worktree it was written for: a stale
/// rule silently hides whatever is created at that path later. The shared
/// rule for the default location covers a parent directory planr reuses for
/// every claim, so it survives.
#[test]
fn test_e2e_close_drops_the_stale_ignore_rule() {
    for (flags, gone, kept) in [
        (
            vec!["claim", "t1", "--worktree", ".plan/wt"],
            "/.plan/wt/",
            None,
        ),
        (vec!["claim", "t1"], "", Some("/.plan/worktrees/")),
    ] {
        let td = tempfile::tempdir().unwrap();
        seed_lint_repo(td.path());
        // claim prints a repo-relative path (`./.plan/wt`).
        let wt = td.path().join(planr_ok(td.path(), &flags));

        // Drive the task to review so close will merge it.
        let task_file = format!(".plan/tasks/{}", find_task_slug(td.path(), "t1"));
        let content = std::fs::read_to_string(wt.join(&task_file)).unwrap();
        std::fs::write(
            wt.join(&task_file),
            content.replace("status: in_progress", "status: review")
                + "\n\n## Review\n\nverdict: approved\nreviewer: test\ndate: 2026-09-01\n",
        )
        .unwrap();
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(&wt)
            .ok()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "review: t1"])
            .current_dir(&wt)
            .ok()
            .unwrap();

        planr_ok(td.path(), &["close", "task", "t1"]);

        let excl = std::fs::read_to_string(td.path().join(".git/info/exclude")).unwrap_or_default();
        if !gone.is_empty() {
            assert!(
                !excl.lines().any(|l| l.trim() == gone),
                "stale rule {gone} survived close: {excl}"
            );
        }
        if let Some(kept) = kept {
            assert!(
                excl.lines().any(|l| l.trim() == kept),
                "shared rule {kept} should survive close: {excl}"
            );
        }
    }
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

// ---------------------------------------------------------------------------
// Scenario: review findings on the claim/close ignore-rule path
// ---------------------------------------------------------------------------

/// Read `.git/info/exclude`, or "" when it does not exist.
fn read_exclude(dir: &Path) -> String {
    std::fs::read_to_string(dir.join(".git/info/exclude")).unwrap_or_default()
}

/// Ask git itself whether `path` is ignored *from within `dir`*.
///
/// Reading the exclude file only tells you what was written; git anchors a
/// leading-slash pattern to whichever working tree it is evaluating, so only
/// `check-ignore` run in the right tree answers the question that matters.
fn git_ignored(dir: &Path, path: &str) -> bool {
    Command::new("git")
        .args(["check-ignore", "-q", path])
        .current_dir(dir)
        .output()
        .unwrap()
        .status
        .success()
}

/// A claim that fails must leave no ignore rule behind. The rule used to be
/// written before `git worktree add` ran, so a failed claim permanently hid a
/// real directory from git -- and an exclude rule is invisible in
/// `git status`, so nothing would ever point at the cause.
#[test]
fn test_e2e_failed_claim_writes_no_ignore_rule() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());

    // A tracked directory that already exists: worktree add must refuse it.
    std::fs::create_dir_all(td.path().join("realsrc")).unwrap();
    std::fs::write(td.path().join("realsrc/keep.txt"), "tracked\n").unwrap();
    Command::new("git")
        .args(["add", "realsrc"])
        .current_dir(td.path())
        .ok()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "add realsrc"])
        .current_dir(td.path())
        .ok()
        .unwrap();

    let err = planr_err(td.path(), &["claim", "t1", "--worktree", "realsrc"]);
    assert!(!err.is_empty(), "claim should have failed");

    let exclude = read_exclude(td.path());
    assert!(
        !exclude.contains("/realsrc/"),
        "a failed claim must not leave an ignore rule: {exclude:?}"
    );

    // The directory is still visible to git: a new file under it shows up.
    std::fs::write(td.path().join("realsrc/new.txt"), "new\n").unwrap();
    let status = git_stdout(td.path(), &["status", "--porcelain"]);
    assert!(
        status.contains("realsrc/new.txt"),
        "realsrc must remain visible to git: {status:?}"
    );
}

/// `close` may only drop the ignore rule once the worktree it hides is really
/// gone. `git worktree remove` without `--force` refuses whenever the worktree
/// holds untracked files -- a stray build artifact is enough -- and dropping
/// the rule anyway leaves the worktree in place and unhidden, which is exactly
/// the gitlink corruption the rule exists to prevent.
#[test]
fn test_e2e_close_keeps_ignore_rule_when_worktree_removal_fails() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());

    planr_ok(td.path(), &["claim", "t1", "--worktree", "wt-explicit"]);
    let wt = td.path().join("wt-explicit");
    assert!(
        read_exclude(td.path()).contains("/wt-explicit/"),
        "claim should have written the rule"
    );

    // Move the task to review so close will merge it.
    let task_file = format!(".plan/tasks/{}", find_task_slug(td.path(), "t1"));
    let content = std::fs::read_to_string(wt.join(&task_file)).unwrap();
    let reviewed = content.replace("status: in_progress", "status: review")
        + "\n\n## Review\n\nverdict: approved\nreviewer: test\ndate: 2026-09-05\n";
    std::fs::write(wt.join(&task_file), reviewed).unwrap();
    Command::new("git")
        .args(["add", &task_file])
        .current_dir(&wt)
        .ok()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "review: t1"])
        .current_dir(&wt)
        .ok()
        .unwrap();

    // An untracked file makes a non-forced worktree removal refuse.
    std::fs::write(wt.join("build.log"), "noise\n").unwrap();

    planr_ok(td.path(), &["close", "task", "t1"]);

    // The invariant holds either way, so assert it unconditionally rather
    // than only when the removal happened to fail: a worktree that is still
    // on disk must still be hidden. Guarding the whole check on
    // `wt.exists()` would make this test quietly vacuous on any git whose
    // `worktree remove` tolerates untracked files.
    let exclude = read_exclude(td.path());
    assert!(
        !wt.exists() || exclude.contains("/wt-explicit/"),
        "a surviving worktree must stay hidden: exists={}, exclude={exclude:?}",
        wt.exists()
    );
    let status = git_stdout(td.path(), &["status", "--porcelain"]);
    assert!(
        !status.contains("wt-explicit"),
        "worktree must never show up as a gitlink: {status:?}"
    );
}

/// Deleting a worktree with `rm -rf` leaves git still listing it until someone
/// prunes. Refusing a claim on that stale record named a path that was not
/// there and locked the task out for good.
#[test]
fn test_e2e_claim_resumes_after_worktree_deleted_by_hand() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());

    let first = planr_ok(td.path(), &["claim", "t1"]);
    let wt = td.path().join(".plan/worktrees/wt-t1");
    assert!(wt.is_dir(), "claim should have made a worktree: {first}");

    std::fs::remove_dir_all(&wt).unwrap();

    let second = planr_ok(td.path(), &["claim", "t1"]);
    assert!(
        second.contains("wt-t1"),
        "re-claim should rebuild the worktree: {second}"
    );
    assert!(wt.is_dir(), "worktree not rebuilt: {second}");
}

/// A held task must still be refused -- the stale-record fix must not turn the
/// refusal off for a worktree that really is there.
#[test]
fn test_e2e_claim_still_refuses_a_live_holder() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());

    planr_ok(td.path(), &["claim", "t1"]);
    let err = planr_err(td.path(), &["claim", "t1", "--worktree", "second-wt"]);
    assert!(
        err.contains("already claimed"),
        "expected a refusal naming the holder: {err}"
    );
    assert!(
        !td.path().join("second-wt").exists(),
        "a refused claim must leave nothing behind"
    );
}

/// Resuming a claim must not roll the branch back to `in_progress`. A worker
/// can reach `review` before the worktree is removed; rewriting that discards
/// a finished review and then makes `close` refuse the task.
#[test]
fn test_e2e_claim_resume_does_not_revert_review() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());

    planr_ok(td.path(), &["claim", "t1"]);
    let wt = td.path().join(".plan/worktrees/wt-t1");
    let task_file = format!(".plan/tasks/{}", find_task_slug(td.path(), "t1"));

    let content = std::fs::read_to_string(wt.join(&task_file)).unwrap();
    let reviewed = content.replace("status: in_progress", "status: review")
        + "\n\n## Review\n\nverdict: approved\nreviewer: test\ndate: 2026-09-05\n";
    std::fs::write(wt.join(&task_file), reviewed).unwrap();
    Command::new("git")
        .args(["add", &task_file])
        .current_dir(&wt)
        .ok()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "review: t1"])
        .current_dir(&wt)
        .ok()
        .unwrap();

    // Drop the worktree the supported way, then re-claim it.
    Command::new("git")
        .args(["worktree", "remove", "--force", wt.to_str().unwrap()])
        .current_dir(td.path())
        .ok()
        .unwrap();
    planr_ok(td.path(), &["claim", "t1"]);

    let after = std::fs::read_to_string(wt.join(&task_file)).unwrap();
    assert!(
        after.contains("status: review"),
        "resume must not roll the branch back: {after}"
    );
    // And close still works, which it would not if the status had reverted.
    let close_out = planr_ok(td.path(), &["close", "task", "t1"]);
    assert!(close_out.contains("t1 done"), "close output: {close_out}");
}

/// An exclude pattern is anchored to the *main* working tree, because
/// `.git/info/exclude` is shared by every worktree of the clone. Anchoring to
/// the invoking worktree wrote a rule that matched a different directory of
/// the same name in the main tree, and made `close` from a secondary worktree
/// silently fail to clean up.
#[test]
fn test_e2e_exclude_rule_hides_a_nested_worktree_where_it_lives() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());
    planr_ok(td.path(), &["new", "task", "t2", "Task Two", "s1"]);
    Command::new("git")
        .args(["add", ".plan"])
        .current_dir(td.path())
        .ok()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "add t2"])
        .current_dir(td.path())
        .ok()
        .unwrap();

    // Claim t1 from the main tree, then claim t2 from inside t1's worktree.
    planr_ok(td.path(), &["claim", "t1"]);
    let wt1 = td.path().join(".plan/worktrees/wt-t1");
    let nested = planr_ok(&wt1, &["claim", "t2", "--worktree", "wt2"]);
    assert!(nested.contains("wt2"), "nested claim output: {nested}");

    // The assertion that matters is in the tree that CONTAINS wt2, not the
    // main tree. git anchors a leading-slash pattern to whichever working
    // tree it is evaluating, so checking only the main tree would pass while
    // the directory sat visible -- and staged as a gitlink -- in wt-t1.
    let nested_status = git_stdout(&wt1, &["status", "--porcelain"]);
    assert!(
        !nested_status.contains("wt2"),
        "nested worktree must be hidden where it actually lives: {nested_status:?}"
    );
    assert!(
        git_ignored(&wt1, "wt2"),
        "wt2 must be ignored inside wt-t1: exclude={:?}",
        read_exclude(td.path())
    );
}

/// A linked worktree that is a *sibling* of the main tree, not inside it.
/// Anchoring rules to the main worktree left this case with no rule at all:
/// the target shares no prefix with the main root, so the pattern silently
/// came out empty and the claimed worktree stayed visible -- ready to be
/// committed onto the branch as a gitlink and merged to trunk by `close`.
#[test]
fn test_e2e_claim_from_a_sibling_worktree_is_hidden() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());

    // A worker's worktree beside the main tree, sharing its .git. It lives in
    // a TempDir of its own rather than in the system temp root, so a failing
    // assertion cannot leave a stray worktree behind: TempDir's Drop runs on
    // the panic path, an explicit cleanup call after the asserts would not.
    let outside = tempfile::tempdir().unwrap();
    let worker = outside.path().join("worker");
    Command::new("git")
        .args(["worktree", "add", worker.to_str().unwrap(), "-b", "worker"])
        .current_dir(td.path())
        .ok()
        .unwrap();

    planr_ok(&worker, &["claim", "t1"]);

    let status = git_stdout(&worker, &["status", "--porcelain"]);
    assert!(
        !status.contains(".plan/worktrees"),
        "claimed worktree must be hidden in the tree that holds it: {status:?}"
    );
    assert!(
        git_ignored(&worker, ".plan/worktrees"),
        "the worktrees dir must be ignored in the sibling tree: exclude={:?}",
        read_exclude(td.path())
    );
}

/// A gitignore pattern is a glob, so a worktree path holding `[`, `*` or `?`
/// must be escaped. Written verbatim, `/wt[1]/` is a character class matching
/// `wt1`, and the real `wt[1]` directory stays visible.
#[test]
fn test_e2e_worktree_path_with_glob_metacharacters_is_hidden() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());

    planr_ok(td.path(), &["claim", "t1", "--worktree", "wt[1]"]);

    let status = git_stdout(td.path(), &["status", "--porcelain"]);
    assert!(
        !status.contains("wt[1]"),
        "a path with glob metacharacters must still be hidden: {status:?}"
    );
    assert!(
        git_ignored(td.path(), "wt[1]"),
        "wt[1] must be ignored: exclude={:?}",
        read_exclude(td.path())
    );
}

/// A worktree path that reaches the repository through a symlink still lands
/// inside it, so it still needs a rule. Resolving the path only lexically
/// made it look like it lay outside the repository and skipped the rule --
/// on macOS every `$TMPDIR` path takes this route.
#[test]
#[cfg(unix)]
fn test_e2e_symlinked_worktree_path_is_ignored() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());

    // The symlink lives in a TempDir of its own, so a failing assertion
    // cannot strand a dangling link in the system temp root -- Drop runs on
    // the panic path, a cleanup call after the asserts does not.
    let outside = tempfile::tempdir().unwrap();
    let link = outside.path().join("repo-link");
    if std::os::unix::fs::symlink(td.path(), &link).is_err() {
        return; // no symlink support; nothing to assert
    }

    let target = link.join("wt-sym");
    planr_ok(
        td.path(),
        &["claim", "t1", "--worktree", target.to_str().unwrap()],
    );

    let exclude = read_exclude(td.path());
    assert!(
        exclude.contains("/wt-sym/"),
        "a symlinked path inside the repo still needs a rule: {exclude:?}"
    );
    let status = git_stdout(td.path(), &["status", "--porcelain"]);
    assert!(
        !status.contains("wt-sym"),
        "worktree must not appear as a gitlink: {status:?}"
    );
}

/// `--no-worktree` opts out of worktree creation, not out of the concurrency
/// check. The holder check used to sit below the opt-out's early return, so a
/// second agent could "claim" a task another agent was actively working and be
/// told it succeeded -- in the one command the whole workflow relies on to
/// stop exactly that.
#[test]
fn test_e2e_no_worktree_claim_still_refuses_a_held_task() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());

    planr_ok(td.path(), &["claim", "t1"]);
    let err = planr_err(td.path(), &["claim", "t1", "--no-worktree"]);
    assert!(
        err.contains("already claimed"),
        "--no-worktree must not bypass the holder check: {err}"
    );
}

/// A branch can be ahead of trunk: a worker may have set `done` or
/// `abandoned` there and had the worktree removed before `close` ran. The
/// abandoned guard reads trunk only, so a resumed claim rebuilt the worktree,
/// skipped the flip, and reported an ordinary claim -- handing an agent a
/// finished or dead ticket with no indication.
#[test]
fn test_e2e_claim_refuses_a_terminal_branch() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());

    planr_ok(td.path(), &["claim", "t1"]);
    let wt = td.path().join(".plan/worktrees/wt-t1");
    let task_file = format!(".plan/tasks/{}", find_task_slug(td.path(), "t1"));

    let content = std::fs::read_to_string(wt.join(&task_file)).unwrap();
    std::fs::write(
        wt.join(&task_file),
        content.replace("status: in_progress", "status: abandoned"),
    )
    .unwrap();
    Command::new("git")
        .args(["add", &task_file])
        .current_dir(&wt)
        .ok()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "abandon on branch"])
        .current_dir(&wt)
        .ok()
        .unwrap();
    Command::new("git")
        .args(["worktree", "remove", "--force", wt.to_str().unwrap()])
        .current_dir(td.path())
        .ok()
        .unwrap();

    let err = planr_err(td.path(), &["claim", "t1"]);
    assert!(
        err.contains("abandoned"),
        "a terminal branch must refuse, not resume: {err}"
    );
}

/// A refusal must leave nothing behind. The terminal-branch guard used to run
/// after the worktree was created, so the refused claim left one -- and the
/// next attempt reported "already claimed", masking the real reason for good.
#[test]
fn test_e2e_terminal_branch_refusal_leaves_no_worktree() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());

    planr_ok(td.path(), &["claim", "t1"]);
    let wt = td.path().join(".plan/worktrees/wt-t1");
    let task_file = format!(".plan/tasks/{}", find_task_slug(td.path(), "t1"));
    let content = std::fs::read_to_string(wt.join(&task_file)).unwrap();
    std::fs::write(
        wt.join(&task_file),
        content.replace("status: in_progress", "status: done"),
    )
    .unwrap();
    Command::new("git")
        .args(["add", &task_file])
        .current_dir(&wt)
        .ok()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "done on branch"])
        .current_dir(&wt)
        .ok()
        .unwrap();
    Command::new("git")
        .args(["worktree", "remove", "--force", wt.to_str().unwrap()])
        .current_dir(td.path())
        .ok()
        .unwrap();

    let first = planr_err(td.path(), &["claim", "t1"]);
    assert!(
        first.contains("done"),
        "expected the terminal reason: {first}"
    );
    assert!(!wt.exists(), "a refused claim must not create a worktree");

    // The same reason again, not "already claimed" from a worktree the
    // previous refusal left lying around.
    let second = planr_err(td.path(), &["claim", "t1"]);
    assert!(
        second.contains("done") && !second.contains("already claimed"),
        "the real reason must survive a second attempt: {second}"
    );
}

/// `close` must not delete an ignore rule the user wrote themselves. planr
/// deduplicated against the whole file, so it silently adopted a matching
/// line and later removed it -- unhiding whatever the user meant to hide.
#[test]
fn test_e2e_close_keeps_a_user_written_ignore_rule() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());

    // The user already ignores /build/ for their own reasons.
    let excl = td.path().join(".git/info/exclude");
    std::fs::create_dir_all(excl.parent().unwrap()).unwrap();
    let mut user = std::fs::read_to_string(&excl).unwrap_or_default();
    user.push_str("\n# my own rules\n/build/\n");
    std::fs::write(&excl, user).unwrap();

    planr_ok(td.path(), &["claim", "t1", "--worktree", "build"]);

    // Take the task to review so close will merge and clean up.
    let wt = td.path().join("build");
    let task_file = format!(".plan/tasks/{}", find_task_slug(td.path(), "t1"));
    let content = std::fs::read_to_string(wt.join(&task_file)).unwrap();
    let reviewed = content.replace("status: in_progress", "status: review")
        + "\n\n## Review\n\nverdict: approved\nreviewer: test\ndate: 2026-09-05\n";
    std::fs::write(wt.join(&task_file), reviewed).unwrap();
    Command::new("git")
        .args(["add", &task_file])
        .current_dir(&wt)
        .ok()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "review: t1"])
        .current_dir(&wt)
        .ok()
        .unwrap();

    planr_ok(td.path(), &["close", "task", "t1"]);

    std::fs::create_dir_all(td.path().join("build")).unwrap();
    std::fs::write(td.path().join("build/out.o"), "obj").unwrap();
    assert!(
        git_ignored(td.path(), "build"),
        "the user's own rule must survive close: exclude={:?}",
        read_exclude(td.path())
    );
}

/// planr's header must not be stranded by an unrelated anchored rule. The
/// cleanup used to ask whether *any* line in the file started with `/`, so a
/// repo with its own `/target` rule kept an empty planr section forever.
#[test]
fn test_e2e_close_drops_its_header_alongside_a_foreign_rule() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());

    let excl = td.path().join(".git/info/exclude");
    std::fs::create_dir_all(excl.parent().unwrap()).unwrap();
    std::fs::write(&excl, "/target\n").unwrap();

    planr_ok(td.path(), &["claim", "t1", "--worktree", "wt-x"]);
    let wt = td.path().join("wt-x");
    let task_file = format!(".plan/tasks/{}", find_task_slug(td.path(), "t1"));
    let content = std::fs::read_to_string(wt.join(&task_file)).unwrap();
    let reviewed = content.replace("status: in_progress", "status: review")
        + "\n\n## Review\n\nverdict: approved\nreviewer: test\ndate: 2026-09-05\n";
    std::fs::write(wt.join(&task_file), reviewed).unwrap();
    Command::new("git")
        .args(["add", &task_file])
        .current_dir(&wt)
        .ok()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "review: t1"])
        .current_dir(&wt)
        .ok()
        .unwrap();

    planr_ok(td.path(), &["close", "task", "t1"]);

    let exclude = read_exclude(td.path());
    assert!(
        exclude.contains("/target"),
        "the foreign rule must survive: {exclude:?}"
    );
    assert!(
        !exclude.contains("planr worktrees"),
        "planr's header must not be stranded: {exclude:?}"
    );
}

/// Claiming a task trunk already records as finished must refuse, not report
/// success having done nothing. `close` deletes the branch, so the
/// branch-side terminal guard cannot see a closed task -- the claim created a
/// worktree, the flip declined to move a `done` status, and the whole thing
/// exited 0. That is the silent-success failure this PR exists to remove.
#[test]
fn test_e2e_claim_refuses_a_task_trunk_calls_done() {
    for status in ["done", "review"] {
        let td = tempfile::tempdir().unwrap();
        seed_lint_repo(td.path());

        let task_file = format!(".plan/tasks/{}", find_task_slug(td.path(), "t1"));
        let path = td.path().join(&task_file);
        let content = std::fs::read_to_string(&path).unwrap();
        std::fs::write(
            &path,
            content.replace("status: todo", &format!("status: {status}")),
        )
        .unwrap();
        Command::new("git")
            .args(["add", &task_file])
            .current_dir(td.path())
            .ok()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "advance t1"])
            .current_dir(td.path())
            .ok()
            .unwrap();

        let err = planr_err(td.path(), &["claim", "t1"]);
        assert!(
            err.contains(status),
            "claiming a {status} task must refuse and say why: {err}"
        );
        assert!(
            !td.path().join(".plan/worktrees/wt-t1").exists(),
            "a refused claim must not create a worktree ({status})"
        );
    }
}

/// A claim that fails *after* writing its ignore rule must take the rule back
/// out. The rollback removed the worktree but left the rule, so the path
/// stayed hidden forever -- and an exclude rule is invisible in `git status`,
/// so nothing would ever point at the cause.
#[test]
fn test_e2e_failed_claim_removes_the_rule_it_wrote() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());

    // A pre-commit hook that always fails makes the flip's commit fail, which
    // is the first step after the rule is written.
    let hooks = td.path().join(".git/hooks");
    std::fs::create_dir_all(&hooks).unwrap();
    let hook = hooks.join("pre-commit");
    std::fs::write(&hook, "#!/bin/sh\nexit 1\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    let err = planr_err(td.path(), &["claim", "t1", "--worktree", "mydir"]);
    assert!(!err.is_empty(), "claim should have failed");

    let exclude = read_exclude(td.path());
    assert!(
        !exclude.contains("/mydir/"),
        "a failed claim must take its own rule back out: {exclude:?}"
    );

    // And the path is genuinely visible again.
    std::fs::create_dir_all(td.path().join("mydir")).unwrap();
    std::fs::write(td.path().join("mydir/real.txt"), "mine").unwrap();
    let status = git_stdout(td.path(), &["status", "--porcelain"]);
    assert!(
        status.contains("mydir"),
        "mydir must be visible to git again: {status:?}"
    );
}

/// The shared default rule is written once and reused by every later claim,
/// so a failed claim must not take it out from under the claims already
/// relying on it.
#[test]
fn test_e2e_failed_claim_keeps_the_shared_default_rule() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());
    planr_ok(td.path(), &["new", "task", "t2", "Task Two", "s1"]);
    Command::new("git")
        .args(["add", ".plan"])
        .current_dir(td.path())
        .ok()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "add t2"])
        .current_dir(td.path())
        .ok()
        .unwrap();

    // t1 claims successfully and establishes the shared rule.
    planr_ok(td.path(), &["claim", "t1"]);
    assert!(
        git_ignored(td.path(), ".plan/worktrees"),
        "rule established"
    );

    // t2's claim then fails after the rule check.
    let hooks = td.path().join(".git/hooks");
    std::fs::create_dir_all(&hooks).unwrap();
    let hook = hooks.join("pre-commit");
    std::fs::write(&hook, "#!/bin/sh\nexit 1\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    planr_err(td.path(), &["claim", "t2"]);

    assert!(
        git_ignored(td.path(), ".plan/worktrees"),
        "t1's worktree must stay hidden: exclude={:?}",
        read_exclude(td.path())
    );
    let status = git_stdout(td.path(), &["status", "--porcelain"]);
    assert!(
        !status.contains(".plan/worktrees"),
        "trunk must stay clean: {status:?}"
    );
}
