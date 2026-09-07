//! A slug names one ticket, forever.
//!
//! `Planr-Ticket: <slug>` is the identity every event is attributed by, so a
//! slug that names two tickets makes the model incoherent rather than merely
//! untidy: the second ticket folds the first one's events, `board` and `state`
//! answer differently about the same live ticket, and every `from` gate reads
//! the second answer -- so a terminal ticket becomes re-enterable.
//!
//! `new` therefore refuses a slug that has ever been created. What this file
//! pins is the two ways that guarantee was wrong before, both of which are
//! about WHERE the question is asked:
//!
//!   * **keyed on the event log, not on the file path.** A path-shaped
//!     reservation is a different question wearing the same clothes, and every
//!     gap between the two -- a renamed plan directory, a purged path, a
//!     shallow clone -- reopened the hole with the trailers still in place.
//!   * **asked atomically with the write.** Reserving against one read of
//!     trunk and then committing against another is a TOCTOU: the loser's
//!     compare-and-swap succeeded, because it asserted a tip newer than the
//!     one its reservation had examined.

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
    String::from_utf8_lossy(&out.stdout).to_string()
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

/// Create a ticket, end it, and remove its file -- the history that makes the
/// slug look free while every one of its events survives.
fn create_and_retire(dir: &Path, slug: &str) {
    ok(dir, &["next", "new", "task", slug, "Original"]);
    ok(dir, &["next", "do", "abandon", slug, "not doing it"]);
    ok(dir, &["next", "do", "archive", slug, ""]);
}

#[test]
fn a_slug_whose_creation_is_unreachable_is_refused_rather_than_reported_free() {
    // The gap between the reservation and the fold, one layer down. The fold
    // reacts to ANY event for a slug; a reservation that reacted only to the
    // CREATION record drifts from it exactly where a history has been
    // rewritten above the creation. Grafting the `abandon` commit onto the
    // root cuts the `new` record out of reachability and leaves every later
    // trailer in place -- so the slug looked free while the fold still
    // answered for it, and creating it restored the whole divergence:
    // `state: todo`, `board: abandoned`, terminal ticket re-enterable.
    //
    // `filter-branch` dropping just the creation commit does the same thing.
    // The walk was already holding the evidence and threw it away.
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup(dir);
    create_and_retire(dir, "foo");

    let root = git_out(dir, &["rev-list", "--max-parents=0", "HEAD"])
        .trim()
        .to_string();
    let abandon = git_out(
        dir,
        &[
            "log",
            "--format=%H",
            "-1",
            "--grep=Planr-Verb: abandon",
            "main",
        ],
    )
    .trim()
    .to_string();
    git(dir, &["replace", "--graft", &abandon, &root]);

    assert!(
        git_out(dir, &["log", "--format=%s", "main"])
            .lines()
            .all(|l| l.trim() != "plan: new foo"),
        "the graft did not cut the creation commit out, so this proves nothing"
    );
    assert!(
        git_out(dir, &["log", "--format=%s", "main"])
            .lines()
            .any(|l| l.trim() == "plan: abandon foo"),
        "the graft cut too much -- the later events must survive"
    );

    let err = refused(dir, &["next", "new", "task", "foo", "Foo again"]);
    assert!(
        err.contains("no reachable creation"),
        "a severed lineage must not be reported as a free slug: {err}"
    );
    // A user seeing this has a broken repository, not a name collision, and
    // the two refusals must not read alike.
    assert!(
        !err.contains("has been used"),
        "the two refusals should be distinguishable: {err}"
    );
    assert!(err.contains("abandon") || err.contains("archive"), "{err}");
}

