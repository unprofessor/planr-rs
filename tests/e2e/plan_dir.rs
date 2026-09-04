//! One plan directory, wherever planr is run from, and what every
//! command says when it cannot find or read it.

use crate::common::*;
use assert_cmd::Command;
use std::path::Path;

// ---------------------------------------------------------------------------
// Scenario: the plan directory is the repository's, wherever planr is run
// ---------------------------------------------------------------------------

/// `new` writes the plan directory and `board` reads it, so from the same
/// subdirectory they have to be talking about the same one. They were not:
/// `new` resolved `.plan` against the process directory, wrote
/// `<subdir>/.plan/epics/01-e1.md`, printed it and exited 0, while `board`
/// read the repository root and reported a total of zero with an empty
/// stderr -- a ticket the tool had just made and then denied existed.
#[test]
fn test_e2e_new_and_board_agree_from_a_subdirectory() {
    let td = tempfile::tempdir().unwrap();
    init_repo(td.path());
    let sub = td.path().join("sub");
    std::fs::create_dir_all(&sub).unwrap();

    let created = planr_ok(&sub, &["new", "epic", "e1", "E1"]);
    // The path is printed for the caller to open, and the caller is standing
    // in `sub`. A repo-relative path taken at face value there -- by
    // `$EDITOR $(planr new ...)`, say -- made a stray file under `sub`.
    assert!(
        Path::new(&created).is_absolute(),
        "the printed path must resolve from the caller's directory: {created}"
    );
    assert!(
        sub.join(&created).exists(),
        "the printed path must open from where planr was run: {created}"
    );
    assert!(
        td.path().join(".plan/epics/01-e1.md").exists(),
        "the ticket belongs to the repository's backlog, not the subdirectory's"
    );
    assert!(
        !sub.join(".plan").exists(),
        "and the printed path must not have made a second backlog under `sub`"
    );

    let (out, err) = planr_ok_both(&sub, &["board"]);
    assert!(
        out.lines().any(|l| l.starts_with("e1")),
        "the board run from the same directory must show the ticket: {out}"
    );
    assert!(
        err.is_empty(),
        "nothing to warn about -- the backlog was found: {err}"
    );

    // And the board run from the root agrees with the board run from `sub`.
    let (root_out, _) = planr_ok_both(td.path(), &["board"]);
    assert_eq!(
        out, root_out,
        "the board must not depend on which directory it was run from"
    );
}

/// The same defect with an explicit plan directory: `board` read
/// `<root>/myplan` while `new` wrote `<subdir>/myplan`.
#[test]
fn test_e2e_new_honours_a_relative_plan_dir_from_a_subdirectory() {
    let td = tempfile::tempdir().unwrap();
    init_repo(td.path());
    for d in &["myplan/epics", "myplan/stories", "myplan/tasks"] {
        std::fs::create_dir_all(td.path().join(d)).unwrap();
    }
    let sub = td.path().join("sub");
    std::fs::create_dir_all(&sub).unwrap();

    planr_ok(&sub, &["-D", "myplan", "new", "epic", "e1", "E1"]);
    assert!(
        td.path().join("myplan/epics/01-e1.md").exists(),
        "the plan directory is relative to the repository"
    );
    assert!(
        !sub.join("myplan").exists(),
        "and never to the directory planr was run from"
    );
    let out = planr_ok(&sub, &["-D", "myplan", "board"]);
    assert!(
        out.lines().any(|l| l.starts_with("e1")),
        "the board reads what new wrote: {out}"
    );
}

/// `lint` prints nothing at all for a clean backlog, so a typo'd plan
/// directory was byte-identical to a clean bill of health, exit code
/// included: it certified a backlog it had never opened.
#[test]
fn test_e2e_lint_says_so_when_the_plan_directory_is_not_there() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());

    let (out, err) = planr_ok_both(td.path(), &["-D", ".plans", "lint"]);
    assert!(
        err.contains("no plan directory at '.plans'"),
        "lint must not certify a directory it never opened: stderr={err:?} stdout={out:?}"
    );

    // A backlog that is really there still lints in silence.
    let (_, clean_err) = planr_ok_both(td.path(), &["lint"]);
    assert!(
        !clean_err.contains("no plan directory"),
        "the real backlog is there: {clean_err}"
    );
}

