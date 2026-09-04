//! Scenarios for `planr board`: what it counts, where it reads from,
//! and the in-flight section the leader sees from trunk.

use crate::common::*;

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

/// The backlog lives at the repository root, and planr resolves `--plan-dir`
/// relative to it -- but every reader used to open that path relative to the
/// process directory. Run from a subdirectory, `board` read no tickets at
/// all: it rendered empty tables and warned that every in-flight branch named
/// a task that is not on trunk, about tickets that were committed and
/// present. `lint` was worse: it reported a clean backlog it had never
/// opened.
#[test]
fn test_e2e_board_and_lint_read_the_backlog_from_a_subdirectory() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());
    // A dangling dependency, so lint has something to find.
    let t1 = td.path().join(t1_path_of(td.path()));
    let body = std::fs::read_to_string(&t1).unwrap();
    std::fs::write(&t1, body.replace("depends_on: []", "depends_on: [ghost]")).unwrap();

    let sub = td.path().join("src/deep");
    std::fs::create_dir_all(&sub).unwrap();

    let (root_board, _) = planr_ok_both(td.path(), &["board"]);
    let (sub_board, sub_err) = planr_ok_both(&sub, &["board"]);
    assert!(
        sub_board.contains("## tasks") && sub_board.lines().any(|l| l.starts_with("t1")),
        "the board from a subdirectory must show the backlog: {sub_board}"
    );
    // Identical but for the source header, which names the same repo root.
    assert_eq!(
        sub_board.split("## tasks").nth(1),
        root_board.split("## tasks").nth(1),
        "same repo, same board: sub={sub_board} root={root_board}"
    );
    assert!(
        sub_err.is_empty(),
        "nothing is wrong with this backlog's branches: {sub_err}"
    );

    let sub_lint = planr(&sub, &["lint"]);
    assert!(
        !sub_lint.status.success(),
        "lint from a subdirectory must not certify a backlog it never read"
    );
    let sub_lint_out = String::from_utf8(sub_lint.stdout).unwrap();
    assert!(
        sub_lint_out.contains("ghost"),
        "lint from a subdirectory should find the dangling dep: {sub_lint_out}"
    );
}

/// A task whose frontmatter does not parse is absent from the board's
/// kind-filtered slug set even though the file is committed and sitting right
/// where the reader left it. The board used to report its branch as naming a
/// task that was "renamed or not committed" -- a cause it had not
/// established, about a file nobody had touched.
#[test]
fn test_e2e_board_does_not_call_a_committed_ticket_missing() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());

    // Break t1's frontmatter with an unquoted colon, and give it a branch.
    let t1 = td.path().join(t1_path_of(td.path()));
    let body = std::fs::read_to_string(&t1).unwrap();
    std::fs::write(
        &t1,
        body.replace("title: Task One", "title: Task One: broken"),
    )
    .unwrap();
    git_must(td.path(), &["commit", "-am", "break t1"]);
    git_must(td.path(), &["branch", "plan/t1"]);

    let (out, err) = planr_ok_both(td.path(), &["board"]);
    assert!(
        !err.contains("no task 't1'") && !err.contains("not committed"),
        "the file is committed and present: stderr={err}"
    );
    assert!(
        err.contains("plan/t1") && err.contains("frontmatter did not parse"),
        "the branch warning should name the cause the board can establish: stderr={err}"
    );
    assert!(
        err.contains("ticket 't1'"),
        "the ticket shown in no table should be named: stderr={err}"
    );

    // Shown nowhere means counted nowhere: e1 and s1 are the only rows.
    let total = out
        .lines()
        .find(|l| l.starts_with("total"))
        .and_then(|l| l.split_whitespace().last().map(String::from));
    assert_eq!(
        total,
        Some("2".to_string()),
        "only the two rendered tickets may count: {out}"
    );
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
