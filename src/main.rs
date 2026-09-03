use clap::{Parser, Subcommand};
use std::process;

// The build-time git-derived version (set by build.rs via semvertag-shell).
// Falls back to Cargo.toml's version when git is unreachable.
const VERSION: &str = env!("PLANR_VERSION");

// Module skeleton -- filled in by subsequent tasks
mod abandon;
mod board;
mod claim;
mod close_cmd;
mod git;
mod lint;
mod lock;
mod new_cmd;
mod parse;
mod review;
mod ticket;

// ---------------------------------------------------------------------------
// Shared error/exit convention: every command calls `fail()` for user-facing
// errors. The message goes to stderr, the process exits 1 -- no panic, no
// stack trace. (Panics are reserved for bugs, not bad input.)
// ---------------------------------------------------------------------------
pub fn fail(msg: &str) -> ! {
    eprintln!("{msg}");
    process::exit(1);
}

/// Read the abandon message from the CLI argument or stdin.
///
/// - `Some("-")` or `None`: read from stdin until EOF.
/// - `Some(text)`: use the text as-is.
fn read_abandon_message(input: Option<&str>) -> Result<String, String> {
    match input {
        Some(s) if s != "-" => Ok(s.to_string()),
        _ => {
            use std::io::Read;
            let mut buf = String::new();
            std::io::stdin()
                .read_to_string(&mut buf)
                .map_err(|e| format!("cannot read message from stdin: {e}"))?;
            Ok(buf.trim().to_string())
        }
    }
}

// ---------------------------------------------------------------------------
// CLI definition -- matches the commands documented in README.md.
// Global env vars: PLANR_DIR (.plan), PLANR_TRUNK (main).
// ---------------------------------------------------------------------------
#[derive(Parser)]
#[command(
    name = "planr",
    version = VERSION,
    about = "Trunk-based backlog management for multi-agent development",
    disable_help_subcommand = true
)]
struct Cli {
    #[command(subcommand)]
    command: Command,

    /// Override the plan directory (env: PLANR_DIR)
    #[arg(
        short = 'D',
        long,
        env = "PLANR_DIR",
        default_value = ".plan",
        global = true
    )]
    plan_dir: String,

    /// Override the trunk branch (env: PLANR_TRUNK)
    #[arg(
        short = 't',
        long,
        env = "PLANR_TRUNK",
        default_value = "main",
        global = true
    )]
    trunk: String,
}

#[derive(Subcommand)]
enum Command {
    /// Show the backlog board (tickets + in-flight branches)
    Board {
        /// Commit-ish to read tickets from, e.g. main, HEAD, HEAD~2, a
        /// branch, or a SHA (default: the current on-disk working tree)
        r#ref: Option<String>,
    },

    /// Structural lint checks on the backlog
    Lint {
        /// Optional git ref to lint (omit for working tree)
        r#ref: Option<String>,
    },

    /// Scaffold a new ticket file
    New {
        /// kind: epic, story, or task
        kind: String,
        /// kebab-case slug (becomes the file identity)
        slug: String,
        /// human-readable title
        title: String,
        /// parent slug (required for story and task; omit for epic)
        parent_slug: Option<String>,
    },

    /// Claim a task: create a worktree branch and flip status to in_progress
    Claim {
        /// task slug
        slug: String,
        /// override trunk branch for this invocation
        trunk_override: Option<String>,
        /// Path for the worktree, relative to the directory planr was run
        /// in (default: <plan-dir>/worktrees/wt-<slug>, used both when the
        /// flag is omitted and when it is passed with no value;
        /// use --no-worktree to skip worktree creation entirely)
        #[arg(
            long = "worktree",
            num_args = 0..=1,
            default_missing_value = ""
        )]
        worktree: Option<String>,
        /// Skip worktree creation (for agents that manage their own).
        /// Conflicts with --worktree.
        #[arg(long, conflicts_with = "worktree")]
        no_worktree: bool,
    },

    /// Print a review brief for a task on its plan/<slug> branch
    Review {
        /// task slug
        slug: String,
    },

    /// Abandon a ticket without review, recording a free-text reason
    Abandon {
        /// ticket kind: task, story, or epic
        kind: String,
        /// ticket slug
        slug: String,
        /// Abandonment message (free text). Use "-" or omit to read from stdin.
        message: Option<String>,
    },

    /// Complete a ticket: gate-check children, flip to done, merge
    Close {
        /// ticket kind: task, story, or epic
        kind: String,
        /// ticket slug
        slug: String,
    },
}