#[test]
fn a_slug_must_be_what_its_own_trailer_reads_back() {
    // Not an attack -- a trailing space from a shell paste, or an agent
    // assembling arguments. Git's trailer reader trims and so does the record
    // parser, so `new task "foo "` wrote `.plan/tickets/foo .md` while
    // declaring `Planr-Ticket: foo`: two files, one identity. `abandon "foo "`
    // then reported `todo -> todo` because it could not see its own effect,
    // `board` printed two rows called `foo`, and the ticket that actually went
    // terminal was the one nobody had touched.
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup(dir);
    ok(dir, &["next", "new", "task", "foo", "Foo"]);

    for bad in [
        "foo ", " foo", "Foo", "a/b", "", "-foo", "_foo", "foo bar", "foo\t",
    ] {
        // `--` so clap reads a leading-dash slug as the argument it is rather
        // than as an unknown flag; the check under test is planr's, not clap's.
        let err = refused(dir, &["next", "new", "task", "--", bad, "Bad"]);
        assert!(
            err.contains("invalid slug"),
            "'{bad}' was refused for the wrong reason: {err}"
        );
        assert!(
            err.contains("^[a-z0-9][a-z0-9_-]*$"),
            "the refusal should name the rule: {err}"
        );
    }

    // Nothing was written by any of those: one ticket, one file, one row.
    let tickets = std::fs::read_dir(dir.join(".plan/tickets"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("md"))
        .count();
    assert_eq!(tickets, 1, "a refused slug left a file behind");
}

#[test]
fn every_accepted_slug_survives_its_own_trailer_unchanged() {
    // The property the pattern stands in for. A slug is the ticket's filename
    // AND its identity in the event log, so the two have to be the same
    // string after the trailer has been written and read back -- by git's
    // reader, which is what planr folds from.
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup(dir);

    for slug in [
        "foo",
        "foo-bar",
        "foo_bar",
        "2fa",
        "v1-ship-it",
        "a",
        "foo--bar",
        "foo-",
    ] {
        ok(dir, &["next", "new", "task", slug, "Accepted"]);
        let read_back = git_out(
            dir,
            &[
                "log",
                "-1",
                "--format=%(trailers:key=Planr-Ticket,valueonly)",
                "main",
            ],
        );
        assert_eq!(
            read_back.trim_end_matches('\n'),
            slug,
            "'{slug}' does not survive its own Planr-Ticket trailer"
        );
        assert!(
            dir.join(format!(".plan/tickets/{slug}.md")).exists(),
            "'{slug}' did not land at the path its identity names"
        );
    }
}

#[test]
fn an_archived_slug_is_not_free() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup(dir);
    create_and_retire(dir, "foo");

    assert!(
        git_out(dir, &["show", "main:.plan/tickets/foo.md"]).is_empty(),
        "archive did not remove the ticket from trunk"
    );

    let err = refused(dir, &["next", "new", "task", "foo", "Foo again"]);
    // The refusal has to say what happened, not merely refuse.
    assert!(err.contains("has been used"), "{err}");
    assert!(
        err.contains("archive"),
        "the refusal should say what was last declared: {err}"
    );

    // The rule is about identity, not about archival poisoning the backlog.
    ok(dir, &["next", "new", "task", "foo-2", "A different ticket"]);
}

#[test]
fn renaming_the_plan_directory_does_not_free_a_slug() {
    // One `git mv` and a flag, or one PLANR_DIR export. The reservation used
    // to ask whether `.plan/tickets/foo.md` appeared in history; after the
    // rename it does not, while every `Planr-Ticket: foo` trailer still does.
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup(dir);
    create_and_retire(dir, "foo");

    git(dir, &["mv", ".plan", ".planr"]);
    git(dir, &["commit", "-q", "-m", "move the plan directory"]);

    let err = refused(
        dir,
        &["--plan-dir", ".planr", "next", "new", "task", "foo", "Foo"],
    );
    assert!(err.contains("has been used"), "{err}");
}

#[test]
fn purging_the_path_from_history_does_not_free_a_slug() {
    // The same class as the rename, reached the other way: `filter-branch`
    // removes the file from every tree while every commit, and so every
    // trailer, survives untouched.
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup(dir);
    create_and_retire(dir, "foo");

    let out = Command::new("git")
        .args([
            "filter-branch",
            "-f",
            "--index-filter",
            "git rm --cached --ignore-unmatch .plan/tickets/foo.md",
            "HEAD",
        ])
        .env("FILTER_BRANCH_SQUELCH_WARNING", "1")
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "filter-branch failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        git_out(
            dir,
            &[
                "log",
                "--format=%h",
                "--diff-filter=A",
                "--",
                ".plan/tickets/foo.md"
            ]
        )
        .trim()
        .is_empty(),
        "the purge did not remove the path from history, so this proves nothing"
    );

    let err = refused(dir, &["next", "new", "task", "foo", "Foo"]);
    assert!(err.contains("has been used"), "{err}");
}

#[test]
fn a_shallow_clone_refuses_to_create_rather_than_reserve_what_it_cannot_see() {
    // A truncated history answers "no such record" exactly as a complete one
    // does, so a reservation made here is a promise the repository cannot
    // back. Reads degrade transiently and recover when the clone is deepened;
    // a duplicate creation record does not, so this is the operation that
    // refuses.
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup(dir);
    create_and_retire(dir, "foo");
    ok(dir, &["next", "new", "task", "later", "Later"]);

    let clone = tempfile::tempdir().unwrap();
    let out = Command::new("git")
        .args([
            "clone",
            "--depth",
            "1",
            &format!("file://{}", dir.display()),
            ".",
        ])
        .current_dir(clone.path())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "clone failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        git_out(clone.path(), &["rev-parse", "--is-shallow-repository"]).trim(),
        "true",
        "the clone is not shallow, so this proves nothing"
    );

    let err = refused(clone.path(), &["next", "new", "task", "foo", "Foo"]);
    assert!(err.contains("shallow"), "{err}");
    assert!(
        err.contains("unshallow"),
        "the refusal should say how to fix it: {err}"
    );

    // Reads still work -- they are what the truncation degrades rather than
    // breaks, and refusing them would make a shallow checkout useless.
    assert!(ok(clone.path(), &["next", "state", "later"]).contains("later: todo"));
}

