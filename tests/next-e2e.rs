//! End-to-end suite for `planr next` -- the 0.4 typed-graph spike.
//!
//! The whole point is that no ticket file ever carries a `status`. Every
//! assertion about state here is an assertion about a fold over commit
//! trailers, so if enumeration is wrong these fail.

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

fn show(dir: &Path, spec: &str) -> String {
    show_args(dir, &["show", spec])
}

fn show_args(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).to_string()
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

/// A repo with the reference schema and nothing else.
fn setup(dir: &Path) {
    git(dir, &["init", "-b", "main", "."]);
    git(dir, &["config", "user.email", "e2e@test"]);
    git(dir, &["config", "user.name", "E2E Test"]);

    std::fs::create_dir_all(dir.join(".plan/tickets")).unwrap();
    let schema = include_str!("../.plan/schema.yml");
    std::fs::write(dir.join(".plan/schema.yml"), schema).unwrap();
    std::fs::write(dir.join(".plan/tickets/.gitkeep"), "").unwrap();
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-m", "seed"]);
}

#[test]
fn the_five_verb_loop_completes() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup(dir);

    ok(
        dir,
        &["next", "new", "task", "wire-runner", "Wire the runner"],
    );
    assert!(ok(dir, &["next", "state", "wire-runner"]).contains("wire-runner: todo"));

    ok(dir, &["next", "do", "claim", "wire-runner"]);
    assert!(ok(dir, &["next", "state", "wire-runner"]).contains("in_progress"));

    // `submit` gates on a section it does not write.
    let err = refused(dir, &["next", "do", "submit", "wire-runner"]);
    assert!(err.contains("Validation"), "unexpected refusal: {err}");

    // Satisfy the gate on the ticket's own branch, as a worker would.
    let wt = dir.join(".plan/worktrees/task/wire-runner");
    let ticket = wt.join(".plan/tickets/wire-runner.md");
    let body = std::fs::read_to_string(&ticket).unwrap();
    std::fs::write(&ticket, format!("{body}\n## Validation\n\ncargo test\n")).unwrap();
    git(&wt, &["add", "-A"]);
    git(&wt, &["commit", "-m", "work: add validation"]);

    ok(dir, &["next", "do", "submit", "wire-runner"]);
    assert!(ok(dir, &["next", "state", "wire-runner"]).contains("review"));

    // close before approve must be refused: the gate is a state, not a parse.
    let err = refused(dir, &["next", "do", "close", "wire-runner"]);
    assert!(err.contains("approved"), "unexpected refusal: {err}");

    ok(dir, &["next", "do", "approve", "wire-runner", "looks good"]);
    assert!(ok(dir, &["next", "state", "wire-runner"]).contains("approved"));

    let out = ok(dir, &["next", "do", "close", "wire-runner"]);
    assert!(out.contains("merged into main"), "close output: {out}");
    assert!(ok(dir, &["next", "state", "wire-runner"]).contains("done"));

    // The declaration rode in WITH the work: trunk has the review prose.
    let merged = std::fs::read_to_string(dir.join(".plan/tickets/wire-runner.md"));
    let merged = merged.unwrap_or_else(|_| {
        String::from_utf8_lossy(
            &Command::new("git")
                .args(["show", "main:.plan/tickets/wire-runner.md"])
                .current_dir(dir)
                .output()
                .unwrap()
                .stdout,
        )
        .to_string()
    });
    assert!(merged.contains("looks good"), "review prose lost: {merged}");
    assert!(
        !merged.contains("status:"),
        "a status field appeared: {merged}"
    );
}

