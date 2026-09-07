//! Scenarios for `planr close`: the child gates on a story and an epic,
//! and the cleanup after a task merges.

use crate::common::*;
use assert_cmd::Command;

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

/// A child ticket planr cannot read is not a child that is done.
///
/// `find_children_on_trunk` swallows nothing now, but it used to: a failed
/// `ls-tree` came back as an empty listing and a child whose blob would not
/// `show` was skipped. Both gates that call it read a short list the way
/// they read a complete one -- no unfinished children, so close the parent.
/// A story whose only task is unreadable closed with that task still `todo`,
/// and the epic above it followed. The gate fails closed instead: the read
/// failure is the answer, not a clean bill of health for tickets nothing
/// opened.
#[test]
fn test_e2e_close_refuses_a_parent_whose_child_it_cannot_read() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());
    let task_file = format!(".plan/tasks/{}", find_task_slug(td.path(), "t1"));
    let story_file = format!(
        ".plan/stories/{}",
        find_ticket_filename(td.path(), "stories", "s1")
    );
    let epic_file = format!(
        ".plan/epics/{}",
        find_ticket_filename(td.path(), "epics", "e1")
    );

    // The tree still names the task, so the story's gate finds a child and
    // cannot open it.
    destroy_blob(td.path(), "main", &task_file);
    let err = planr_err(td.path(), &["close", "story", "s1"]);
    assert!(
        !err.contains("unfinished"),
        "a child that will not read has no status to report: {err}"
    );
    let story = git_stdout(td.path(), &["show", &format!("main:{story_file}")]);
    assert!(
        story.contains("status: todo"),
        "the story must not close over a task planr could not read: {story}"
    );

    // Same shape one level up: the epic's only story is now unreadable too.
    destroy_blob(td.path(), "main", &story_file);
    let err = planr_err(td.path(), &["close", "epic", "e1"]);
    assert!(
        !err.contains("unfinished"),
        "a child that will not read has no status to report: {err}"
    );
    let epic = git_stdout(td.path(), &["show", &format!("main:{epic_file}")]);
    assert!(
        epic.contains("status: todo"),
        "the epic must not close over a story planr could not read: {epic}"
    );
}

/// A locked worktree survives `close`'s removal, so the branch delete after
/// it fails too. `close` must say so rather than printing unqualified success
/// while `board` keeps listing the task in flight.
///
/// Note this is the *reachable* half of that failure. A locked worktree whose
/// directory was also deleted cannot get this far: `close` writes the done
/// flip into the worktree before merging, so it aborts there. The stale-record
/// guard in `close_cmd.rs` is defensive for that reason -- it now requires the
/// worktree record to be gone as well as the directory, so it cannot drop a
/// live worktree's rule if some future path does reach it.
#[test]
fn test_e2e_close_warns_when_a_locked_worktree_survives() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());

    planr_ok(td.path(), &["claim", "t1", "--worktree", "wt-locked"]);
    let wt = td.path().join("wt-locked");
    let task_file = format!(".plan/tasks/{}", find_task_slug(td.path(), "t1"));
    let content = std::fs::read_to_string(wt.join(&task_file)).unwrap();
    let reviewed = content.replace("status: in_progress", "status: review")
        + "\n\n## Review\n\nverdict: approved\nreviewer: test\ndate: 2026-09-05\n";
    std::fs::write(wt.join(&task_file), reviewed).unwrap();
    git_must(&wt, &["add", &task_file]);
    git_must(&wt, &["commit", "-m", "review: t1"]);

    // Locked, but still present: the merge succeeds and the removal refuses.
    git_must(td.path(), &["worktree", "lock", wt.to_str().unwrap()]);

    let out = planr(td.path(), &["close", "task", "t1"]);
    assert!(out.status.success(), "close should still report the merge");
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(
        stderr.contains("could not be removed"),
        "a surviving worktree must be reported: stderr={stderr:?}"
    );
    // Its rule stays while it does, so trunk cannot pick it up as a gitlink.
    assert!(
        git_ignored(td.path(), "wt-locked"),
        "a surviving worktree must stay hidden: exclude={:?}",
        read_exclude(td.path())
    );

    Command::new("git")
        .args(["worktree", "unlock", wt.to_str().unwrap()])
        .current_dir(td.path())
        .ok()
        .ok();
}

