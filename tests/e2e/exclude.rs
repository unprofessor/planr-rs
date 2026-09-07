//! Scenarios for the ignore rules planr writes into `.git/info/exclude`
//! on the claim and close paths -- what it adds, what it prunes, and what
//! it must leave exactly as it found it.

use crate::common::*;

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
        git_must(&wt, &["add", "-A"]);
        git_must(&wt, &["commit", "-m", "review: t1"]);

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

/// Every path out of the prune leaves the rules in place, which is right --
/// but silence is not. What stays behind hides whatever is created at that
/// path and leaves no trace in `git status`, so unless the prune says it
/// failed, nothing ever points at the rule. `planr abandon` used to exit 0
/// with an empty stderr and the stale rule still sitting there.
#[test]
#[cfg(unix)]
fn test_e2e_abandon_warns_when_it_cannot_prune_ignore_rules() {
    use std::os::unix::fs::PermissionsExt;

    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());

    // A planr rule for a worktree that no longer exists: exactly what the
    // prune is there to remove.
    let exclude = td.path().join(".git/info/exclude");
    std::fs::create_dir_all(exclude.parent().unwrap()).unwrap();
    let stale = "# planr worktrees -- checkouts, not backlog content\n/.plan/worktrees/t1/\n\n";
    std::fs::write(&exclude, stale).unwrap();
    std::fs::set_permissions(&exclude, std::fs::Permissions::from_mode(0o000)).unwrap();
    if std::fs::read_to_string(&exclude).is_ok() {
        // Running as root, or on a filesystem that ignores the mode: the
        // prune would succeed and there would be nothing to report.
        return;
    }

    let (out, err) = planr_ok_both(td.path(), &["abandon", "task", "t1", "OBE"]);
    assert!(
        out.contains("abandoned task t1"),
        "abandon should still do its own job: {out}"
    );
    assert!(
        err.contains("could not prune stale local ignore rules"),
        "a prune that failed must say so: stderr={err}"
    );
    assert!(
        err.contains(".git/info/exclude"),
        "the warning should point at the file to check: stderr={err}"
    );

    std::fs::set_permissions(&exclude, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(
        read_exclude(td.path()),
        stale,
        "the rule is kept, not silently dropped"
    );
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
    git_must(td.path(), &["add", "realsrc"]);
    git_must(td.path(), &["commit", "-m", "add realsrc"]);

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
    git_must(&wt, &["add", &task_file]);
    git_must(&wt, &["commit", "-m", "review: t1"]);

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
    git_must(td.path(), &["add", ".plan"]);
    git_must(td.path(), &["commit", "-m", "add t2"]);

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
    git_must(
        td.path(),
        &["worktree", "add", worker.to_str().unwrap(), "-b", "worker"],
    );

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
    git_must(&wt, &["add", &task_file]);
    git_must(&wt, &["commit", "-m", "review: t1"]);

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
    git_must(&wt, &["add", &task_file]);
    git_must(&wt, &["commit", "-m", "review: t1"]);

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
    git_must(td.path(), &["add", ".plan"]);
    git_must(td.path(), &["commit", "-m", "add t2"]);

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

/// The shared default rule is load-bearing for every worktree beneath it, so
/// the claim that *wrote* it must not take it away when it fails later --
/// another claim may have started under it in the meantime.
///
/// This is the ordering the earlier guard missed: it only kept a rule some
/// other worktree resolved to *identically*, never one that other worktrees
/// merely lived under.
#[test]
fn test_e2e_failed_claim_keeps_the_shared_rule_other_claims_rely_on() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());
    planr_ok(td.path(), &["new", "task", "t2", "Task Two", "s1"]);
    git_must(td.path(), &["add", ".plan"]);
    git_must(td.path(), &["commit", "-m", "add t2"]);

    // t2 claims first and lives under the shared rule.
    planr_ok(td.path(), &["claim", "t2"]);
    let wt2 = td.path().join(".plan/worktrees/wt-t2");
    assert!(wt2.is_dir(), "t2's worktree should exist");

    // Drop planr's block, so the *next* claim is the one that writes the
    // shared rule -- the interleaving that matters, since a rollback only
    // removes a rule its own call wrote. This stands in for the concurrent
    // ordering: claim A writes the rule, claim B starts under it, A fails.
    let excl = td.path().join(".git/info/exclude");
    let stripped: Vec<String> = read_exclude(td.path())
        .lines()
        .filter(|l| !l.contains("planr worktrees") && !l.trim().starts_with("/.plan/"))
        .map(String::from)
        .collect();
    std::fs::write(&excl, stripped.join("\n") + "\n").unwrap();
    assert!(
        !git_ignored(td.path(), ".plan/worktrees"),
        "precondition: the shared rule is gone"
    );

    let hooks = td.path().join(".git/hooks");
    std::fs::create_dir_all(&hooks).unwrap();
    let hook = hooks.join("pre-commit");
    std::fs::write(&hook, "#!/bin/sh\nexit 1\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let _ = planr(td.path(), &["claim", "t1"]);

    // t2 is still there, so it must still be hidden.
    assert!(wt2.is_dir(), "t2's worktree must survive t1's rollback");
    assert!(
        git_ignored(td.path(), ".plan/worktrees"),
        "the shared rule must survive while t2 relies on it: exclude={:?}",
        read_exclude(td.path())
    );
    let status = git_stdout(td.path(), &["status", "--porcelain"]);
    assert!(
        !status.contains(".plan/worktrees"),
        "trunk must not see t2's worktree as a gitlink: {status:?}"
    );
}

