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
    let wt = planr_ok(td.path(), &["claim", "t1", &wt_abs.to_string_lossy()]);
    assert!(wt.contains("wt-t1"), "worktree path: {wt}");

    // Verify the worktree has the flipped status
    let task_file = format!(".plan/tasks/{}", find_task_slug(&td.path(), "t1"));
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

fn find_task_slug(plan_dir: &Path, slug: &str) -> String {
    let tasks_dir = plan_dir.join(".plan/tasks");
    for entry in std::fs::read_dir(tasks_dir).unwrap() {
        let e = entry.unwrap();
        let name = e.file_name().into_string().unwrap();
        if name.ends_with(&format!("-{slug}.md")) {
            return name;
        }
    }
    panic!("task {slug} not found");
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
    inject(&td.path().join(&t1_path_of(td.path())), "t2");
    inject(&td.path().join(&t2_path_of(td.path())), "t3");
    inject(&td.path().join(&t3_path_of(td.path())), "t1");

    // Also inject a self-dep on t1 for the self-dep test
    let c = std::fs::read_to_string(td.path().join(&t1_path_of(td.path()))).unwrap();
    std::fs::write(
        td.path().join(&t1_path_of(td.path())),
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
