//! Reproducible measurement of how `planr next board` scales.
//!
//! Run with `cargo bench`, or `PLANR_BENCH_SIZES=25,50,100 cargo bench` to
//! sweep other sizes.
//!
//! # What this measures, and why per-ticket cost is the number that matters
//!
//! Folding a ticket's state means finding its events, and an event carries no
//! path to limit a history walk by -- an empty declaration such as `submit`
//! touches no file, so `git log -- <ticket>` provably misses it. A board that
//! folds tickets one at a time therefore pays a full history walk per ticket:
//! O(tickets x commits). Because every ticket also *adds* commits, that is
//! quadratic in the backlog.
//!
//! Walking once and bucketing by `Planr-Ticket` is O(commits + events).
//!
//! A total-time speedup alone cannot distinguish "removed a factor" from
//! "shaved a constant". **Per-ticket cost can**: under the per-ticket walk it
//! rises as history grows, and under the single walk it stays flat. That is
//! the column to read.
//!
//! Two costs have been removed from board, and each shows a different shape.
//! Debug-profile numbers, from a scratch script that could build the older
//! implementations; `cargo bench` runs release and is several times faster in
//! absolute terms, so compare shapes across rows and never absolutes across
//! profiles.
//!
//! ```text
//!   tickets  commits   board   per-ticket
//!        25      151   314ms      12.6ms   <- a history walk per ticket:
//!        50      301   669ms      13.4ms      per-ticket cost RISES with
//!       100      601  1767ms      17.7ms      history
//!
//!        25      151    79ms       3.2ms   <- one shared walk: per-ticket
//!        50      301   132ms       2.6ms      cost goes FLAT
//!       100      601   283ms       2.8ms
//! ```
//!
//! Then the bottleneck moved from walking to *spawning*: one `git show` per
//! ticket to read its kind. Batching those into a single `cat-file --batch`
//! leaves two git processes for the whole board (release profile):
//!
//! ```text
//!        40      241   110ms       2.75ms  <- a process per ticket
//!        80      481   203ms       2.54ms
//!
//!        40      241    31ms       0.78ms  <- two processes total: per-ticket
//!        80      481    41ms       0.51ms     cost FALLS as it amortises
//! ```
//!
//! Absolute numbers are machine-specific and will drift; the *shape* is the
//! regression to watch. If per-ticket cost starts climbing with history
//! again, something has reintroduced a per-ticket walk.

use std::path::Path;
use std::process::Command;
use std::time::Instant;

const PLANR: &str = env!("CARGO_BIN_EXE_planr");
const SCHEMA: &str = include_str!("../.plan/schema.yml");

fn run(bin: &str, dir: &Path, args: &[&str]) {
    let out = Command::new(bin)
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap_or_else(|e| panic!("{bin} {args:?} could not run: {e}"));
    assert!(
        out.status.success(),
        "{bin} {args:?} failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn git(dir: &Path, args: &[&str]) {
    run("git", dir, args);
}

fn planr(dir: &Path, args: &[&str]) {
    run(PLANR, dir, args);
}

fn capture(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// A backlog of `n` tickets, so history grows with the ticket count -- the
/// regime where a per-ticket walk diverges.
///
/// Every fourth ticket is left **in flight**: claimed, with a live branch and
/// worktree. That is not decoration. An earlier version of this bench closed
/// every ticket, so board always ran with zero `plan/*` refs -- and therefore
/// never enumerated branches at all. It consequently could not see that board
/// failed outright whenever any ticket was claimed, because `git branch
/// --list` prefixes a ref checked out in another worktree with "+ " and the
/// marker travelled into the revision list. A benchmark that only exercises
/// the empty case measures the one state where the bug is invisible.
fn build_backlog(dir: &Path, n: usize) {
    git(dir, &["init", "-q", "-b", "main", "."]);
    git(dir, &["config", "user.email", "bench@test"]);
    git(dir, &["config", "user.name", "Bench"]);
    std::fs::create_dir_all(dir.join(".plan/tickets")).unwrap();
    std::fs::write(dir.join(".plan/schema.yml"), SCHEMA).unwrap();
    std::fs::write(dir.join(".plan/tickets/.gitkeep"), "").unwrap();
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-qm", "seed"]);

    for i in 1..=n {
        let slug = format!("task-{i}");
        let title = format!("Task {i}");
        planr(dir, &["next", "new", "task", &slug, &title]);
        planr(dir, &["next", "do", "claim", &slug]);

        // A worker satisfies submit's gate on the ticket's own branch.
        let wt = dir.join(format!(".plan/worktrees/task/{slug}"));
        let ticket = wt.join(format!(".plan/tickets/{slug}.md"));
        let body = std::fs::read_to_string(&ticket).unwrap();
        std::fs::write(&ticket, format!("{body}\n## Validation\n\nchecked\n")).unwrap();
        git(&wt, &["add", "-A"]);
        git(&wt, &["commit", "-qm", "work"]);

        // Leave a quarter of the backlog in flight, so the branch-enumeration
        // path is exercised rather than assumed away.
        if i % 4 == 0 {
            continue;
        }

        planr(dir, &["next", "do", "submit", &slug]);
        planr(dir, &["next", "do", "approve", &slug, "ok"]);
        planr(dir, &["next", "do", "close", &slug]);
    }
}

/// Median of `reps` runs -- a median rather than a mean because the first run
/// pays cold page-cache costs that are not what is being compared.
fn time_board(dir: &Path, reps: usize) -> u128 {
    let mut samples: Vec<u128> = (0..reps)
        .map(|_| {
            let start = Instant::now();
            planr(dir, &["next", "board"]);
            start.elapsed().as_micros()
        })
        .collect();
    samples.sort_unstable();
    samples[samples.len() / 2]
}

fn sizes() -> Vec<usize> {
    match std::env::var("PLANR_BENCH_SIZES") {
        Ok(raw) => raw
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect(),
        Err(_) => vec![20, 40, 80],
    }
}

fn main() {
    let reps: usize = std::env::var("PLANR_BENCH_REPS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5);

    println!("planr next board -- scaling\n");
    println!(
        "{:>8}  {:>8}  {:>9}  {:>11}",
        "tickets", "commits", "board", "per-ticket"
    );

    for n in sizes() {
        let tmp = tempfile::tempdir().expect("cannot create a temp dir");
        let dir = tmp.path();
        build_backlog(dir, n);

        let commits = capture(dir, &["rev-list", "--count", "main"]);
        let micros = time_board(dir, reps);

        println!(
            "{:>8}  {:>8}  {:>8.1}ms  {:>9.2}ms",
            n,
            commits,
            micros as f64 / 1000.0,
            micros as f64 / 1000.0 / n as f64
        );
    }

    println!(
        "\nper-ticket cost should stay FLAT as commits grow.\n\
         If it climbs, a per-ticket history walk has come back."
    );
}