/// `close` must never delete a worktree that holds another one.
///
/// `git worktree remove` decides it is safe by asking `git status
/// --porcelain`, which does not list ignored paths -- and planr's own rule
/// hides `<plan-dir>/worktrees/` inside every working tree. A worker that
/// claims from inside its own worktree nests one there by default, so git's
/// safety check could not see it and deleted it recursively, uncommitted work
/// and all, while `close` reported success. Without the ignore rule git
/// refuses the removal; the rule is what made this silent.
#[test]
fn test_e2e_close_does_not_delete_a_nested_worktree() {
    let td = tempfile::tempdir().unwrap();
    seed_lint_repo(td.path());
    planr_ok(td.path(), &["new", "task", "t2", "Task Two", "s1"]);
    git_must(td.path(), &["add", ".plan"]);
    git_must(td.path(), &["commit", "-m", "add t2"]);

    planr_ok(td.path(), &["claim", "t1"]);
    let wt1 = td.path().join(".plan/worktrees/wt-t1");
    // The worker claims its next task from inside its own worktree, which is
    // where the default path nests one.
    planr_ok(&wt1, &["claim", "t2"]);
    let nested = wt1.join(".plan/worktrees/wt-t2");
    assert!(nested.is_dir(), "nested worktree should exist");
    std::fs::write(nested.join("PRECIOUS.txt"), "uncommitted work").unwrap();

    // Take t1 to review and close it.
    let task_file = format!(".plan/tasks/{}", find_task_slug(td.path(), "t1"));
    let content = std::fs::read_to_string(wt1.join(&task_file)).unwrap();
    let reviewed = content.replace("status: in_progress", "status: review")
        + "\n\n## Review\n\nverdict: approved\nreviewer: test\ndate: 2026-09-05\n";
    std::fs::write(wt1.join(&task_file), reviewed).unwrap();
    git_must(&wt1, &["add", &task_file]);
    git_must(&wt1, &["commit", "-m", "review: t1"]);

    let out = planr(td.path(), &["close", "task", "t1"]);
    assert!(out.status.success(), "close should still merge");

    assert!(
        nested.join("PRECIOUS.txt").is_file(),
        "closing the parent must not destroy the nested worktree's work"
    );
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(
        stderr.contains("live worktree"),
        "the operator must be told why the worktree was left: {stderr:?}"
    );
}

// ---------------------------------------------------------------------------
// Scenario: closing from inside the worktree being removed
// ---------------------------------------------------------------------------

/// `close` ran the rest of its cleanup from inside the directory it had just
/// deleted. A worker closes their own task from their own worktree -- it is
/// the ordinary way to run the command -- and the process was then standing
/// in a path that no longer resolved, so every git run after the removal
/// failed at `chdir`, before git was even reached.
///
/// Two victims, neither of which named the cause. The ignore rule for the
/// worktree could not be dropped, so a rule hiding a path that no longer
/// existed outlived it, silently hiding whatever was created there next. And
/// the "all tasks under this story are done" hint was computed from a failed
/// listing and dropped, so the close that finished a story said nothing
/// about it.
///
/// Every way of standing inside is covered, because it is the standing that
/// matters and not the exact directory: the worktree's own root, a
/// subdirectory of it, and the default `.plan/worktrees/` path -- where the
/// shared rule is meant to survive, so only the hint shows the damage.
#[test]
fn test_e2e_close_from_inside_the_worktree_finishes_its_cleanup() {
    for (case, flags, subdir, gone, kept) in [
        (
            "the worktree's own root",
            vec!["claim", "t1", "--worktree", "wt-t1"],
            "",
            Some("/wt-t1/"),
            None,
        ),
        (
            "a subdirectory of the worktree",
            vec!["claim", "t1", "--worktree", "wt-t1"],
            "deep/deeper",
            Some("/wt-t1/"),
            None,
        ),
        (
            "the default worktree path",
            vec!["claim", "t1"],
            "",
            None,
            Some("/.plan/worktrees/"),
        ),
    ] {
        let td = tempfile::tempdir().unwrap();
        seed_lint_repo(td.path());
        let wt = td.path().join(planr_ok(td.path(), &flags));
        approve_in_worktree(&wt, td.path(), "t1");

        let from = if subdir.is_empty() {
            wt.clone()
        } else {
            let deep = wt.join(subdir);
            std::fs::create_dir_all(&deep).unwrap();
            deep
        };
        let (out, err) = planr_ok_both(&from, &["close", "task", "t1"]);

        assert!(
            out.contains("merged plan/t1"),
            "[{case}] the close itself must still land: {out}"
        );
        // t1 is the only task under s1, so closing it finishes the story --
        // and the caller has to be told, whichever directory they ran from.
        assert!(
            out.contains("all tasks under story 's1' are done"),
            "[{case}] the sibling hint was computed from a failed listing \
             and dropped: stdout={out:?} stderr={err:?}"
        );

        let excl = read_exclude(td.path());
        if let Some(gone) = gone {
            assert!(
                !excl.lines().any(|l| l.trim() == gone),
                "[{case}] the rule for the removed worktree survived it: {excl:?}"
            );
            // And the path is visible to git again, which is what the rule
            // being gone is for.
            std::fs::create_dir_all(td.path().join("wt-t1")).unwrap();
            std::fs::write(td.path().join("wt-t1/new.txt"), "new\n").unwrap();
            assert!(
                !git_ignored(td.path(), "wt-t1"),
                "[{case}] the old path must no longer be hidden: {excl:?}"
            );
        }
        if let Some(kept) = kept {
            assert!(
                excl.lines().any(|l| l.trim() == kept),
                "[{case}] the shared default rule covers a parent and must \
                 survive: {excl:?}"
            );
        }
    }
}
