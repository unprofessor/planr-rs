//! Whose answer is it -- the commit graph's, or a committer's clock?
//!
//! Two mechanisms, one question. The **authority rule** says a claimed
//! ticket's branch answers for it until the branch is integrated, which removes
//! the ambiguity this model manufactures on every claim: cutting a branch makes
//! two lanes concurrent by construction, and a union walk had nothing but
//! `--date-order` with which to decide between them. The **integration check**
//! reports what is left -- histories where the graph genuinely declines to
//! order two declarations, and slugs whose creation record is duplicated or
//! missing.
//!
//! What these tests pin is that the answers do not move when a clock does.
//! Several of them build history with `commit-tree` and an explicit
//! `GIT_COMMITTER_DATE` rather than through verbs, deliberately: the reader
//! must be right about histories planr did not write, since the failure being
//! guarded is two clones merging.

#![cfg(feature = "next")]

use assert_cmd::Command;
use std::path::Path;

fn git(dir: &Path, args: &[&str]) {
    Command::new("git")
        .args(args)
        .current_dir(dir)
        .ok()
        .unwrap_or_else(|e| panic!("git {args:?} failed: {e}"));
}

fn git_out(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn planr(dir: &Path, args: &[&str]) -> (bool, String, String) {
    let out = Command::cargo_bin("planr")
        .unwrap()
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

fn ok(dir: &Path, args: &[&str]) -> String {
    let (success, stdout, stderr) = planr(dir, args);
    assert!(success, "planr {args:?} failed:\n{stderr}");
    stdout
}

fn setup(dir: &Path) {
    git(dir, &["init", "-b", "main", "."]);
    git(dir, &["config", "user.email", "e2e@test"]);
    git(dir, &["config", "user.name", "E2E Test"]);

    std::fs::create_dir_all(dir.join(".plan/tickets")).unwrap();
    std::fs::write(
        dir.join(".plan/schema.yml"),
        include_str!("../.plan/schema.yml"),
    )
    .unwrap();
    std::fs::write(dir.join(".plan/tickets/.gitkeep"), "").unwrap();
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-m", "seed"]);
}

/// Put a declaration on a branch with a chosen committer date, without going
/// through a verb.
///
/// `--date-order` reads the COMMITTER date, not the author date, so the
/// committer date is the one a test has to control -- setting only
/// `--date` moves the author date and changes nothing the walk sees.
fn declare(dir: &Path, branch: &str, verb: &str, slug: &str, when: &str) -> String {
    let tip = git_out(dir, &["rev-parse", branch]);
    let tree = git_out(dir, &["rev-parse", &format!("{branch}^{{tree}}")]);
    let msg = format!("plan: {verb} {slug}\n\nPlanr-Verb: {verb}\nPlanr-Ticket: {slug}\n");
    let out = Command::new("git")
        .args(["commit-tree", &tree, "-p", &tip, "-m", &msg])
        .env("GIT_COMMITTER_DATE", when)
        .env("GIT_AUTHOR_DATE", when)
        .current_dir(dir)
        .output()
        .unwrap();
    let sha = String::from_utf8_lossy(&out.stdout).trim().to_string();
    assert!(!sha.is_empty(), "commit-tree produced nothing");
    git(dir, &["update-ref", &format!("refs/heads/{branch}"), &sha]);
    sha
}

/// A claimed task whose branch has been submitted -- the ordinary two-lane
/// situation, built entirely through verbs.
fn claim_and_submit(dir: &Path, slug: &str) {
    ok(dir, &["next", "new", "task", slug, "Work"]);
    ok(dir, &["next", "do", "claim", slug, ""]);

    // The worker's own commit, in the worker's own worktree, which is where
    // `submit`'s gate reads from.
    let wt = dir.join(format!(".plan/worktrees/task/{slug}"));
    let ticket = wt.join(format!(".plan/tickets/{slug}.md"));
    let body = std::fs::read_to_string(&ticket).unwrap();
    std::fs::write(&ticket, format!("{body}\n## Validation\n\ncargo test\n")).unwrap();
    git(&wt, &["add", "-A"]);
    git(&wt, &["commit", "-m", "work"]);

    ok(dir, &["next", "do", "submit", slug, ""]);
}

fn state(dir: &Path, slug: &str) -> String {
    let out = ok(dir, &["next", "state", slug]);
    out.lines()
        .next()
        .unwrap()
        .split(": ")
        .nth(1)
        .unwrap()
        .to_string()
}

fn board_row(dir: &Path, slug: &str) -> String {
    let out = ok(dir, &["next", "board"]);
    out.lines()
        .find(|l| l.split_whitespace().next() == Some(slug))
        .unwrap_or_else(|| panic!("no board row for '{slug}' in:\n{out}"))
        .to_string()
}

fn check(dir: &Path) -> (bool, String) {
    let (success, stdout, _) = planr(dir, &["next", "check"]);
    (success, stdout)
}

// ---------------------------------------------------------------------------
// The authority rule
// ---------------------------------------------------------------------------

/// The case the whole slice exists for.
///
/// A worker submits on the branch; the leader declares something else on trunk
/// afterwards. The two commits are neither ancestor nor descendant, so the
/// commit graph has no opinion -- and the union walk that used to answer this
/// resolved it with `--date-order`, which means the ticket's state was decided
/// by whichever machine's clock read later. Here trunk's declaration is dated
/// a year ahead, which is what a skewed clock looks like: under the old reader
/// it wins outright.
#[test]
fn a_live_branch_answers_for_its_ticket_however_the_clocks_read() {
    for (label, when) in [
        ("trunk dated later", "2030-01-01T00:00:00Z"),
        ("trunk dated earlier", "2000-01-01T00:00:00Z"),
    ] {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        setup(dir);
        claim_and_submit(dir, "foo");

        // A trunk-lane declaration that would fold to `todo`.
        declare(dir, "main", "yield", "foo", when);

        assert_eq!(
            state(dir, "foo"),
            "review",
            "{label}: the branch is authoritative until it integrates, so the trunk lane must \
             not decide the state -- and least of all by its clock"
        );
        assert!(
            board_row(dir, "foo").contains("review"),
            "{label}: board must fold the same events `state` does, or a `from` gate accepts a \
             ticket the board says is elsewhere"
        );
    }
}

/// The authority rule is a claim about the *unintegrated*, not about the
/// merely present.
///
/// A branch left standing after its commits reached trunk carries nothing
/// trunk does not have. Keeping it authoritative would hide trunk-lane
/// declarations forever rather than until integration, which is the difference
/// between deferring a decision and losing it.
#[test]
fn a_branch_trunk_has_already_taken_stops_answering() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup(dir);
    claim_and_submit(dir, "foo");

    // Integrate the branch by hand and leave the ref in place -- the state
    // `close` reaches through, minus the ref deletion.
    git(
        dir,
        &["merge", "--no-ff", "-m", "integrate", "plan/task/foo"],
    );
    assert_eq!(state(dir, "foo"), "review");

    declare(dir, "main", "yield", "foo", "2030-01-01T00:00:00Z");
    assert_eq!(
        state(dir, "foo"),
        "todo",
        "once trunk has taken the branch's commits, trunk answers -- a stale ref must not \
         shadow the integration lane permanently"
    );
}

/// The walk has to respect the graph before the bound means anything.
///
/// `git log` with no ordering flag walks a queue ordered by committer date
/// alone, and a merge is where that stops agreeing with ancestry: the merge
/// puts both parents on the frontier at once, so a back-dated declaration loses
/// its place to the very commit it descends from. Here `new` carries today's
/// date, `abandon` descends from it and is dated 2000 -- an imported history, a
/// graft, or a machine whose clock is wrong -- and the merge makes them
/// siblings in the queue.
///
/// The backwards scan then meets `new` first, floors on it, and reports an
/// abandoned ticket as `todo`. The bound is a theorem about a sequence that
/// respects ancestry, and this is what it costs when the sequence does not.
#[test]
fn a_declaration_dated_before_its_own_parent_still_folds_after_it() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup(dir);
    ok(dir, &["next", "new", "task", "foo", "Work"]);

    git(dir, &["branch", "lane"]);
    declare(dir, "lane", "abandon", "foo", "2000-01-01T00:00:00Z");
    git(dir, &["merge", "--no-ff", "-m", "integrate", "lane"]);

    assert_eq!(
        state(dir, "foo"),
        "abandoned",
        "a child is a child however its clock reads -- floor on the creation record here and \
         every ticket whose history was imported reports its initial state"
    );
}