#[test]
fn empty_declarations_are_still_found() {
    // The regression this whole module exists for: `submit` changes no bytes,
    // so a path-filtered log would miss it and the fold would report the
    // ticket as still in_progress.
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup(dir);

    ok(
        dir,
        &["next", "new", "task", "empty-decl", "Empty declaration"],
    );
    ok(dir, &["next", "do", "claim", "empty-decl"]);

    let wt = dir.join(".plan/worktrees/task/empty-decl");
    let ticket = wt.join(".plan/tickets/empty-decl.md");
    let body = std::fs::read_to_string(&ticket).unwrap();
    std::fs::write(&ticket, format!("{body}\n## Validation\n\nnone\n")).unwrap();
    git(&wt, &["add", "-A"]);
    git(&wt, &["commit", "-m", "work"]);

    ok(dir, &["next", "do", "submit", "empty-decl"]);

    // Prove the submit commit really is empty, and still counted.
    let diff = Command::new("git")
        .args(["show", "--stat", "--format=", "plan/task/empty-decl"])
        .current_dir(dir)
        .output()
        .unwrap();
    let diff = String::from_utf8_lossy(&diff.stdout);
    assert!(
        diff.trim().is_empty(),
        "submit was expected to change no files, but changed: {diff}"
    );
    assert!(ok(dir, &["next", "state", "empty-decl"]).contains("review"));
}

#[test]
fn dependency_gate_blocks_claim() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup(dir);

    ok(dir, &["next", "new", "task", "first", "First"]);
    ok(dir, &["next", "new", "task", "second", "Second"]);
    ok(dir, &["next", "do", "add-dep", "second", "first"]);

    let err = refused(dir, &["next", "do", "claim", "second"]);
    assert!(err.contains("first(todo)"), "unexpected refusal: {err}");

    // An unmet gate must not have left a ref behind.
    let (found, _, _) = planr(dir, &["next", "state", "second"]);
    assert!(found);
    let refs = Command::new("git")
        .args(["branch", "--list", "plan/task/second"])
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        String::from_utf8_lossy(&refs.stdout).trim().is_empty(),
        "a refused claim created a branch"
    );
}

#[test]
fn claim_is_atomic_create_or_fail() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup(dir);

    ok(dir, &["next", "new", "task", "once", "Once"]);
    ok(dir, &["next", "do", "claim", "once"]);
    // The ref IS the claim, so a second claim loses the compare-and-swap.
    let err = refused(dir, &["next", "do", "claim", "once"]);
    assert!(
        err.contains("in_progress") || err.contains("cannot create branch"),
        "unexpected refusal: {err}"
    );
}

#[test]
fn lifecycle_is_derived_not_declared() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup(dir);

    let out = ok(dir, &["next", "lifecycle", "task"]);
    assert!(out.contains("initial:  todo"), "{out}");
    assert!(out.contains("abandoned"), "{out}");
    assert!(out.contains("done"), "{out}");
    assert!(
        out.contains("in_progress --[submit         ]--> review"),
        "{out}"
    );

    // The unit is derived from `worktree: create`, never declared.
    let all = ok(dir, &["next", "lifecycle"]);
    assert!(all.contains("unit: task"), "{all}");
}

#[test]
fn a_stored_status_field_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup(dir);

    std::fs::write(
        dir.join(".plan/tickets/legacy.md"),
        "---\nkind: task\ntitle: \"Legacy\"\nstatus: in_progress\n---\n\n# Legacy\n",
    )
    .unwrap();
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-m", "legacy ticket"]);

    let err = refused(dir, &["next", "state", "legacy"]);
    assert!(err.contains("status"), "unexpected error: {err}");
}

#[test]
fn a_later_trunk_declaration_outranks_an_earlier_branch_one() {
    // Regression: events were once walked per-ref and concatenated, which put
    // every trunk event before every branch event regardless of when they
    // happened. A supervisor abandoning a yielded ticket then lost to the
    // worker's earlier yield, and the ticket stayed `todo`.
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup(dir);

    ok(dir, &["next", "new", "task", "scope-creep", "Add caching"]);
    ok(dir, &["next", "do", "claim", "scope-creep"]);
    ok(
        dir,
        &[
            "next",
            "do",
            "yield",
            "scope-creep",
            "needs a decision first",
        ],
    );
    assert!(ok(dir, &["next", "state", "scope-creep"]).contains("todo"));

    // The supervisor decides, on trunk, after the worker's branch event.
    ok(
        dir,
        &["next", "do", "abandon", "scope-creep", "wrong layer"],
    );
    let state = ok(dir, &["next", "state", "scope-creep"]);
    assert!(
        state.contains("abandoned"),
        "the supervisor's later decision lost to the worker's earlier one: {state}"
    );
}

