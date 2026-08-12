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
    /// Show the backlog board (trunk + in-flight branches)
    Board {
        /// Git ref to read (default: trunk; empty string = working tree)
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
        /// worktree path (default: ../wt-<slug>)
        worktree: Option<String>,
        /// override trunk branch for this invocation
        trunk_override: Option<String>,
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

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Command::Board { r#ref } => {
            let tickets = match r#ref {
                Some(ref_) if ref_.is_empty() => board::read_working_tree_tickets(&cli.plan_dir),
                Some(ref_) => board::read_ref_tickets(&ref_, &cli.plan_dir),
                None => board::read_ref_tickets(&cli.trunk, &cli.plan_dir),
            };
            let branches = board::read_in_flight_branches(&cli.plan_dir);
            let input = board::BoardInput {
                trunk_tickets: tickets,
                branch_statuses: branches,
            };
            let out = board::render_board(&input);
            if !out.is_empty() {
                print!("{out}");
            }
        }
        Command::Lint { r#ref } => {
            let report = match r#ref {
                Some(ref_) if !ref_.is_empty() => lint::lint_ref(&ref_, &cli.plan_dir),
                _ => lint::lint_working_tree(&cli.plan_dir),
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
                    println!("{relative_path}");
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
            worktree,
            trunk_override,
        } => {
            let trunk = trunk_override.as_deref().unwrap_or(&cli.trunk);
            match claim::claim_task(
                &slug,
                trunk,
                &cli.plan_dir,
                worktree.as_deref(),
                std::path::Path::new("."),
            ) {
                Ok(wt_path) => println!("{wt_path}"),
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
            match abandon::abandon_ticket(
                &kind,
                &slug,
                &msg,
                &cli.trunk,
                &cli.plan_dir,
                std::path::Path::new("."),
            ) {
                Ok(out) => println!("{out}"),
                Err(e) => fail(&e),
            }
        }
        Command::Close { kind, slug } => {
            let result = match kind.as_str() {
                "task" => close_cmd::close_task(
                    &slug,
                    &cli.trunk,
                    &cli.plan_dir,
                    std::path::Path::new("."),
                ),
                "story" => close_cmd::close_story(
                    &slug,
                    &cli.trunk,
                    &cli.plan_dir,
                    std::path::Path::new("."),
                ),
                "epic" => close_cmd::close_epic(
                    &slug,
                    &cli.trunk,
                    &cli.plan_dir,
                    std::path::Path::new("."),
                ),
                _ => Err(format!("unknown kind: {kind} (want task|story|epic)")),
            };
            match result {
                Ok(msg) => println!("{msg}"),
                Err(e) => fail(&e),
            }
        }
    }
}
