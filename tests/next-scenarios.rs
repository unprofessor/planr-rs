//! The two topology scenarios the 0.4 rework exists to justify.
//!
//! `next-e2e.rs` proves the verb machinery works on a flat backlog. This file
//! proves the two shapes that motivated the rework in the first place:
//!
//!   * a CONTAINER whose close is gated on its children (Request A), and
//!   * a UNIT ABOVE A LEAF (Request B) -- one worktree per story rather than
//!     one per task, which is the entire cost argument for the redesign.
//!
//! Both had existed only as prose and a YAML fixture that nothing executed.

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

fn show(dir: &Path, spec: &str) -> String {
    let out = Command::new("git")
        .args(["show", spec])
        .current_dir(dir)
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).to_string()
}

/// A repo seeded with an arbitrary schema. The reference schema describes one
/// topology; these scenarios need two.
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

fn setup(dir: &Path) {
    setup_with(dir, include_str!("../.plan/schema.yml"));
}

/// Drive a task to `done` under the reference schema, doing real work in its
/// worktree on the way.
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
fn a_container_closes_only_when_every_child_is_terminal() {
    // Request A, executed. Children merge to trunk; the container's own close
    // is gated on them. `terminal` has to be stratified: `done` and
    // `abandoned` are different outcomes, but both end a child's life and the
    // container cannot tell them apart.
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup(dir);

    ok(dir, &["next", "new", "epic", "platform", "Platform"]);
    for slug in ["parser", "loader"] {
        ok(
            dir,
            &["next", "new", "task", slug, slug, "--parent", "platform"],
        );
    }

    // Nothing terminal yet, and the refusal has to name the blockers.
    let err = refused(dir, &["next", "do", "close", "platform"]);
    assert!(err.contains("children"), "{err}");
    assert!(err.contains("parser(todo)"), "{err}");
    assert!(err.contains("loader(todo)"), "{err}");

    finish_task(dir, "parser");

    // One down, one open: still refused, and only the open one is named.
    let err = refused(dir, &["next", "do", "close", "platform"]);
    assert!(err.contains("loader(todo)"), "{err}");
    assert!(
        !err.contains("parser("),
        "a done child is still reported as blocking: {err}"
    );

    // The second child ends differently -- abandoned, not done.
    ok(
        dir,
        &["next", "do", "abandon", "loader", "superseded by parser"],
    );

    ok(dir, &["next", "do", "close", "platform"]);
    assert!(ok(dir, &["next", "state", "platform"]).contains("done"));
    assert!(ok(dir, &["next", "state", "parser"]).contains("done"));
    assert!(ok(dir, &["next", "state", "loader"]).contains("abandoned"));

    // The done child's work reached trunk. The abandoned child's did not --
    // but its rationale did, which is the whole point of `ticket-only`.
    let parser = show(dir, "main:.plan/tickets/parser.md");
    assert!(parser.contains("## Validation"), "child work not merged");
    let loader = show(dir, "main:.plan/tickets/loader.md");
    assert!(loader.contains("superseded by parser"), "{loader}");
}

/// Request B: the unit sits ABOVE the leaf. `claim` declares `worktree:
/// create` on `story`, so the story is the cut; tasks beneath it are real
/// tickets that fold state, but own no ref and no worktree.
// Note the `r###`: the YAML contains `"##`, which would close an `r##` string.
const STORY_AS_UNIT: &str = r###"
kinds: [story, task]

worktrees: .plan/worktrees/$kind/$slug

templates:
  story: { body: "## Goal\n\n## Tasks\n" }
  task:  { body: "## Goal\n" }

verbs:
  # The cut. This verb, and only this verb, declares a worktree -- which is
  # what makes `story` the unit.
  - name: claim
    applies-to: [story]
    from: todo
    to: in_progress
    base: home
    effect: create
    worktree: create

  # Below the cut. No worktree, no ref: a sub-unit task is a bookkeeping
  # ticket whose code lives on its parent's branch.
  - name: start
    applies-to: [task]
    from: todo
    to: in_progress
    base: home

  - name: finish
    applies-to: [task]
    from: in_progress
    to: done
    base: home

  - name: close
    applies-to: [story]
    from: in_progress
    to: done
    require: { neighbors: { children: terminal } }
    base: own
    effect: merge
    worktree: remove