/// In-flight rows against a total of zero, in complete silence, is a gap the
/// reader cannot explain. The per-branch warning was rightly suppressed --
/// an empty list says nothing about any one slug -- but nothing replaced it.
#[test]
fn test_e2e_board_says_so_when_it_read_no_tickets_at_all() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());
    git_must(td.path(), &["branch", "plan/t1"]);

    // Take the backlog away: every working-tree read now comes back empty.
    std::fs::remove_dir_all(td.path().join(".plan")).unwrap();

    let (out, err) = planr_ok_both(td.path(), &["board"]);
    assert!(out.contains("plan/t1"), "the branch is still listed: {out}");
    assert!(
        err.contains("read no tickets at all") && err.contains("1 in-flight branch"),
        "the gap between the in-flight table and the totals must be explained: {err}"
    );
    assert!(
        !err.contains("no task 't1'"),
        "an empty read says nothing about 't1' in particular: {err}"
    );
}

/// A ticket with no frontmatter at all, or with frontmatter that omits `id`,
/// used to warn as "(no id)" -- two identical, unattributable lines for two
/// different files, while `lint` next door named both paths.
#[test]
fn test_e2e_board_names_every_ticket_it_cannot_place() {
    let td = tempfile::tempdir().unwrap();
    init_repo(td.path());
    write_file(td.path(), ".plan/tasks/notes.md", "# Notes\n");
    write_file(td.path(), ".plan/tasks/scratch.md", "# More notes\n");

    let (_, err) = planr_ok_both(td.path(), &["board"]);
    assert!(
        !err.contains("(no id)"),
        "no ticket should stay anonymous: {err}"
    );
    assert!(
        err.contains("ticket 'notes'") && err.contains("ticket 'scratch'"),
        "both files must be named apart: {err}"
    );
}

/// The broken file's id is recovered from its filename and its status is the
/// default `todo`, not a value read from anywhere. Letting that into the
/// status map overwrote the real `done`, and every dependent task turned up
/// BLOCKED-BY on the same screen where the ticket itself read `done`.
#[test]
fn test_e2e_a_broken_duplicate_does_not_block_a_satisfied_dependency() {
    let td = tempfile::tempdir().unwrap();
    init_repo(td.path());
    write_file(
        td.path(),
        ".plan/stories/01-s1.md",
        "---\nid: s1\nkind: story\nstatus: todo\ntitle: S1\n---\n",
    );
    write_file(
        td.path(),
        ".plan/tasks/01-dep.md",
        "---\nid: dep\nkind: task\nparent: s1\nstatus: done\ntitle: Dep\n---\n",
    );
    write_file(
        td.path(),
        ".plan/tasks/03-user.md",
        "---\nid: user\nkind: task\nparent: s1\nstatus: todo\ntitle: User\ndepends_on: [dep]\n---\n",
    );

    // Every way a second file can end up carrying the slug `dep` without
    // having claimed it. The parse error was the only one the first fix
    // guarded, and the third walked straight through that guard: valid
    // frontmatter, a real kind, no `id` -- so it passed the kind filter,
    // took the finished ticket's slug, and overwrote its `done` in silence.
    for (what, body) in [
        (
            "frontmatter that does not parse",
            "---\nid: dep\nkind: task\ntitle: Dep: broken\n---\n",
        ),
        ("no frontmatter at all", "# Dep notes\n\nnothing to see\n"),
        (
            "valid frontmatter, a real kind, no id",
            "---\nkind: task\nparent: s1\nstatus: todo\ntitle: Broken dup\n---\n",
        ),
    ] {
        write_file(td.path(), ".plan/tasks/02-dep.md", body);

        let (out, err) = planr_ok_both(td.path(), &["board"]);
        let user_row = out
            .lines()
            .find(|l| l.starts_with("user"))
            .unwrap_or_else(|| panic!("{what}: no row for 'user': {out}"))
            .to_string();
        assert!(
            !user_row.contains(" dep "),
            "{what}: 'dep' is done, so 'user' must not be BLOCKED-BY it: {user_row}"
        );
        assert!(
            out.lines()
                .any(|l| l.starts_with("blocked") && l.ends_with('0')),
            "{what}: nothing is blocked: {out}"
        );
        // And the board says what it did with the file. Silence was the worst
        // part of this: the parse-error variant at least warned.
        assert!(
            err.contains(".plan/tasks/02-dep.md"),
            "{what}: the board must name the file it declined to trust: {err:?}"
        );
    }
}