/// A backslash is an ordinary filename character on Unix. Rewriting it as a
/// separator split `wt\1` into `wt/1`, so git looked for a `1` inside a `wt`
/// directory and the real worktree stayed visible.
#[test]
#[cfg(unix)]
fn test_e2e_worktree_path_with_backslash_is_hidden() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());

    planr_ok(td.path(), &["claim", "t1", "--worktree", "wt\\1"]);

    let status = git_stdout(td.path(), &["status", "--porcelain"]);
    assert!(
        !status.contains("wt"),
        "a path holding a backslash must still be hidden: {status:?}"
    );
    assert!(
        git_ignored(td.path(), "wt\\1"),
        "wt\\1 must be ignored: exclude={:?}",
        read_exclude(td.path())
    );
}

/// planr writes its block last, so the ordinary way to add a rule by hand --
/// appending to the file -- used to land *inside* that block. planr then read
/// the line as its own, declined to write a duplicate, and `close` deleted it.
///
/// The existing user-rule test adds the rule before the first claim, so it
/// lands above planr's header and never exercises this.
#[test]
fn test_e2e_close_keeps_a_rule_appended_after_planrs_block() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());
    planr_ok(td.path(), &["new", "task", "t2", "Task Two", "s1"]);
    git_must(td.path(), &["add", ".plan"]);
    git_must(td.path(), &["commit", "-m", "add t2"]);

    // planr establishes its block first...
    planr_ok(td.path(), &["claim", "t1", "--worktree", "build"]);

    // ...then the user appends their own rule the obvious way.
    let excl = td.path().join(".git/info/exclude");
    let mut content = std::fs::read_to_string(&excl).unwrap();
    content.push_str("/mydir/\n");
    std::fs::write(&excl, content).unwrap();

    // A claim at that same path must not adopt the user's line.
    planr_ok(td.path(), &["claim", "t2", "--worktree", "mydir"]);

    // Take t2 to review and close it, which removes planr's own rule.
    let wt = td.path().join("mydir");
    let task_file = format!(".plan/tasks/{}", find_task_slug(td.path(), "t2"));
    let content = std::fs::read_to_string(wt.join(&task_file)).unwrap();
    let reviewed = content.replace("status: in_progress", "status: review")
        + "\n\n## Review\n\nverdict: approved\nreviewer: test\ndate: 2026-09-05\n";
    std::fs::write(wt.join(&task_file), reviewed).unwrap();
    git_must(&wt, &["add", &task_file]);
    git_must(&wt, &["commit", "-m", "review: t2"]);
    planr_ok(td.path(), &["close", "task", "t2"]);

    std::fs::create_dir_all(td.path().join("mydir")).unwrap();
    std::fs::write(td.path().join("mydir/out.o"), "obj").unwrap();
    assert!(
        git_ignored(td.path(), "mydir"),
        "the user's appended rule must survive close: exclude={:?}",
        read_exclude(td.path())
    );
}