/// Work on the backlog at the repository root, wherever planr was invoked.
///
/// A relative `--plan-dir` (the default `.plan`) is relative to the
/// repository, but every reader opens it relative to the process directory
/// and passes it to git as a pathspec, which git resolves the same way. Run
/// from a subdirectory, every command that touches the plan directory
/// therefore looked at the wrong one -- and each half of planr was wrong
/// about it in its own way. `board` rendered empty tables and warned that
/// every in-flight branch had no task on trunk; `lint` reported a clean
/// backlog it had never opened; `new` created a second backlog under the
/// subdirectory and printed the path; `claim`, `close` and `review` refused,
/// naming a file as absent from trunk while it sat committed at the root.
///
/// Fixing the readers alone, then the readers and `new`, only moved the
/// disagreement: `new` and `board` agreed on where the backlog was and
/// `claim` did not, so `planr claim <slug>` from a subdirectory failed with
/// "no task file for slug '<slug>' on main" about a file that was on main.
/// Every command enters the root, so there is one answer to where the backlog
/// is and every command gives it.
///
/// The one thing that must not move with it is a path the caller typed:
/// `claim --worktree <relative path>` means relative to where they are
/// standing, and entering the root first would silently redirect it. `main`
/// resolves that against the invocation directory before calling this, which
/// is the only reason `claim` was exempted before.
///
/// Outside a git repository there is no root to enter; the working-tree
/// readers still work relative to the current directory, as before. Any other
/// failure to ask git is worth a word, because what follows is a report about
/// a backlog read from wherever the process happened to start.
fn enter_repo_root() {
    let root = match git::toplevel_or_none() {
        Ok(Some(root)) => root,
        Ok(None) => return,
        Err(e) => {
            eprintln!(
                "warning: could not ask git for the repository root ({e}); \
                 using the plan directory relative to the current directory instead"
            );
            return;
        }
    };
    if let Err(e) = std::env::set_current_dir(&root) {
        eprintln!(
            "warning: could not enter the repository root {root} ({e}); \
             using the plan directory relative to the current directory instead"
        );
    }
}

