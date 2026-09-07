//! Scenarios for `planr claim`: the worktree it builds, the states it
//! refuses, and what it leaves behind when a step of it fails.

use crate::common::*;
use assert_cmd::Command;
use std::path::Path;

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
    git_must(&wt_abs, &["add", &task_file]);
    git_must(&wt_abs, &["commit", "-m", "review: t1"]);

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
    git_must(
        td.path(),
        &["worktree", "remove", ".plan/worktrees/wt-t1", "--force"],
    );

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
    git_must(&wt, &["add", &task_file]);
    git_must(&wt, &["commit", "-m", "review: t1"]);

    // Drop the worktree the supported way, then re-claim it.
    git_must(
        td.path(),
        &["worktree", "remove", "--force", wt.to_str().unwrap()],
    );
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
    git_must(&wt, &["add", &task_file]);
    git_must(&wt, &["commit", "-m", "abandon on branch"]);
    git_must(
        td.path(),
        &["worktree", "remove", "--force", wt.to_str().unwrap()],
    );

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
    git_must(&wt, &["add", &task_file]);
    git_must(&wt, &["commit", "-m", "done on branch"]);
    git_must(
        td.path(),
        &["worktree", "remove", "--force", wt.to_str().unwrap()],
    );

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
        git_must(td.path(), &["add", &task_file]);
        git_must(td.path(), &["commit", "-m", "advance t1"]);

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

/// `git worktree add -b <branch> <path>` creates the branch *before* it
/// validates the path, so a refused path left the branch behind. `board` then
/// listed an in-flight branch for a task nobody claimed, and `abandon` refused
/// the task for having an active branch.
#[test]
fn test_e2e_failed_worktree_add_leaves_no_branch() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());

    // A non-empty directory that `worktree add` will refuse.
    std::fs::create_dir_all(td.path().join("occupied")).unwrap();
    std::fs::write(td.path().join("occupied/keep.txt"), "mine").unwrap();

    planr_err(td.path(), &["claim", "t1", "--worktree", "occupied"]);

    let branches = git_stdout(td.path(), &["branch", "--list"]);
    assert!(
        !branches.contains("plan/t1"),
        "a failed claim must not leave its branch: {branches:?}"
    );
    let board = planr_ok(td.path(), &["board"]);
    assert!(
        !board.contains("## in flight"),
        "no in-flight branch for a task nobody claimed: {board}"
    );
}

/// The terminal guards compared bare words against a frontmatter reader that
/// did not strip YAML quotes, so `status: "done"` -- which lint accepts and
/// board renders as `done` -- slipped past and was reopened and committed.
#[test]
fn test_e2e_claim_refuses_a_quoted_terminal_status() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());

    let task_file = format!(".plan/tasks/{}", find_task_slug(td.path(), "t1"));
    let path = td.path().join(&task_file);
    let content = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, content.replace("status: todo", "status: \"done\"")).unwrap();
    git_must(td.path(), &["add", &task_file]);
    git_must(td.path(), &["commit", "-m", "quoted done"]);

    // Precondition: the rest of the tool reads this as `done`.
    let board = planr_ok(td.path(), &["board"]);
    assert!(
        board
            .lines()
            .any(|l| l.starts_with("t1 ") && l.contains("done")),
        "board should read the quoted status as done: {board}"
    );

    let err = planr_err(td.path(), &["claim", "t1"]);
    assert!(
        err.contains("done"),
        "a quoted terminal status must refuse too: {err}"
    );
    let after = std::fs::read_to_string(&path).unwrap();
    assert!(
        after.contains("\"done\""),
        "the ticket must not be rewritten: {after}"
    );
}

/// A resumed claim must not rewrite `blocked` either. The guard listed the
/// statuses it would not touch, and fell one short of the vocabulary, so a
/// worker who marked their branch blocked had it silently reopened.
#[test]
fn test_e2e_claim_resume_does_not_revert_blocked() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());

    planr_ok(td.path(), &["claim", "t1"]);
    let wt = td.path().join(".plan/worktrees/wt-t1");
    let task_file = format!(".plan/tasks/{}", find_task_slug(td.path(), "t1"));

    let content = std::fs::read_to_string(wt.join(&task_file)).unwrap();
    std::fs::write(
        wt.join(&task_file),
        content.replace("status: in_progress", "status: blocked"),
    )
    .unwrap();
    git_must(&wt, &["add", &task_file]);
    git_must(&wt, &["commit", "-m", "blocked on branch"]);
    git_must(
        td.path(),
        &["worktree", "remove", "--force", wt.to_str().unwrap()],
    );

    planr_ok(td.path(), &["claim", "t1"]);

    let after = std::fs::read_to_string(wt.join(&task_file)).unwrap();
    assert!(
        after.contains("status: blocked"),
        "resume must not reopen a blocked branch: {after}"
    );
}