/// Dropping a stale worktree record must drop its ignore rule too. The next
/// claim can land somewhere else entirely, and `close` only ever considers
/// the path the task is holding now -- so the old rule would stay forever,
/// silently hiding anything created at that path.
#[test]
fn test_e2e_stale_record_takes_its_ignore_rule_with_it() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());

    // Claim at an explicit path, then delete it by hand.
    planr_ok(td.path(), &["claim", "t1", "--worktree", "mydir"]);
    assert!(
        git_ignored(td.path(), "mydir"),
        "precondition: rule written"
    );
    std::fs::remove_dir_all(td.path().join("mydir")).unwrap();

    // Re-claim: the stale record is dropped and the worktree lands at the
    // default location instead.
    planr_ok(td.path(), &["claim", "t1"]);

    std::fs::create_dir_all(td.path().join("mydir")).unwrap();
    std::fs::write(td.path().join("mydir/real.txt"), "mine").unwrap();
    assert!(
        !git_ignored(td.path(), "mydir/real.txt"),
        "the old rule must not outlive its worktree: exclude={:?}",
        read_exclude(td.path())
    );
    let status = git_stdout(td.path(), &["status", "--porcelain"]);
    assert!(
        status.contains("mydir"),
        "mydir must be visible to git again: {status:?}"
    );
}

/// `abandon` refuses until the branch and worktree are cleaned up by hand, so
/// it never learns the worktree path and cannot remove that rule by name --
/// and `close`, which normally removes it, never runs for an abandoned task.
/// The rule outlived everything that referred to it and went on hiding
/// whatever was created at that path, with nothing in `git status` to say so.
/// A rule a live worktree still justifies must survive the same pass.
#[test]
fn test_e2e_abandon_prunes_the_stale_ignore_rule() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());
    planr_ok(td.path(), &["new", "task", "t2", "Task Two", "s1"]);
    git_must(td.path(), &["add", ".plan"]);
    git_must(td.path(), &["commit", "-m", "add t2"]);

    planr_ok(td.path(), &["claim", "t1", "--worktree", "wt-t1"]);
    planr_ok(td.path(), &["claim", "t2", "--worktree", "wt-t2"]);
    let exclude = read_exclude(td.path());
    assert!(
        exclude.contains("/wt-t1/") && exclude.contains("/wt-t2/"),
        "both claims should have written a rule: {exclude:?}"
    );

    // What abandon demands before it will run: the branch and the worktree
    // gone. Doing that by hand is the only way, and it is what loses the path.
    git_must(td.path(), &["worktree", "remove", "--force", "wt-t1"]);
    git_must(td.path(), &["branch", "-D", "plan/t1"]);

    planr_ok(td.path(), &["abandon", "task", "t1", "wont-do -- OBE"]);

    let exclude = read_exclude(td.path());
    assert!(
        !exclude.contains("/wt-t1/"),
        "the rule for the gone worktree must be pruned: {exclude:?}"
    );
    assert!(
        exclude.contains("/wt-t2/"),
        "a rule a live worktree still needs must survive: {exclude:?}"
    );

    // The pruned path is visible to git again -- which is the whole point.
    std::fs::create_dir_all(td.path().join("wt-t1")).unwrap();
    std::fs::write(td.path().join("wt-t1/new.txt"), "new\n").unwrap();
    let status = git_stdout(td.path(), &["status", "--porcelain"]);
    assert!(
        status.contains("wt-t1"),
        "the old path must no longer be hidden: {status:?}"
    );
    // The surviving worktree is still hidden, so it cannot be staged as a
    // gitlink.
    assert!(
        git_ignored(td.path(), "wt-t2"),
        "the live worktree must stay ignored: {exclude:?}"
    );
    assert!(
        !status.contains("wt-t2"),
        "the live worktree must never show up as a gitlink: {status:?}"
    );
}

// ---------------------------------------------------------------------------
// Scenario: rewriting the exclude file without destroying it
// ---------------------------------------------------------------------------

