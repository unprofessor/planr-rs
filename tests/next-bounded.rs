//! The bounded state read, checked two ways.
//!
//! `events::for_ticket` stops the backwards walk at the newest event carrying
//! a `to`, because absorption (`docs/semantics.md` section 4) says everything
//! earlier is annihilated. Two things have to be true, and each needs its own
//! kind of test:
//!
//!   * **the answer is unchanged.** Every scenario here folds the ticket twice
//!     -- bounded, and through the unbounded oracle `PLANR_NEXT_ORACLE`
//!     reaches -- and asserts the two agree. A suite that only exercised the
//!     new path would agree with itself.
//!   * **the bound actually binds.** Agreement says nothing about cost, so the
//!     counting tests below read `commit(s) scanned` back out of the command
//!     and pin how it moves as history grows.
//!
//! **What the differential does NOT cover, stated so it is not mistaken for
//! more.** Both walks consume the same `git log` stream in the same order,
//! sharing the ref set, the format string and the record parser; only the read
//! path and the stop rule differ. So this catches framing bugs and stop-rule
//! bugs, and by construction cannot catch a wrong format, a wrong ref set, a
//! wrong ordering flag, or a parser bug -- the two agree on the same wrong
//! answer. The live instance of that is committer-date skew between two
//! declarations that are neither ancestor nor descendant: it changes the
//! folded state and no differential test can see it. That needs a
//! graph-ordered oracle, which is a different mechanism.

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

fn refused(dir: &Path, args: &[&str]) -> String {
    let (success, stdout, stderr) = planr(dir, args);
    assert!(
        !success,
        "planr {args:?} should have been refused but succeeded:\n{stdout}"
    );
    stderr
}