/// Only a `todo` task is claimable. `blocked` on trunk means the leader has
/// said so; a claim used to create a worktree, skip the flip and exit 0.
#[test]
fn test_e2e_claim_refuses_a_blocked_task_on_trunk() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());

    let task_file = format!(".plan/tasks/{}", find_task_slug(td.path(), "t1"));
    let path = td.path().join(&task_file);
    let content = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, content.replace("status: todo", "status: blocked")).unwrap();
    git_must(td.path(), &["add", &task_file]);
    git_must(td.path(), &["commit", "-m", "block t1"]);

    let err = planr_err(td.path(), &["claim", "t1"]);
    assert!(err.contains("blocked"), "expected the reason: {err}");
    assert!(
        !td.path().join(".plan/worktrees/wt-t1").exists(),
        "a refused claim must create nothing"
    );
}

/// The refusal must name an operation planr offers. No command sets a ticket
/// back to `todo`, so "reopen the ticket" pointed at nothing.
#[test]
fn test_e2e_blocked_refusal_names_a_real_remedy() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());

    let task_file = format!(".plan/tasks/{}", find_task_slug(td.path(), "t1"));
    let path = td.path().join(&task_file);
    let content = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, content.replace("status: todo", "status: blocked")).unwrap();
    git_must(td.path(), &["add", &task_file]);
    git_must(td.path(), &["commit", "-m", "block t1"]);

    let err = planr_err(td.path(), &["claim", "t1"]);
    assert!(
        err.contains("status: todo") && err.contains("commit"),
        "the refusal should say how to unblock: {err}"
    );
}

/// A gitignore file is line-oriented, so a worktree path holding a newline
/// cannot be expressed as a rule. Writing it anyway split the pattern across
/// two lines: the worktree stayed visible and was staged as a gitlink, the
/// claim reported success, and neither fragment could ever be removed --
/// `/wt` and `evil/` then hid unrelated paths in every worktree, forever.
#[test]
#[cfg(unix)]
fn test_e2e_claim_refuses_a_worktree_path_with_a_newline() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());

    let before = read_exclude(td.path());
    let err = planr_err(td.path(), &["claim", "t1", "--worktree", "wt\nevil"]);
    assert!(
        err.contains("line break"),
        "the refusal should name the reason: {err}"
    );

    let after = read_exclude(td.path());
    assert_eq!(
        after.trim_end(),
        before.trim_end(),
        "no rule -- whole or broken -- may be written"
    );
    let status = git_stdout(td.path(), &["status", "--porcelain"]);
    assert!(
        !status.contains("evil"),
        "a refused claim must leave no worktree: {status:?}"
    );
}

/// Dropping a stale worktree record is not silent: the directory being absent
/// cannot be told apart from a volume that is merely unmounted, so the
/// operator gets the one chance to notice.
#[test]
fn test_e2e_dropping_a_stale_record_warns() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());

    planr_ok(td.path(), &["claim", "t1"]);
    std::fs::remove_dir_all(td.path().join(".plan/worktrees/wt-t1")).unwrap();

    let out = planr(td.path(), &["claim", "t1"]);
    assert!(out.status.success(), "re-claim should succeed");
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(
        stderr.contains("dropping that record") && stderr.contains("unmounted"),
        "dropping a record must be reported, with the volume caveat: {stderr:?}"
    );
}

// ---------------------------------------------------------------------------
// Scenario: a path the caller typed comes back as one place
// ---------------------------------------------------------------------------

/// The path `claim` prints is what the caller pastes into their next
/// command, and it was not the path planr had used itself: `--worktree
/// ../out` from `sub/` printed `/repo/sub/../out` while the ignore rule was
/// written for `/out`, because the rule is anchored after normalizing. It
/// opened, so nothing broke -- it just handed back a path with the caller's
/// `..` still in it and disagreed with planr's own reading of it.
///
/// Every form that carries a `.` or `..` is checked, including the absolute
/// one: typing a path out in full is no reason to be handed `..` back.
#[test]
fn test_e2e_claim_prints_the_path_it_resolved() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());
    planr_ok(td.path(), &["new", "task", "t2", "Task Two", "s1"]);
    planr_ok(td.path(), &["new", "task", "t3", "Task Three", "s1"]);
    git_must(td.path(), &["add", ".plan"]);
    git_must(td.path(), &["commit", "-m", "more tasks"]);
    let sub = td.path().join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    let abs_with_dots = format!("{}/sub/../abs", td.path().display());

    for (slug, typed, expected) in [
        ("t1", "../out".to_string(), td.path().join("out")),
        ("t2", abs_with_dots, td.path().join("abs")),
        ("t3", "./a/../b".to_string(), sub.join("b")),
    ] {
        let printed = planr_ok(&sub, &["claim", slug, "--worktree", &typed]);
        assert!(
            !Path::new(&printed)
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir)),
            "'{typed}' came back with the caller's `..` still in it: {printed}"
        );
        assert_eq!(
            Path::new(&printed),
            expected,
            "'{typed}' must print the one place planr resolved it to"
        );
        assert!(
            Path::new(&printed).is_dir(),
            "and the worktree must be there: {printed}"
        );
        // The rule planr wrote and the path it printed have to be the same
        // place: that is the disagreement the normalization removes.
        let rel = expected.strip_prefix(td.path()).unwrap();
        assert!(
            git_ignored(td.path(), rel.to_str().unwrap()),
            "the printed worktree must be the one that is hidden: {}",
            read_exclude(td.path())
        );
    }
}