/// planr rewrites `.git/info/exclude` whole, from a split of what it read --
/// so what it could not read, it destroyed. The file is a list of paths, and
/// on Unix a path is bytes: one Latin-1 byte in a `/caf\xe9/` rule made the
/// text read fail, `unwrap_or_default` turned that into "the file is empty",
/// and `claim` wrote back planr's block alone. Exit 0, no warning, every
/// other rule gone -- and the file is untracked, so git cannot put it back.
///
/// Both writers are checked here, not just the one the report reached
/// through. `close` rewrites the same file from the same split, so a claim
/// that preserved the file and a close that did not would leave the user
/// exactly where they started, one command later.
#[cfg(unix)]
#[test]
fn test_e2e_a_non_utf8_exclude_file_survives_claim_and_close() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());
    // A rule naming a directory whose name is not UTF-8, which is an ordinary
    // thing to find in a repository older than the encoding was settled.
    let users: &[u8] = b"/build/\n/caf\xe9/\n/secrets.txt\n";
    std::fs::write(td.path().join(".git/info/exclude"), users).unwrap();
    // Something for the rules to bite on, so the test can ask git whether
    // they still work rather than only whether the bytes are still there.
    std::fs::create_dir_all(td.path().join("build")).unwrap();
    std::fs::write(td.path().join("secrets.txt"), "hunter2\n").unwrap();

    let wt = td
        .path()
        .join(planr_ok(td.path(), &["claim", "t1", "--worktree", "wt-t1"]));
    let after = read_exclude_bytes(td.path());
    assert!(
        after.starts_with(users),
        "claim must give back every rule it did not write: {after:?}"
    );
    assert!(
        after.windows(8).any(|w| w == b"/wt-t1/\n"),
        "and it still has to write its own: {after:?}"
    );
    // Not just present in the file -- still doing their job.
    assert!(
        git_ignored(td.path(), "build"),
        "the user's rules must still be the rules git evaluates"
    );
    assert!(git_ignored(td.path(), "wt-t1"), "and so must planr's");

    approve_in_worktree(&wt, td.path(), "t1");
    planr_ok(td.path(), &["close", "task", "t1"]);

    let after = read_exclude_bytes(td.path());
    assert_eq!(
        after.trim_ascii_end(),
        users.trim_ascii_end(),
        "close must take back its own rule and nothing else: {after:?}"
    );
    assert!(
        git_ignored(td.path(), "build"),
        "the user's rules outlive the whole lifecycle"
    );
}

/// planr owns the lines inside its own header block and nothing else. The
/// blank lines above the header and whatever follows the block are the user's
/// bytes, and `close` hands them back exactly as it found them -- it does not
/// normalize the file on its way past.
///
/// The two shapes pinned here are the ones a tidier would change: two blank
/// lines above the header, which planr does not collapse to one, and a comment
/// directly after the block, which planr does not push down with a blank line
/// of its own. Both are outside the block, so both are the user's to decide.
///
/// This is the arbiter for folding `exclude_add`'s separator handling into
/// `write_planr_block`. `exclude_add` does trim and pad around its block,
/// because it is placing a header the file may not have had; teaching the
/// shared writer to do the same would make every `close` rewrite these bytes
/// too. If that change is ever attempted, this test is what says whether it
/// is visible to a user. Do not relax it to let such a change through.
#[test]
fn test_e2e_close_leaves_the_bytes_outside_planrs_block_alone() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());
    planr_ok(td.path(), &["new", "task", "t2", "Task Two", "s1"]);
    git_must(td.path(), &["add", ".plan"]);
    git_must(td.path(), &["commit", "-m", "add t2"]);

    let wt1 = td
        .path()
        .join(planr_ok(td.path(), &["claim", "t1", "--worktree", "wt-t1"]));
    planr_ok(td.path(), &["claim", "t2", "--worktree", "wt-t2"]);

    // The file as a user who has edited it by hand would leave it: their own
    // rule, two blank lines, planr's block, then a comment of theirs with no
    // blank line between. Written whole so the exact bytes are the fixture.
    let seeded = "/build/\n\n\n\
         # planr worktrees -- checkouts, not backlog content\n\
         /wt-t1/\n/wt-t2/\n\
         # my own rules\n/logs/\n";
    std::fs::write(td.path().join(".git/info/exclude"), seeded).unwrap();

    approve_in_worktree(&wt1, td.path(), "t1");
    planr_ok(td.path(), &["close", "task", "t1"]);

    // t2 is still claimed, so planr's block keeps its rule and the header
    // stays -- the case where `before` and `after` are written back untouched.
    let expected = "/build/\n\n\n\
         # planr worktrees -- checkouts, not backlog content\n\
         /wt-t2/\n\
         # my own rules\n/logs/\n";
    assert_eq!(
        read_exclude(td.path()),
        expected,
        "close must take out its own rule and leave every other byte where it was"
    );
}