// ---------------------------------------------------------------------------
// The integration check
// ---------------------------------------------------------------------------

#[test]
fn a_backlog_whose_history_is_sound_reports_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup(dir);
    ok(dir, &["next", "new", "epic", "big", "Big"]);
    ok(dir, &["next", "new", "task", "one", "One"]);
    claim_and_submit(dir, "two");
    ok(dir, &["next", "do", "approve", "two", "looks right"]);
    ok(dir, &["next", "do", "close", "two", ""]);
    ok(dir, &["next", "do", "abandon", "one", "not doing it"]);
    ok(dir, &["next", "do", "archive", "one", ""]);

    let (success, report) = check(dir);
    assert!(success, "a sound backlog must exit zero:\n{report}");
    assert!(report.contains("ok:"), "{report}");
}

/// Two lineages, one slug -- the failure no check at creation time can catch.
///
/// Each clone consults a history the other does not exist in, so both `new`
/// commands are correct. The merge is *clean*, because archival deleted the
/// file on one side and a deletion and an addition do not conflict. Two
/// `Planr-Verb: new` records then coexist, permanently non-ancestral, and a
/// bounded walk floors at whichever one it meets first.
#[test]
fn two_creations_of_one_slug_are_reported_after_they_merge() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup(dir);
    let seed = git_out(dir, &["rev-parse", "HEAD"]);

    // Lineage one: create, retire, archive. The file is gone; the events stay.
    ok(dir, &["next", "new", "task", "foo", "First"]);
    ok(dir, &["next", "do", "abandon", "foo", "no"]);
    ok(dir, &["next", "do", "archive", "foo", ""]);

    // Lineage two, cut from before the slug existed, so its reservation
    // legitimately finds the name free.
    git(dir, &["branch", "other", &seed]);
    ok(
        dir,
        &["--trunk", "other", "next", "new", "task", "foo", "Second"],
    );

    git(dir, &["merge", "--no-ff", "-m", "integrate", "other"]);

    let (success, report) = check(dir);
    assert!(!success, "a duplicated creation is a fault:\n{report}");
    assert!(
        report.contains("duplicate-genesis foo"),
        "the finding must name the slug and the kind of breakage:\n{report}"
    );
    assert!(
        report.contains("2 'new' records"),
        "and must count them, since 'at most one' is the rule that walks past the other \
         half of the invariant:\n{report}"
    );
}