/// Helper: claim a ticket and leave real work on its branch, then yield it.
fn yield_with_work(dir: &Path, slug: &str) {
    ok(dir, &["next", "new", "task", slug, "Work in flight"]);
    ok(dir, &["next", "do", "claim", slug]);
    let wt = dir.join(format!(".plan/worktrees/task/{slug}"));
    std::fs::write(wt.join("partial.rs"), "half an implementation\n").unwrap();
    git(&wt, &["add", "-A"]);
    git(&wt, &["commit", "-m", "wip"]);
    ok(
        dir,
        &["next", "do", "yield", slug, "needs a decision first"],
    );
}

#[test]
fn abandoning_work_in_flight_preserves_the_rationale_and_the_work() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup(dir);
    yield_with_work(dir, "held");

    let out = ok(dir, &["next", "do", "abandon", "held", "wrong layer"]);
    assert!(
        out.contains("preserved in history"),
        "unexpected output: {out}"
    );
    assert!(ok(dir, &["next", "state", "held"]).contains("abandoned"));

    // 1. The rationale for the handback reaches trunk, alongside the reason
    //    for the abandonment. Losing it was the whole problem.
    let ticket = show(dir, "main:.plan/tickets/held.md");
    assert!(ticket.contains("## Blocked"), "yield note lost: {ticket}");
    assert!(
        ticket.contains("needs a decision first"),
        "yield note lost: {ticket}"
    );
    assert!(ticket.contains("## Abandoned"), "{ticket}");

    // 2. The work is NOT applied to trunk's tree.
    assert!(
        Command::new("git")
            .args(["show", "main:partial.rs"])
            .current_dir(dir)
            .output()
            .unwrap()
            .status
            .success()
            .eq(&false),
        "abandoned work was applied to trunk"
    );

    // 3. ...but it is reachable from trunk, so nothing can be collected.
    let log = show_args(dir, &["log", "--oneline", "main"]);
    assert!(log.contains("wip"), "work not reachable from trunk: {log}");

    // 4. The ref is released, safely, because of 3.
    let refs = show_args(dir, &["branch", "--list", "plan/task/held"]);
    assert!(refs.trim().is_empty(), "ref survived: {refs}");
}

#[test]
fn abandoning_an_unclaimed_ticket_has_nothing_to_preserve() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup(dir);
    ok(dir, &["next", "new", "task", "idle", "Never started"]);

    // No ref, so nothing to preserve -- an ordinary advance on trunk.
    let out = ok(dir, &["next", "do", "abandon", "idle", "out of scope"]);
    assert!(
        !out.contains("preserved"),
        "nothing should have needed preserving: {out}"
    );
    assert!(ok(dir, &["next", "state", "idle"]).contains("abandoned"));
}

#[test]
fn a_yielded_ticket_can_be_resumed() {
    // The supervisor's likelier option. `yield` keeps the ref so the work and
    // the handback note survive the decision; `resume` re-dispatches it.
    // Without this verb only abandonment worked, because `claim` is
    // create-or-fail and the ref is still standing.
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup(dir);
    yield_with_work(dir, "cache");
    assert!(ok(dir, &["next", "state", "cache"]).contains("todo"));

    ok(dir, &["next", "do", "resume", "cache"]);
    assert!(ok(dir, &["next", "state", "cache"]).contains("in_progress"));

    // The partial work survives -- resuming is not re-claiming.
    let wt = dir.join(".plan/worktrees/task/cache");
    assert!(wt.join("partial.rs").exists(), "work lost on resume");

    // And the loop closes: a resumed ticket goes on through the normal path.
    let ticket = wt.join(".plan/tickets/cache.md");
    let body = std::fs::read_to_string(&ticket).unwrap();
    std::fs::write(&ticket, format!("{body}\n## Validation\n\nchecked\n")).unwrap();
    git(&wt, &["add", "-A"]);
    git(&wt, &["commit", "-m", "work"]);
    ok(dir, &["next", "do", "submit", "cache"]);
    assert!(ok(dir, &["next", "state", "cache"]).contains("review"));
}

