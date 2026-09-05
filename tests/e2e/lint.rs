//! Scenarios for `planr lint` in both working-tree and ref mode --
//! the finding classes, and what lint says about a backlog it could
//! not read rather than certifying it.

use crate::common::*;
use std::path::Path;

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
// Scenario: a plan directory that cannot be read is not an empty one
// ---------------------------------------------------------------------------

/// `Path::exists` answers true for a plan directory planr cannot open, and
/// every reader below treats an I/O error as an empty directory, so a backlog
/// that could not be read came out byte-identical to a clean one: `lint` exit
/// 0, no stdout, no stderr. That is the fail-open direction, on the same
/// `Path::exists` this branch has already had to correct twice elsewhere.
#[test]
fn test_e2e_an_unreadable_plan_directory_is_not_a_clean_bill_of_health() {
    let td = tempfile::tempdir().unwrap();
    init_repo(td.path());
    std::fs::remove_dir_all(td.path().join(".plan")).unwrap();
    // A plain file where the backlog should be: no permissions involved, so
    // this case reads the same for every user, root included.
    std::fs::write(td.path().join(".plan"), "not a directory\n").unwrap();

    let (out, err) = planr_ok_both(td.path(), &["lint"]);
    assert!(out.is_empty(), "there was no backlog to report on: {out}");
    assert!(
        err.contains("could not read the plan directory"),
        "lint must not certify a backlog it could not open: {err:?}"
    );
    let (_, board_err) = planr_ok_both(td.path(), &["board"]);
    assert!(
        board_err.contains("could not read the plan directory"),
        "board reads the same directory and owes the same warning: {board_err:?}"
    );
}

/// The same fact one level down: one unreadable `tasks/` hides every task in
/// the backlog just as quietly as an unreadable `.plan` hides all of it.
///
/// Two ways in, because the obvious one does not reach every runner. A mode
/// of `0o000` does not stop root from reading the directory, so under root
/// -- which is where CI often runs -- the chmod half has nothing to check
/// and used to return early, leaving the whole test passing vacuously in
/// exactly the environment least likely to notice. A plain file where the
/// kind directory should be is the same defect by another route, and no
/// privilege lets `read_dir` open a regular file, so that half runs and
/// asserts for everyone.
#[cfg(unix)]
#[test]
fn test_e2e_an_unreadable_kind_directory_is_reported() {
    use std::os::unix::fs::PermissionsExt;

    // A plain file where `tasks/` should be. This half holds for every user.
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());
    let tasks = td.path().join(".plan/tasks");
    std::fs::remove_dir_all(&tasks).unwrap();
    std::fs::write(&tasks, "not a directory\n").unwrap();
    let (out, err) = planr_ok_both(td.path(), &["lint"]);
    assert!(
        err.contains(".plan/tasks"),
        "the directory whose tickets went missing must be named: {err:?}"
    );
    assert!(
        !out.contains("error"),
        "the tickets that were read are still reported on: {out}"
    );

    // And the case the warning was written for: a directory the caller may
    // not open. Skipped under root, where the mode does not bite -- the half
    // above is what keeps the test honest there.
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());
    let tasks = td.path().join(".plan/tasks");
    std::fs::set_permissions(&tasks, std::fs::Permissions::from_mode(0o000)).unwrap();
    let readable_anyway = std::fs::read_dir(&tasks).is_ok();
    let (_, err) = planr_ok_both(td.path(), &["lint"]);
    std::fs::set_permissions(&tasks, std::fs::Permissions::from_mode(0o755)).unwrap();
    if !readable_anyway {
        assert!(
            err.contains(".plan/tasks"),
            "the directory whose tickets went missing must be named: {err:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// Scenario: lint in ref mode
// ---------------------------------------------------------------------------

/// `lint` prints nothing for a clean backlog, so an empty read is
/// byte-identical to a clean bill of health -- and in ref mode there is no
/// directory for the missing-directory warning to look at. A typo'd
/// `--plan-dir` therefore certified a backlog `lint` had never opened, which
/// is the defect that warning exists to prevent, one mode over.
#[test]
fn test_e2e_lint_says_so_when_a_ref_holds_no_backlog() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());

    let (out, err) = planr_ok_both(td.path(), &["-D", "typo", "lint", "main"]);
    assert!(
        out.is_empty(),
        "nothing was read, so nothing to report: {out}"
    );
    assert!(
        err.contains("nothing under 'typo' at 'main'"),
        "lint must not certify a ref it read nothing from: {err:?}"
    );

    // The real plan directory at the same ref is not warned about.
    let (_, clean_err) = planr_ok_both(td.path(), &["lint", "main"]);
    assert!(
        clean_err.is_empty(),
        "a backlog that was read must not be reported as absent: {clean_err:?}"
    );
}