#[test]
fn concurrent_creations_of_one_slug_leave_one_genesis() {
    // The reservation and the write have to be one atomic step. When the
    // reservation read one tip and the compare-and-swap asserted another, a
    // racer that reserved before the winner's ref move and read the tip after
    // it committed on top and its CAS SUCCEEDED: two processes reported
    // success, one ticket file survived, and two `new` records existed for one
    // slug. Staggering across that window is what finds it; a synchronised
    // start does not, because every racer then reads the same tip.
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup(dir);
    // Some history, so the reservation walk takes long enough to be raced.
    for i in 0..40 {
        let file = format!("pad-{i}.txt");
        std::fs::write(dir.join(&file), "x\n").unwrap();
        git(dir, &["add", &file]);
        git(dir, &["commit", "-q", "-m", &format!("work {i}")]);
    }

    let bin = assert_cmd::cargo::cargo_bin("planr");
    for (round, stagger_ms) in [0u64, 3, 6, 10, 15, 25].into_iter().enumerate() {
        let slug = format!("race-{round}");
        let racers: Vec<_> = [0, stagger_ms]
            .into_iter()
            .map(|delay| {
                let bin = bin.clone();
                let dir = dir.to_path_buf();
                let slug = slug.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(delay));
                    std::process::Command::new(bin)
                        .args(["next", "new", "task", &slug, "Racer"])
                        .current_dir(dir)
                        .output()
                        .unwrap()
                })
            })
            .collect();

        let results: Vec<_> = racers.into_iter().map(|r| r.join().unwrap()).collect();
        let winners = results.iter().filter(|o| o.status.success()).count();
        assert!(
            winners <= 1,
            "{winners} processes reported creating '{slug}'"
        );

        // The invariant, independent of what anyone reported: one genesis.
        let subjects = git_out(dir, &["log", "--format=%s", "main"]);
        let created = subjects
            .lines()
            .filter(|l| l.trim() == format!("plan: new {slug}"))
            .count();
        assert_eq!(created, 1, "'{slug}' has {created} creation records");

        // And the loser is told what happened, in planr's own terms rather
        // than as raw plumbing output.
        for out in results.iter().filter(|o| !o.status.success()) {
            let err = String::from_utf8_lossy(&out.stderr);
            assert!(
                err.contains(&slug),
                "a losing racer's diagnosis does not name the slug: {err}"
            );
            assert!(
                !err.contains("cannot lock ref"),
                "a losing racer got raw git output: {err}"
            );
        }
    }
}

#[test]
fn the_oracle_seam_is_decided_by_its_value_and_not_by_its_presence() {
    // It is an env var, so it is inherited by every child process and CI
    // shell. Presence-testing meant PLANR_NEXT_ORACLE=0 turned the unbounded
    // walk ON -- the opposite of what anyone setting it that way intended,
    // and silent apart from a word in the cost line.
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup(dir);
    ok(dir, &["next", "new", "task", "foo", "Foo"]);

    let with = |value: &str| {
        let out = Command::cargo_bin("planr")
            .unwrap()
            .args(["next", "state", "foo"])
            .env("PLANR_NEXT_ORACLE", value)
            .current_dir(dir)
            .output()
            .unwrap();
        (
            out.status.success(),
            String::from_utf8_lossy(&out.stdout).to_string(),
            String::from_utf8_lossy(&out.stderr).to_string(),
        )
    };

    for off in ["0", "false", "off", "no", ""] {
        let (success, stdout, stderr) = with(off);
        assert!(success, "PLANR_NEXT_ORACLE={off} failed:\n{stderr}");
        assert!(
            !stdout.contains("UNBOUNDED"),
            "PLANR_NEXT_ORACLE={off} enabled the oracle:\n{stdout}"
        );
    }
    for on in ["1", "true", "YES", " on "] {
        let (success, stdout, stderr) = with(on);
        assert!(success, "PLANR_NEXT_ORACLE={on} failed:\n{stderr}");
        assert!(
            stdout.contains("UNBOUNDED"),
            "PLANR_NEXT_ORACLE={on} did not enable the oracle:\n{stdout}"
        );
        // Loud, because a cost line is too quiet for a setting that arrives by
        // inheritance from a shell nobody is looking at.
        assert!(
            stderr.contains("UNBOUNDED"),
            "no warning on stderr: {stderr}"
        );
    }

    let (success, _, stderr) = with("maybe");
    assert!(
        !success,
        "an unreadable value should be an error, not a shrug"
    );
    assert!(stderr.contains("PLANR_NEXT_ORACLE"), "{stderr}");
}