/// The unbounded oracle. Not a CLI mode -- an env-gated seam, because the
/// oracle needs a real repository and the crate has no library target.
fn oracle(dir: &Path, slug: &str) -> String {
    let out = Command::cargo_bin("planr")
        .unwrap()
        .args(["next", "state", slug])
        .env("PLANR_NEXT_ORACLE", "1")
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "the oracle failed for '{slug}':\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).to_string()
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

/// Ordinary commits carrying no trailer -- the history a state read must not
/// pay for. Added by name rather than `-A`, so a ticket worktree standing
/// under `.plan/worktrees` is not swept in as an embedded repository.
fn noise(dir: &Path, tag: &str, n: usize) {
    for i in 0..n {
        let file = format!("{tag}-{i}.txt");
        std::fs::write(dir.join(&file), "x\n").unwrap();
        git(dir, &["add", &file]);
        git(dir, &["commit", "-q", "-m", &format!("work {tag} {i}")]);
    }
}

fn state_line(out: &str) -> String {
    out.lines().next().unwrap_or_default().trim().to_string()
}

/// `  N commit(s) scanned, M event(s) folded -- ...`
fn scanned(out: &str) -> usize {
    let line = out
        .lines()
        .nth(1)
        .unwrap_or_else(|| panic!("no cost line in:\n{out}"));
    line.split_whitespace()
        .next()
        .and_then(|n| n.parse().ok())
        .unwrap_or_else(|| panic!("cannot read commits scanned from: {line}"))
}

/// Fold the ticket bounded and unbounded, and require the same state from
/// both. Returns the state, so a scenario reads as a walkthrough and every
/// step is differentially checked on the way past. See the module header for
/// the limits of what this can detect.
fn agree(dir: &Path, slug: &str) -> String {
    let bounded = ok(dir, &["next", "state", slug]);
    let whole = oracle(dir, slug);
    assert_eq!(
        state_line(&bounded),
        state_line(&whole),
        "bounded and unbounded folds disagree for '{slug}'\n bounded:\n{bounded}\n unbounded:\n{whole}"
    );
    assert!(
        scanned(&bounded) <= scanned(&whole),
        "the bounded walk read MORE commits than the unbounded one for '{slug}':\n{bounded}\n{whole}"
    );
    state_line(&bounded)
}

/// Drive a task to `done`, doing real work in its worktree on the way.
fn finish_task(dir: &Path, slug: &str) {
    ok(dir, &["next", "do", "claim", slug]);
    let wt = dir.join(format!(".plan/worktrees/task/{slug}"));
    let ticket = wt.join(format!(".plan/tickets/{slug}.md"));
    let body = std::fs::read_to_string(&ticket).unwrap();
    std::fs::write(&ticket, format!("{body}\n## Validation\n\nchecked\n")).unwrap();
    git(&wt, &["add", "-A"]);
    git(&wt, &["commit", "-m", "work"]);
    ok(dir, &["next", "do", "submit", slug]);
    ok(dir, &["next", "do", "approve", slug, "looks right"]);
    ok(dir, &["next", "do", "close", slug]);
}

#[test]
fn bounded_and_unbounded_agree_across_a_whole_lifecycle() {
    // One repository carrying every shape the stop rule has to survive: a
    // ticket that never moves, a rework cycle back to the initial state,
    // declarations on trunk and on a branch interleaved, unrelated commits,
    // other tickets' events, and an archived ticket whose file is gone.
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup(dir);

    ok(dir, &["next", "new", "epic", "platform", "Platform"]);
    noise(dir, "pre", 5);

    // Never transitioned: only the creation commit can stop this walk.
    assert_eq!(agree(dir, "platform"), "platform: todo");

    ok(dir, &["next", "new", "task", "parser", "Parser"]);
    ok(dir, &["next", "new", "task", "loader", "Loader"]);
    assert_eq!(agree(dir, "parser"), "parser: todo");

    // Claimed: the declaration is on the BRANCH while trunk keeps moving.
    ok(dir, &["next", "do", "claim", "parser"]);
    noise(dir, "mid", 4);
    assert_eq!(agree(dir, "parser"), "parser: in_progress");

    // An integration-lane verb on trunk, newer than the branch declaration and
    // carrying no `to`. It must not stop the walk, and it must not be mistaken
    // for the answer.
    ok(dir, &["next", "do", "add-dep", "parser", "loader"]);
    assert_eq!(agree(dir, "parser"), "parser: in_progress");

    // Another ticket's whole lifecycle, including a `close` that reaches
    // trunk. Its `to: done` must not terminate parser's walk.
    finish_task(dir, "loader");
    assert_eq!(agree(dir, "loader"), "loader: done");
    assert_eq!(agree(dir, "parser"), "parser: in_progress");

    // Rework: back to the state the ticket started in. The fold cannot tell
    // "never moved" from "moved back" by the state alone, so the walk must
    // stop at the `yield` rather than run on to creation.
    ok(dir, &["next", "do", "yield", "parser", "blocked on loader"]);
    assert_eq!(agree(dir, "parser"), "parser: todo");

    ok(dir, &["next", "do", "resume", "parser"]);
    assert_eq!(agree(dir, "parser"), "parser: in_progress");

    let wt = dir.join(".plan/worktrees/task/parser");
    let ticket = wt.join(".plan/tickets/parser.md");
    let body = std::fs::read_to_string(&ticket).unwrap();
    std::fs::write(&ticket, format!("{body}\n## Validation\n\nchecked\n")).unwrap();
    git(&wt, &["add", "-A"]);
    git(&wt, &["commit", "-m", "work"]);

    ok(dir, &["next", "do", "submit", "parser"]);
    assert_eq!(agree(dir, "parser"), "parser: review");
    ok(dir, &["next", "do", "request-changes", "parser", "again"]);
    assert_eq!(agree(dir, "parser"), "parser: in_progress");
    ok(dir, &["next", "do", "submit", "parser"]);
    ok(dir, &["next", "do", "approve", "parser", "fine"]);
    assert_eq!(agree(dir, "parser"), "parser: approved");
    ok(dir, &["next", "do", "close", "parser"]);
    assert_eq!(agree(dir, "parser"), "parser: done");

    // Archived: the file is gone from every tree, so the fold has only
    // trailers left -- and `archive` carries no `to`, so the walk has to run
    // past it to the `close` underneath.
    ok(dir, &["next", "do", "archive", "loader", ""]);
    noise(dir, "post", 3);
    assert_eq!(agree(dir, "loader"), "loader: done");

    // ...and the slug stays taken. Re-creating it is what makes the archived
    // ticket's events attach to a new one; see the dedicated test below.
    refused(dir, &["next", "new", "task", "loader", "Loader again"]);

    // The container, still never having moved, after all of that history.
    assert_eq!(agree(dir, "platform"), "platform: todo");
    ok(dir, &["next", "do", "abandon", "platform", "superseded"]);
    assert_eq!(agree(dir, "platform"), "platform: abandoned");
}

#[test]
fn a_verb_outside_this_kinds_machine_does_not_terminate_the_walk() {
    // `fold_state` skips a verb `schema.verb(name, kind)` does not resolve, so
    // the stop rule must skip exactly the same ones. `claim` is a task verb
    // with `to: in_progress`; on an EPIC it resolves to nothing and denotes
    // `id`. A stop rule that looked the verb up without its kind would halt
    // here and report the epic as never having closed.
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup(dir);

    ok(dir, &["next", "new", "epic", "platform", "Platform"]);
    ok(dir, &["next", "do", "close", "platform"]);

    // A hand-written declaration -- a schema that once gave epics a `claim`,
    // or a migration, would leave exactly this.
    git(
        dir,
        &[
            "commit",
            "--allow-empty",
            "-m",
            "plan: claim platform",
            "-m",
            "Planr-Verb: claim\nPlanr-Ticket: platform",
        ],
    );

    assert_eq!(agree(dir, "platform"), "platform: done");
}

#[test]
fn a_slug_is_never_reused_after_archival() {
    // The history that breaks the floor, and it needed no hand-written commit:
    // archive a ticket and create the slug again, and one identifier names two
    // tickets. The bounded walk then stops at the newer `new` and the
    // unbounded one folds the dead ticket's events, so `state` said `todo`
    // while `board` said `abandoned` about the same live ticket -- and every
    // `from` gate reads the bounded answer, so a terminal ticket could be
    // re-entered.
    //
    // The defect is the identity, not the floor. The slug-to-path mapping is
    // one-to-one and permanent, so `new` refuses a slug that has EVER existed
    // rather than one that exists now. That makes `docs/semantics.md` section
    // 6 assumption 3 -- a ticket's events all descend from its creation commit
    // -- true rather than assumed.
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup(dir);

    ok(dir, &["next", "new", "task", "foo", "Foo"]);
    ok(dir, &["next", "do", "abandon", "foo", "not doing it"]);
    ok(dir, &["next", "do", "archive", "foo", ""]);
    assert!(
        show(dir, "main:.plan/tickets/foo.md").is_empty(),
        "archive did not remove the ticket from trunk"
    );

    let err = refused(dir, &["next", "new", "task", "foo", "Foo again"]);
    // The refusal has to say what happened, not merely refuse: the slug, that
    // it was used, and where it went.
    assert!(err.contains("foo"), "{err}");
    assert!(err.contains("has been used"), "{err}");
    // Named from the trailer, not from the commit subject: the reservation
    // reads the event stream, so what it can name is the verb that declared
    // last. Asserting a subject line here would test a different source than
    // the one the check consults.
    assert!(
        err.contains("'archive'"),
        "the refusal should name the verb that retired it: {err}"
    );

    // A different slug is unaffected -- the rule is about identity, not about
    // archival poisoning the backlog.
    ok(dir, &["next", "new", "task", "foo-2", "Foo again"]);
    assert_eq!(agree(dir, "foo-2"), "foo-2: todo");
    assert_eq!(agree(dir, "foo"), "foo: abandoned");
}

#[test]
fn a_schema_cannot_declare_a_verb_named_new() {
    // `new` is the floor of every bounded walk, so a verb of that name ends
    // each walk at itself -- silently, since the runner reads a verb's before
    // and after states through that same walk. Rejected at load, which means
    // every command fails rather than one misbehaving.
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup_with(
        dir,
        "kinds: [task]\nverbs:\n  - name: new\n    applies-to: [task]\n    to: todo\n",
    );

    let err = refused(dir, &["next", "board"]);
    assert!(err.contains("reserved"), "unhelpful refusal: {err}");
}

fn setup_with(dir: &Path, schema: &str) {
    git(dir, &["init", "-b", "main", "."]);
    git(dir, &["config", "user.email", "e2e@test"]);
    git(dir, &["config", "user.name", "E2E Test"]);
    std::fs::create_dir_all(dir.join(".plan/tickets")).unwrap();
    std::fs::write(dir.join(".plan/schema.yml"), schema).unwrap();
    std::fs::write(dir.join(".plan/tickets/.gitkeep"), "").unwrap();
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-m", "seed"]);
}

fn show(dir: &Path, spec: &str) -> String {
    let out = Command::new("git")
        .args(["show", spec])
        .current_dir(dir)
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).to_string()
}