/// The mirror image: events with no reachable creation at all.
///
/// A `>= 2` rule walks straight past this one, which is why the invariant is
/// stated as *exactly* one. The fold still answers for these events, so the
/// slug is neither free nor explicable.
#[test]
fn events_with_no_reachable_creation_are_reported() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup(dir);
    ok(dir, &["next", "new", "task", "foo", "Work"]);
    ok(dir, &["next", "do", "abandon", "foo", "no"]);

    // Graft the abandon commit onto the root, cutting the creation record out
    // of reachability while every later trailer survives.
    let root = git_out(dir, &["rev-list", "--max-parents=0", "HEAD"]);
    let abandon = git_out(dir, &["rev-parse", "HEAD"]);
    git(dir, &["replace", "--graft", &abandon, &root]);

    let (success, report) = check(dir);
    assert!(!success, "a severed lineage is a fault:\n{report}");
    assert!(report.contains("severed foo"), "{report}");
}

/// Two state-changing declarations the commit graph declines to order.
///
/// This is the residue the authority rule cannot remove: both lanes reached
/// trunk, neither descends from the other, and the fold picks by committer
/// date. The test proves the finding is real rather than pedantic -- the same
/// repository, built twice with the two dates swapped, folds to two different
/// states.
#[test]
fn declarations_the_graph_cannot_order_are_reported_and_really_do_flip() {
    let mut states = Vec::new();
    for (early, late) in [
        ("2020-01-01T00:00:00Z", "2021-01-01T00:00:00Z"),
        ("2021-01-01T00:00:00Z", "2020-01-01T00:00:00Z"),
    ] {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        setup(dir);
        ok(dir, &["next", "new", "task", "foo", "Work"]);

        git(dir, &["branch", "lane-a"]);
        git(dir, &["branch", "lane-b"]);
        declare(dir, "lane-a", "abandon", "foo", early);
        declare(dir, "lane-b", "yield", "foo", late);
        git(dir, &["merge", "--no-ff", "-m", "take a", "lane-a"]);
        git(dir, &["merge", "--no-ff", "-m", "take b", "lane-b"]);

        let (success, report) = check(dir);
        assert!(!success, "an unorderable pair is a fault:\n{report}");
        assert!(report.contains("divergent foo"), "{report}");

        states.push(state(dir, "foo"));
    }
    assert_ne!(
        states[0], states[1],
        "the finding claims the state is decided by a clock; if swapping the two committer \
         dates did not change it, the check would be reporting a hazard that is not there"
    );
}

/// A shadowed declaration is the authority rule working, not a fault.
///
/// It is reported because the design promises a two-lane clash reaches a human
/// -- a leader abandoning while a worker submits is two people disagreeing
/// about a ticket's fate, and no tool should pick a winner quietly. It exits
/// zero because failing here would fail on every claimed ticket whose leader
/// touched trunk, which is the workflow, not a defect.
#[test]
fn a_declaration_a_live_branch_shadows_is_reported_without_failing() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup(dir);
    claim_and_submit(dir, "foo");
    declare(dir, "main", "yield", "foo", "2030-01-01T00:00:00Z");

    let (success, report) = check(dir);
    assert!(
        success,
        "the authority rule deciding is not a broken repository:\n{report}"
    );
    assert!(report.contains("shadowed foo"), "{report}");
    assert!(
        report.contains("0 of them faults"),
        "the report has to separate 'needs a human' from 'is broken':\n{report}"
    );
}

/// Absence is not proof in a truncated history, and three of the four findings
/// are absence claims. A shallow clone answers "no such record" exactly the way
/// a complete one does.
#[test]
fn a_shallow_clone_refuses_to_certify_what_it_cannot_see() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup(dir);
    ok(dir, &["next", "new", "task", "foo", "Work"]);
    ok(dir, &["next", "do", "abandon", "foo", "no"]);

    let shallow = tmp.path().parent().unwrap().join("shallow-check");
    git(
        dir,
        &[
            "clone",
            "--depth",
            "1",
            "--no-local",
            &format!("file://{}", dir.display()),
            shallow.to_str().unwrap(),
        ],
    );

    let (success, _, stderr) = planr(&shallow, &["next", "check"]);
    assert!(!success, "a shallow clone must not report a clean bill");
    assert!(stderr.contains("shallow"), "{stderr}");
    let _ = std::fs::remove_dir_all(&shallow);
}
