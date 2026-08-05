use clap::{Parser, Subcommand};
use std::process;

// Module skeleton — filled in by subsequent tasks
mod parse;
mod ticket;
mod git;
mod lock;
mod lint;
mod board;
mod review;
mod new_cmd;
mod claim;
mod close_cmd;

// ---------------------------------------------------------------------------
// Shared error/exit convention: every command calls `fail()` for user-facing
// errors. The message goes to stderr, the process exits 1 — no panic, no
// stack trace. (Panics are reserved for bugs, not bad input.)
// ---------------------------------------------------------------------------
pub fn fail(msg: &str) -> ! {
    eprintln!("{msg}");
    process::exit(1);
}

// ---------------------------------------------------------------------------
// CLI definition — matches the six commands documented in README.md.
// Global env vars: PLANR_DIR (.plan), PLANR_TRUNK (main).
// ---------------------------------------------------------------------------
#[derive(Parser)]
#[command(
    name = "planr",
    version,
    about = "Trunk-based backlog management for multi-agent development",
    disable_help_subcommand = true,
)]
struct Cli {
    #[command(subcommand)]
    command: Command,

    /// Override the plan directory (env: PLANR_DIR)
    #[arg(short = 'D', long, env = "PLANR_DIR", default_value = ".plan", global = true)]
    plan_dir: String,

    /// Override the trunk branch (env: PLANR_TRUNK)
    #[arg(short = 't', long, env = "PLANR_TRUNK", default_value = "main", global = true)]
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
        Command::Board { .. } => {
            fail("board: not yet implemented");
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
        Command::New { .. } => {
            fail("new: not yet implemented");
        }
        Command::Claim { .. } => {
            fail("claim: not yet implemented");
        }
        Command::Review { .. } => {
            fail("review: not yet implemented");
        }
        Command::Close { .. } => {
            fail("close: not yet implemented");
        }
    }
}