/// Two repositories differing only in how much unrelated history precedes the
/// ticket's last transition.
fn repo_with_history(dir: &Path, commits: usize) {
    setup(dir);
    ok(dir, &["next", "new", "epic", "platform", "Platform"]);
    noise(dir, "hist", commits);
    ok(dir, &["next", "do", "close", "platform"]);
}

#[test]
fn the_bound_does_not_grow_with_history_before_the_last_transition() {
    // The whole point of the change, measured rather than asserted: reading
    // state costs commits-since-the-ticket-last-moved, so history that
    // predates the last transition is not read at all.
    let small = tempfile::tempdir().unwrap();
    let large = tempfile::tempdir().unwrap();
    repo_with_history(small.path(), 10);
    repo_with_history(large.path(), 40);

    let s = ok(small.path(), &["next", "state", "platform"]);
    let l = ok(large.path(), &["next", "state", "platform"]);
    assert_eq!(state_line(&s), "platform: done");
    assert_eq!(
        scanned(&s),
        scanned(&l),
        "the bounded walk grew with history that cannot affect the answer:\n{s}\n{l}"
    );
    assert_eq!(
        scanned(&s),
        1,
        "the last transition is the newest commit, so one commit should settle it:\n{s}"
    );

    // ...and the unbounded oracle grows by exactly the added history, which is
    // what makes the comparison above mean anything.
    let su = oracle(small.path(), "platform");
    let lu = oracle(large.path(), "platform");
    assert_eq!(
        scanned(&lu) - scanned(&su),
        30,
        "the unbounded walk should read every added commit:\n{su}\n{lu}"
    );
}

