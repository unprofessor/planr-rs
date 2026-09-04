//! Scenarios for `planr new`: the slug and parent guards, the
//! epic -> story -> task happy path, and concurrent creation.

use crate::common::*;
use assert_cmd::Command;

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
    git_must(td.path(), &["add", ".plan"]);
    git_must(td.path(), &["commit", "-m", "seed"]);

    // Verify the task file has aliases
    let content = std::fs::read_to_string(td.path().join(&t1)).unwrap();
    assert!(content.contains("aliases: [http-proxy]"), "aliases filled");
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
