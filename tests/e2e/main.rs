//! End-to-end suite for the `planr` binary.
//!
//! Each scenario builds a throwaway git repo, seeds the minimal .plan/
//! structure needed, and runs the real `planr` binary via assert_cmd.
//!
//! Port of `skills/planr/tests/run-tests.sh` (~252 LOC bash, 40+ checks).

mod abandon;
mod board;
mod claim;
mod close;
mod common;
mod exclude;
mod lint;
mod new_ticket;
mod plan_dir;
mod secondary_worktree;