#[test]
fn the_bound_tracks_staleness_rather_than_age() {
    // The honest direction of the bound, pinned so nobody "fixes" it: history
    // AFTER a ticket's last transition IS scanned, because the walk cannot
    // know it holds nothing for this ticket without reading it. An idle ticket
    // is what costs; abandoning or archiving it is the remedy, and both are
    // planr operations rather than tuning knobs.
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    repo_with_history(dir, 5);
    let before = scanned(&ok(dir, &["next", "state", "platform"]));
    noise(dir, "after", 7);
    let after = scanned(&ok(dir, &["next", "state", "platform"]));
    assert_eq!(
        after - before,
        7,
        "commits since the last transition are the cost, and there are now 7 more"
    );
}

#[test]
fn the_creation_commit_is_the_floor() {
    // A ticket that has never transitioned has no event carrying a `to`, so
    // nothing in the schema can stop the walk. `new` is not a schema verb, but
    // it writes `Planr-Verb: new`, so the creation commit is already in the
    // stream and terminates it by name.
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup(dir);

    noise(dir, "before", 30);
    ok(dir, &["next", "new", "task", "late", "Late"]);
    noise(dir, "after", 4);

    let out = ok(dir, &["next", "state", "late"]);
    assert_eq!(state_line(&out), "late: todo");
    assert_eq!(
        scanned(&out),
        5,
        "the walk should stop at creation, having read only what follows it:\n{out}"
    );
    let whole = oracle(dir, "late");
    assert!(
        scanned(&whole) > 30,
        "the oracle should still be reading the whole history:\n{whole}"
    );
    assert_eq!(state_line(&out), state_line(&whole));
}