/// Say so when the plan directory a command is about to read cannot be read.
///
/// `lint` prints nothing at all for a clean backlog, so a typo'd
/// `--plan-dir` -- or a run in a clone that has no backlog yet -- was
/// byte-identical to a clean bill of health, exit code included: it certified
/// a backlog it had never opened. Neither case is an error, since planr is
/// meant to be usable in a repository before `planr new` has ever made a
/// backlog, but a directory that is not readable is a fact neither command
/// can establish any other way.
///
/// "Not there" is only the commonest of several ways to read nothing, and
/// `Path::exists` answers true for the rest of them: a directory whose
/// permissions forbid opening it, and a plain file sitting where the backlog
/// should be, both read as zero tickets and both certified a clean backlog in
/// total silence. The readers below treat every I/O error as an empty
/// directory, which is the fail-open direction, so the error has to be
/// reported here or nowhere. Each kind subdirectory is checked the same way:
/// one unreadable `tasks/` hides every task in the backlog just as quietly.
/// A subdirectory that is simply absent is ordinary -- a backlog may hold
/// only epics -- so only the plan directory itself is worth a word for that.
///
/// Working-tree mode only -- in ref mode the plan directory is a pathspec in
/// a commit, and an empty read there is git's answer, not the filesystem's.
fn warn_if_plan_dir_missing(plan_dir: &str) {
    match std::fs::read_dir(plan_dir) {
        Ok(entries) => {
            if let Some(e) = first_read_error(entries) {
                eprintln!(
                    "warning: could not read the plan directory at '{plan_dir}' ({e}) -- \
                     planr read no backlog there, which is not the same as there being \
                     none; check --plan-dir and the directory's permissions"
                );
                return;
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            eprintln!(
                "warning: no plan directory at '{plan_dir}' -- there is no backlog here to \
                 read; check --plan-dir, or run `planr new` to start one"
            );
            return;
        }
        Err(e) => {
            eprintln!(
                "warning: could not read the plan directory at '{plan_dir}' ({e}) -- planr \
                 read no backlog there, which is not the same as there being none; check \
                 --plan-dir and the directory's permissions"
            );
            return;
        }
    }

    for kind in ["epics", "stories", "tasks"] {
        let dir = format!("{plan_dir}/{kind}");
        let problem = match std::fs::read_dir(&dir) {
            Ok(entries) => first_read_error(entries),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => Some(e),
        };
        if let Some(e) = problem {
            eprintln!(
                "warning: could not read '{dir}' ({e}) -- any tickets in it are missing \
                 from this run; check the directory's permissions"
            );
        }
    }
}

/// The first error a directory listing produced, if any.
///
/// Opening a directory can succeed and then fail entry by entry, so a listing
/// that was cut short reads as a short listing unless the errors are looked
/// at. The readers discard them.
fn first_read_error(entries: std::fs::ReadDir) -> Option<std::io::Error> {
    entries.filter_map(|e| e.err()).next()
}

/// Resolve a path the caller typed against the directory they typed it in.
///
/// Entering the repository root moves that directory out from under every
/// relative path in the command line, so any such path has to be pinned
/// first. `claim --worktree ../scratch` means the caller's `..`, not the
/// root's, and silently redirecting it is worse than the subdirectory bug
/// that made entering the root necessary.
fn resolve_from_invocation_dir(path: &str) -> String {
    let p = std::path::Path::new(path);
    if p.is_absolute() {
        return path.to_string();
    }
    match std::env::current_dir() {
        Ok(dir) => dir.join(p).to_string_lossy().to_string(),
        // Nothing to resolve against. The path stays as typed, which is what
        // planr did before it entered the root at all.
        Err(e) => {
            eprintln!(
                "warning: could not read the current directory ({e}); using '{path}' as \
                 given, relative to wherever planr ends up running"
            );
            path.to_string()
        }
    }
}

/// The directory planr works from once it has entered the repository root.
///
/// Every command that shells out to git or takes a lock needs a directory to
/// do it in, and after `enter_repo_root` that is the process directory. Ask
/// for it by name rather than passing `.`, so that what a command prints back
/// -- a worktree path, a ticket path -- resolves from anywhere, not only from
/// the directory planr happens to have entered.
fn work_dir() -> std::path::PathBuf {
    std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
}

fn main() {
    let cli = Cli::parse();

    // Before anything moves: a path from the command line belongs to the
    // directory the caller ran planr in.
    let command = match cli.command {
        Command::Claim {
            slug,
            trunk_override,
            worktree,
            no_worktree,
        } => Command::Claim {
            slug,
            trunk_override,
            worktree: worktree.map(|w| {
                if w.is_empty() {
                    w
                } else {
                    resolve_from_invocation_dir(&w)
                }
            }),
            no_worktree,
        },
        other => other,
    };

    // Every command works on the backlog at the repository root, so that no
    // two of them can disagree about where it is.
    enter_repo_root();
    let cwd = work_dir();

    match command {
        Command::Board { r#ref } => {
            let source = board::source_status_line(r#ref.as_deref());
            let tickets = match r#ref {
                Some(ref_) if !ref_.is_empty() => board::read_ref_tickets(&ref_, &cli.plan_dir),
                _ => {
                    warn_if_plan_dir_missing(&cli.plan_dir);
                    board::read_working_tree_tickets(&cli.plan_dir)
                }
            };
            let branches = board::read_in_flight_branches(&cli.plan_dir);
            // Warnings go to stderr so the board on stdout stays a clean,
            // parseable document.
            for w in board::ticket_warnings(&tickets) {
                eprintln!("{w}");
            }
            for w in board::branch_warnings(&branches, &tickets) {
                eprintln!("{w}");
            }
            let input = board::BoardInput {
                trunk_tickets: tickets,
                branch_statuses: branches,
            };
            let out = board::render_board(&input);
            println!("{source}\n");
            if !out.is_empty() {
                print!("{out}");
            }
        }
        Command::Lint { r#ref } => {
            let report = match r#ref {
                Some(ref_) if !ref_.is_empty() => {
                    let report = lint::lint_ref(&ref_, &cli.plan_dir);
                    // The same certification the missing-directory warning
                    // exists to prevent, one mode over: in ref mode there is
                    // no directory to look at, so a typo'd --plan-dir or a
                    // ref that predates the backlog produced an empty read
                    // and a clean bill of health, exit code included.
                    if report.tickets_read == 0 {
                        eprintln!(
                            "warning: no tickets under '{}' at '{}' -- there is no backlog \
                             at that ref to lint; check --plan-dir and the ref",
                            cli.plan_dir, ref_
                        );
                    }
                    report
                }
                _ => {
                    warn_if_plan_dir_missing(&cli.plan_dir);
                    lint::lint_working_tree(&cli.plan_dir)
                }
            };
            let out = lint::render_report(&report);
            if !out.is_empty() {
                print!("{out}");
            }
            if report.error_count > 0 {
                process::exit(1);
            }
        }
        Command::New {
            kind,
            slug,
            title,
            parent_slug,
        } => {
            match new_cmd::create_ticket(
                &kind,
                &slug,
                &title,
                parent_slug.as_deref(),
                &cli.plan_dir,
            ) {
                Ok(relative_path) => {
                    // Absolute, because the path is printed for the caller to
                    // use and the caller is not necessarily standing at the
                    // repository root. `$EDITOR $(planr new ...)` run from a
                    // subdirectory took the repo-relative path at face value
                    // and created a stray file under the subdirectory.
                    println!("{}", cwd.join(&relative_path).display());
                    // Informational lint findings on stderr (never fails)
                    let findings = new_cmd::lint_findings(&cli.plan_dir);
                    if !findings.is_empty() {
                        eprint!("{findings}");
                    }
                }
                Err(e) => fail(&e),
            }
        }
        Command::Claim {
            slug,
            trunk_override,
            worktree,
            no_worktree,
        } => {
            let trunk = trunk_override.as_deref().unwrap_or(&cli.trunk);
            // Only an explicit --no-worktree skips the worktree. An omitted
            // --worktree means the default path, same as passing the flag
            // with no value -- otherwise a bare `claim` would silently do
            // nothing at all (no branch, no status flip, no commit).
            let wt = if no_worktree {
                None
            } else {
                Some(worktree.unwrap_or_default())
            };
            match claim::claim_task(&slug, trunk, &cli.plan_dir, wt, &cwd) {
                Ok(out) => println!("{out}"),
                Err(e) => fail(&e),
            }
        }
        Command::Review { slug } => {
            match review::generate_review_brief(&slug, &cli.trunk, &cli.plan_dir) {
                Ok(brief) => print!("{brief}"),
                Err(e) => fail(&e),
            }
        }
        Command::Abandon {
            kind,
            slug,
            message,
        } => {
            let msg = match read_abandon_message(message.as_deref()) {
                Ok(m) => m,
                Err(e) => fail(&e),
            };
            match abandon::abandon_ticket(&kind, &slug, &msg, &cli.trunk, &cli.plan_dir, &cwd) {
                Ok(out) => println!("{out}"),
                Err(e) => fail(&e),
            }
        }
        Command::Close { kind, slug } => {
            let result = match kind.as_str() {
                "task" => close_cmd::close_task(&slug, &cli.trunk, &cli.plan_dir, &cwd),
                "story" => close_cmd::close_story(&slug, &cli.trunk, &cli.plan_dir, &cwd),
                "epic" => close_cmd::close_epic(&slug, &cli.trunk, &cli.plan_dir, &cwd),
                _ => Err(format!("unknown kind: {kind} (want task|story|epic)")),
            };
            match result {
                Ok(msg) => println!("{msg}"),
                Err(e) => fail(&e),
            }
        }
    }
}