#[test]
fn resume_is_refused_without_a_branch_to_work_from() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup(dir);
    ok(dir, &["next", "new", "task", "fresh", "Fresh"]);

    // A ticket that was never claimed has no ref, so there is nothing to
    // resume -- which is exactly what `base: own` checks, with no new operator.
    let err = refused(dir, &["next", "do", "resume", "fresh"]);
    assert!(err.contains("no branch"), "unexpected refusal: {err}");
}

#[test]
fn board_reports_every_ticket_in_its_own_state() {
    // Board had no coverage at all while being rewritten twice -- once to
    // share the history walk, once to batch the blob reads. Both changes
    // could have silently mixed tickets up.
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup(dir);

    ok(dir, &["next", "new", "task", "untouched", "Untouched"]);
    ok(dir, &["next", "new", "task", "working", "Working"]);
    ok(dir, &["next", "new", "task", "finished", "Finished"]);

    ok(dir, &["next", "do", "claim", "working"]);

    ok(dir, &["next", "do", "claim", "finished"]);
    let wt = dir.join(".plan/worktrees/task/finished");
    let ticket = wt.join(".plan/tickets/finished.md");
    let body = std::fs::read_to_string(&ticket).unwrap();
    std::fs::write(&ticket, format!("{body}\n## Validation\n\nchecked\n")).unwrap();
    git(&wt, &["add", "-A"]);
    git(&wt, &["commit", "-m", "work"]);
    ok(dir, &["next", "do", "submit", "finished"]);
    ok(
        dir,
        &[
            "next",
            "do",
            "approve",
            "finished",
            "several\nlines\nof review",
        ],
    );
    ok(dir, &["next", "do", "close", "finished"]);

    let board = ok(dir, &["next", "board"]);
    assert!(board.contains("3 ticket(s)"), "{board}");
    for (slug, state) in [
        ("untouched", "todo"),
        ("working", "in_progress"),
        ("finished", "done"),
    ] {
        let row = board
            .lines()
            .find(|l| l.trim_start().starts_with(slug))
            .unwrap_or_else(|| panic!("no row for {slug}:\n{board}"));
        assert!(
            row.contains(state),
            "{slug} should be {state}, row was: {row}"
        );
        assert!(row.contains("task"), "{slug} lost its kind: {row}");
    }
}

#[test]
fn board_frames_blobs_by_length_not_by_scanning() {
    // `git cat-file --batch` frames each record as `<oid> <type> <size>` then
    // that many BYTES. A parser that instead scanned for something
    // header-shaped would desynchronise on a ticket body containing a line
    // that looks like one -- and then report every later ticket wrongly.
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup(dir);

    ok(dir, &["next", "new", "task", "adversarial", "Adversarial"]);
    let path = dir.join(".plan/tickets/adversarial.md");
    let body = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        format!("{body}\n## Notes\n\ndeadbeefcafe blob 1234\nmissing\n\nmore text\n"),
    )
    .unwrap();
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-m", "adversarial body"]);

    ok(dir, &["next", "new", "task", "after", "After"]);
    ok(dir, &["next", "do", "claim", "after"]);

    let board = ok(dir, &["next", "board"]);
    assert!(board.contains("2 ticket(s)"), "{board}");
    let after = board
        .lines()
        .find(|l| l.trim_start().starts_with("after"))
        .unwrap_or_else(|| panic!("no row for 'after':\n{board}"));
    assert!(
        after.contains("in_progress"),
        "the ticket after the adversarial body was misread: {after}"
    );
}