"###;

#[test]
fn the_unit_can_sit_above_the_leaf() {
    // The cost argument for the whole rework: N tasks under a story cost ONE
    // worktree and ONE branch, not N of each.
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup_with(dir, STORY_AS_UNIT);

    assert!(
        ok(dir, &["next", "lifecycle", "story"]).contains("in_progress"),
        "story lifecycle should be derivable"
    );

    ok(dir, &["next", "new", "story", "ingest", "Ingest"]);
    for slug in ["read-header", "read-body", "verify-crc"] {
        ok(
            dir,
            &["next", "new", "task", slug, slug, "--parent", "ingest"],
        );
    }

    ok(dir, &["next", "do", "claim", "ingest"]);

    // ONE worktree and ONE branch for the whole story.
    let worktrees = std::fs::read_dir(dir.join(".plan/worktrees/story"))
        .unwrap()
        .count();
    assert_eq!(worktrees, 1, "expected exactly one worktree for the story");
    assert!(
        !dir.join(".plan/worktrees/task").exists(),
        "a task below the cut must not get a worktree"
    );
    let refs = show_refs(dir);
    assert!(refs.contains("plan/story/ingest"), "{refs}");
    assert!(
        !refs.contains("plan/task/"),
        "a task below the cut must not get a branch: {refs}"
    );

    // The work happens once, in the story's worktree.
    let wt = dir.join(".plan/worktrees/story/ingest");
    std::fs::write(wt.join("ingest.rs"), "fn read_header() {}\n").unwrap();
    git(&wt, &["add", "-A"]);
    git(&wt, &["commit", "-m", "read the header"]);

    // Sub-unit tasks still fold state, and still gate the parent.
    ok(dir, &["next", "do", "start", "read-header"]);
    ok(dir, &["next", "do", "finish", "read-header"]);
    assert!(ok(dir, &["next", "state", "read-header"]).contains("done"));

    let err = refused(dir, &["next", "do", "close", "ingest"]);
    assert!(err.contains("read-body(todo)"), "{err}");
    assert!(err.contains("verify-crc(todo)"), "{err}");

    for slug in ["read-body", "verify-crc"] {
        ok(dir, &["next", "do", "start", slug]);
        ok(dir, &["next", "do", "finish", slug]);
    }

    ok(dir, &["next", "do", "close", "ingest"]);
    assert!(ok(dir, &["next", "state", "ingest"]).contains("done"));

    // The story's code reached trunk, and the worktree is gone.
    assert!(
        show(dir, "main:ingest.rs").contains("read_header"),
        "the story's work never merged"
    );
    assert!(
        !wt.exists(),
        "close declared `worktree: remove` but the worktree survives"
    );
}

fn show_refs(dir: &Path) -> String {
    let out = Command::new("git")
        .args(["for-each-ref", "--format=%(refname:short)", "refs/heads/"])
        .current_dir(dir)
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).to_string()
}

#[test]
fn an_archived_ticket_still_folds_from_its_trailers() {
    // `archive` removes the file from trunk. Everything that reads a ticket
    // by path is then blind, and the fold has only the trailer scan left --
    // a path nothing had ever exercised end to end.
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup(dir);

    ok(dir, &["next", "new", "task", "gone", "Gone"]);
    finish_task(dir, "gone");
    ok(dir, &["next", "do", "archive", "gone", ""]);

    // The file really is gone from trunk and from the working tree.
    assert!(
        show(dir, "main:.plan/tickets/gone.md").is_empty(),
        "archive did not remove the ticket from trunk"
    );
    assert!(!dir.join(".plan/tickets/gone.md").exists());

    // ...and its history still folds.
    let state = ok(dir, &["next", "state", "gone"]);
    assert!(
        state.contains("done") || state.contains("archived"),
        "an archived ticket lost its folded state: {state}"
    );

    // Board lists live tickets, so the archived one must be absent -- that is
    // what bounds T while C keeps growing.
    let board = ok(dir, &["next", "board"]);
    assert!(
        !board.contains("gone"),
        "archived ticket still on board: {board}"
    );
}