/// A slug that is both a real trunk task and the recovered id of some
/// unreadable file took the "counts towards nothing" arm -- which is false of
/// a task sitting in the tasks table, and which swallowed the invalid status
/// the reader can actually act on.
#[test]
fn test_e2e_board_keeps_the_actionable_warning_about_a_real_task() {
    let td = tempfile::tempdir().unwrap();
    init_repo(td.path());
    write_file(
        td.path(),
        ".plan/stories/01-net.md",
        "---\nid: net\nkind: story\nstatus: todo\ntitle: Net\n---\n",
    );
    write_file(
        td.path(),
        ".plan/tasks/01-proxy.md",
        "---\nid: proxy\nkind: task\nparent: net\nstatus: todo\ntitle: Proxy\n---\n",
    );
    git_must(td.path(), &["add", "-A"]);
    git_must(td.path(), &["commit", "-m", "seed tickets"]);

    // A branch whose task file carries a status lint would reject.
    git_must(td.path(), &["checkout", "-b", "plan/proxy"]);
    write_file(
        td.path(),
        ".plan/tasks/01-proxy.md",
        "---\nid: proxy\nkind: task\nparent: net\nstatus: wip\ntitle: Proxy\n---\n",
    );
    git_must(td.path(), &["commit", "-am", "claim with a typo'd status"]);
    git_must(td.path(), &["checkout", "main"]);

    // A story of the same slug whose frontmatter does not parse.
    write_file(
        td.path(),
        ".plan/stories/02-proxy.md",
        "---\nid: proxy\nkind: story\ntitle: Proxy: broken\n---\n",
    );

    let (out, err) = planr_ok_both(td.path(), &["board"]);
    assert!(
        err.contains("invalid status") && err.contains("wip"),
        "the actionable warning must survive: {err}"
    );
    assert!(
        !err.contains("counts towards nothing"),
        "the task is shown and counted, so that would be false: {err} / {out}"
    );
}

/// Outside a repository there is no root to enter, and saying so on every run
/// would be noise. Any other reason git could not answer is worth a word,
/// because what follows is a report about a backlog read from wherever the
/// process happened to start.
#[cfg(unix)]
#[test]
fn test_e2e_a_failed_git_toplevel_is_not_silent() {
    use std::os::unix::fs::PermissionsExt;

    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());
    let bin = td.path().join("fakebin");
    std::fs::create_dir_all(&bin).unwrap();
    let shim = bin.join("git");
    std::fs::write(
        &shim,
        "#!/bin/sh\necho 'fatal: detected dubious ownership in repository' >&2\nexit 128\n",
    )
    .unwrap();
    std::fs::set_permissions(&shim, std::fs::Permissions::from_mode(0o755)).unwrap();

    let out = Command::cargo_bin("planr")
        .unwrap()
        .args(["lint"])
        .current_dir(td.path())
        .env("PATH", &bin)
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(
        err.contains("could not ask git for the repository root")
            && err.contains("dubious ownership"),
        "a git failure that is not 'no repo here' must be reported: {err:?}"
    );

    // Outside a repository, though, there is nothing to say.
    let outside = tempfile::tempdir().unwrap();
    let (_, quiet) = planr_ok_both(outside.path(), &["lint"]);
    assert!(
        !quiet.contains("could not ask git"),
        "no repository here is normal, not a failure: {quiet}"
    );
}

// ---------------------------------------------------------------------------
// Scenario: one answer to where the backlog is
// ---------------------------------------------------------------------------