// ---------------------------------------------------------------------------
// Scenario: ref-mode lint on a backlog with no tickets in it yet
// ---------------------------------------------------------------------------

/// The missing-backlog warning must not fire on a backlog that is there.
/// `lint <ref>` warned that there was no backlog at the ref, and told the
/// caller to check `--plan-dir` and the ref, whenever it read no tickets --
/// and a repository that has scaffolded `.plan/{epics,stories,tasks}` and
/// written no tickets yet reads exactly that way. Both the plan directory
/// and the ref were correct, working-tree `lint` and `board` were silent on
/// the identical state, and the warning named a cause it had not
/// established.
///
/// What it must keep saying is checked alongside it, since the fix is a
/// narrowing: a plan directory that really is not in the commit, a typo'd
/// `--plan-dir`, and a ref that does not resolve at all.
#[test]
fn test_e2e_lint_does_not_call_an_empty_backlog_absent() {
    let td = tempfile::tempdir().unwrap();
    init_repo(td.path());
    for kind in ["epics", "stories", "tasks"] {
        std::fs::write(td.path().join(format!(".plan/{kind}/.gitkeep")), "").unwrap();
    }
    git_must(td.path(), &["add", "-A", "-f", ".plan"]);
    git_must(td.path(), &["commit", "-m", "scaffold the backlog"]);

    let (_, err) = planr_ok_both(td.path(), &["lint", "main"]);
    assert!(
        err.is_empty(),
        "the backlog is there and holds no tickets yet -- nothing to warn \
         about, and working-tree lint says nothing either: {err:?}"
    );
    // The claim that they agree, made rather than assumed.
    let (_, wt_err) = planr_ok_both(td.path(), &["lint"]);
    assert!(wt_err.is_empty(), "working-tree lint is silent: {wt_err:?}");

    // A typo'd plan directory is still a plan directory that is not there.
    let (_, err) = planr_ok_both(td.path(), &["-D", "typo", "lint", "main"]);
    assert!(
        err.contains("nothing under 'typo' at 'main'"),
        "lint must not certify a ref it read nothing from: {err:?}"
    );

    // A ref that does not resolve is planr failing to read, not a finding
    // about the backlog -- and the two must not be reported as the same
    // thing.
    let (_, err) = planr_ok_both(td.path(), &["lint", "nosuchref"]);
    assert!(
        err.contains("could not read '.plan' at 'nosuchref'"),
        "a ref that does not resolve has to be named as the reason: {err:?}"
    );

    // And a commit made before the backlog existed still has no backlog in
    // it -- the same repository, one ref earlier.
    let td = tempfile::tempdir().unwrap();
    init_repo(td.path());
    git_must(td.path(), &["checkout", "-q", "--orphan", "before"]);
    git_must(td.path(), &["rm", "-rq", "--cached", "."]);
    std::fs::remove_dir_all(td.path().join(".plan")).unwrap();
    std::fs::write(td.path().join("README.md"), "# before the backlog\n").unwrap();
    git_must(td.path(), &["add", "README.md"]);
    git_must(td.path(), &["commit", "-m", "before the backlog"]);
    let (_, err) = planr_ok_both(td.path(), &["lint", "before"]);
    assert!(
        err.contains("nothing under '.plan' at 'before'"),
        "a ref that predates the backlog has none to lint: {err:?}"
    );
}

// ---------------------------------------------------------------------------
// Scenario: ref-mode lint that could not read the tickets it found
// ---------------------------------------------------------------------------