/// `board` and `new` agreeing about the backlog while `claim`, `close` and
/// `review` disagreed was not half a fix, it was a relocated split -- and the
/// half that was left over stated something untrue. Run from a subdirectory,
/// `planr claim t1` failed with "no task file for slug 't1' on main" about a
/// file committed on main at `.plan/tasks/01-t1.md`.
#[test]
fn test_e2e_every_command_uses_the_backlog_at_the_repository_root() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());
    let sub = td.path().join("src/deep");
    std::fs::create_dir_all(&sub).unwrap();

    // review reads the task file for a branch that does not exist yet, so it
    // fails either way -- but it must fail about the branch, not about a file
    // that is sitting on trunk.
    let review_err = planr_err(&sub, &["review", "t1"]);
    assert!(
        !review_err.contains("no task file"),
        "review from a subdirectory must find the trunk file: {review_err}"
    );

    // claim is the one command that was deliberately left behind.
    let printed = planr_ok(&sub, &["claim", "t1"]);
    assert!(
        Path::new(&printed).is_absolute() && Path::new(&printed).is_dir(),
        "claim must print a worktree path the caller can use: {printed}"
    );
    assert!(
        td.path().join(".plan/worktrees/wt-t1").is_dir(),
        "the worktree belongs to the repository's plan directory: {printed}"
    );

    // close now gets far enough to refuse for the real reason: the task is
    // in_progress, not review. Before, it refused for a false one.
    let close_err = planr_err(&sub, &["close", "task", "t1"]);
    assert!(
        !close_err.contains("no task file"),
        "close from a subdirectory must find the branch's file: {close_err}"
    );
    assert!(
        close_err.contains("must be 'review'"),
        "close should refuse on the status it read: {close_err}"
    );
}

/// The one thing that must not move to the root with everything else: a path
/// the caller typed. `--worktree ../wt` means the caller's `..`, and resolving
/// it at the root instead would put the worktree somewhere they never named.
#[test]
fn test_e2e_claim_resolves_a_relative_worktree_against_the_callers_directory() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());
    let sub = td.path().join("sub");
    std::fs::create_dir_all(&sub).unwrap();

    let printed = planr_ok(&sub, &["claim", "t1", "--worktree", "wt"]);
    assert!(
        sub.join("wt").is_dir(),
        "the worktree belongs where the caller pointed: printed {printed}, \
         root has {}",
        td.path().join("wt").exists()
    );
    assert!(
        !td.path().join("wt").exists(),
        "and not at the repository root"
    );
    assert!(
        git_ignored(td.path(), "sub/wt"),
        "the worktree still has to be hidden from trunk: {}",
        read_exclude(td.path())
    );
}

// ---------------------------------------------------------------------------
// Scenario: reading git's own messages
// ---------------------------------------------------------------------------

/// planr tells "there is no repository here" from a real git failure by
/// matching git's wording, so the child has to be pinned to a locale whose
/// wording planr knows. Left to the environment, every ordinary run outside a
/// repository under a translated locale warned that planr could not ask git
/// where the repository was -- exactly the noise the match exists to avoid.
///
/// The locale is pinned on the child here rather than assumed of the runner:
/// a `git` stub answers in French unless it was run under `LC_ALL=C`, so the
/// test says the same thing on a machine with no French locale installed.
#[cfg(unix)]
#[test]
fn test_e2e_outside_a_repository_is_quiet_under_a_translated_locale() {
    use std::os::unix::fs::PermissionsExt;

    let td = tempfile::tempdir().unwrap();
    let bin = td.path().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let stub = bin.join("git");
    std::fs::write(
        &stub,
        "#!/bin/sh\n\
         if [ \"$1 $2\" = \"rev-parse --show-toplevel\" ] && [ \"$LC_ALL\" != C ]; then\n\
         \techo \"fatal : ni ceci ni aucun de ses repertoires parents n est un depot git : .git\" >&2\n\
         \texit 128\n\
         fi\n\
         echo \"fatal: not a git repository (or any of the parent directories): .git\" >&2\n\
         exit 128\n",
    )
    .unwrap();
    std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();

    let plain = td.path().join("plain");
    std::fs::create_dir_all(&plain).unwrap();
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );

    for locale in ["fr_FR.UTF-8", "de_DE.UTF-8"] {
        let out = Command::cargo_bin("planr")
            .unwrap()
            .args(["board"])
            .current_dir(&plain)
            .env("PATH", &path)
            .env("LC_ALL", locale)
            .env("LANGUAGE", "fr")
            .output()
            .unwrap();
        let err = String::from_utf8(out.stderr).unwrap();
        assert!(
            !err.contains("could not ask git for the repository root"),
            "LC_ALL={locale}: being outside a repository is ordinary: {err}"
        );
        assert!(
            err.contains("no plan directory"),
            "LC_ALL={locale}: planr should still have run: {err}"
        );
    }
}