/// `lint <ref>` must not certify a backlog whose tickets it could not read.
///
/// `lint_ref` skips a ticket file whose blob it cannot show, so a backlog of
/// perfectly ordinary `.md` tickets that planr opened none of renders the
/// way a clean one does: no output, exit 0. The plan directory is populated
/// and the ref resolves, so the missing-backlog warning has nothing to say
/// about it -- correctly, since neither is at fault. What is at fault is the
/// read, and the counts the report carries are what name it.
///
/// The four states are pinned together because the fix has to separate them
/// and not merely fire more often: none of the tickets read, some of them
/// read, a scaffolded `.gitkeep` backlog that holds no tickets at all, and
/// files under the plan directory that are not ticket files. The last two
/// must stay silent -- working-tree `lint` reads them the same way, and
/// planr cannot tell a placeholder from a mis-saved ticket without guessing.
#[test]
fn test_e2e_lint_ref_says_when_it_could_not_read_the_tickets() {
    // ---- none of them read ----
    let td = tempfile::tempdir().unwrap();
    init_repo(td.path());
    planr_ok(td.path(), &["new", "epic", "e1", "E1"]);
    planr_ok(td.path(), &["new", "story", "s1", "S1", "e1"]);
    planr_ok(td.path(), &["new", "task", "foo", "Foo", "s1"]);
    git_must(td.path(), &["add", "-A", "-f", ".plan"]);
    git_must(td.path(), &["commit", "-m", "a backlog"]);
    for f in [
        ".plan/epics/01-e1.md",
        ".plan/stories/01-s1.md",
        ".plan/tasks/01-foo.md",
    ] {
        destroy_blob(td.path(), "main", f);
    }

    let (_, err) = planr_ok_both(td.path(), &["lint", "main"]);
    assert!(
        err.contains("read none of the 3 ticket file(s) under '.plan' at 'main'"),
        "an empty report built from nothing must say so: {err:?}"
    );
    // The cause it established, and not one it did not: the plan directory
    // is right there and the ref resolves.
    assert!(
        !err.contains("nothing under"),
        "the backlog is there -- only the read failed: {err:?}"
    );

    // ---- some of them read ----
    let td = tempfile::tempdir().unwrap();
    init_repo(td.path());
    planr_ok(td.path(), &["new", "epic", "e1", "E1"]);
    planr_ok(td.path(), &["new", "story", "s1", "S1", "e1"]);
    planr_ok(td.path(), &["new", "task", "foo", "Foo", "s1"]);
    git_must(td.path(), &["add", "-A", "-f", ".plan"]);
    git_must(td.path(), &["commit", "-m", "a backlog"]);
    destroy_blob(td.path(), "main", ".plan/tasks/01-foo.md");

    let (_, err) = planr_ok_both(td.path(), &["lint", "main"]);
    assert!(
        err.contains("read 2 of the 3 ticket file(s) under '.plan' at 'main'"),
        "a report built from part of a backlog must say which part: {err:?}"
    );

    // ---- a backlog that holds no tickets is still not a failed read ----
    let td = tempfile::tempdir().unwrap();
    init_repo(td.path());
    for kind in ["epics", "stories", "tasks"] {
        std::fs::write(td.path().join(format!(".plan/{kind}/.gitkeep")), "").unwrap();
    }
    git_must(td.path(), &["add", "-A", "-f", ".plan"]);
    git_must(td.path(), &["commit", "-m", "scaffold the backlog"]);
    let (_, err) = planr_ok_both(td.path(), &["lint", "main"]);
    assert!(
        err.is_empty(),
        "nothing was found and nothing failed to read: {err:?}"
    );

    // ---- and neither are files planr never reads ----
    //
    // Tickets saved under the plan directory with the wrong extension are
    // not ticket files, no more than the `.gitkeep` above is, and planr
    // cannot tell one such file from the other without guessing. Both modes
    // say nothing, and they say it about the same state.
    write_file(
        td.path(),
        ".plan/tasks/01-foo.txt",
        "---\nid: foo\nkind: task\nstatus: todo\ntitle: Foo\ndepends_on: [nope]\n---\n",
    );
    git_must(td.path(), &["add", "-A", "-f", ".plan"]);
    git_must(td.path(), &["commit", "-m", "the wrong extension"]);
    let (_, err) = planr_ok_both(td.path(), &["lint", "main"]);
    assert!(
        err.is_empty(),
        "planr found no ticket files and failed to read none: {err:?}"
    );
    let (_, wt_err) = planr_ok_both(td.path(), &["lint"]);
    assert!(
        wt_err.is_empty(),
        "working-tree lint reads the identical state the identical way: {wt_err:?}"
    );
}
