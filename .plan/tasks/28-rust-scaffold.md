---
id: rust-scaffold
aliases: [rust-scaffold]
kind: task
parent: rust-foundation
title: "Cargo scaffold: six clap subcommands, env config, error conventions"
status: done
assignee: null
created: 2026-08-05
updated: 2026-08-05
tags: [rust, scaffold, clap]
depends_on: []
---

## Goal

Create the Cargo project at the root of this repo: binary `planr`, six clap
subcommands with the exact positional signatures, env-var config, and the
error/exit/stdout conventions every ported command follows.

## Context

Parent story: [[rust-foundation]]. TS sources:
`skills/planr/src/cli/*.ts` in agent-skills. Each TS CLI reads
`PLANR_TRUNK` (default `main`) and `PLANR_DIR` (default `.plan`) and takes
positionals:

- `board [ref]` — ref defaults to the trunk; `board ""` reads the working tree
- `lint [ref]` — no arg = working tree; arg = lint that ref
- `new-ticket <kind> <slug> <title> [parent-slug]`
- `claim <slug> [worktree] [trunk]` — worktree default `../wt-<slug>`
- `review <slug>`
- `merge-task <slug> [worktree] [trunk]`

Conventions to establish once in a shared module (every TS CLI does this ad
hoc): user-facing failures print a plain message to stderr and exit 1 — never
a panic/stack trace; stdout carries only the command's machine-consumed
result; missing required args print usage to stderr and exit 1.

Suggested crate layout: `src/main.rs` (clap + dispatch) plus modules
`parse.rs`, `ticket.rs`, `git.rs`, `lock.rs`, `lint.rs`, `board.rs`,
`review.rs`, `new_ticket.rs`, `claim.rs`, `merge_task.rs`.

Dependency selection (record the choices in Cargo.toml comments or README):

- `clap` (derive) — subcommands + `--help` + `--version`
- a YAML crate with eemeli/yaml-equivalent semantics for our subset:
  plain/quoted scalars, inline `[a, b]` lists, block lists, `null`
  (`serde_yaml` or a maintained fork such as `yaml-rust2` / `serde_yml` —
  pick one and justify)
- `regex` — wiki-links, slug patterns, filename matching
- `fs2` — `lock_shared`/`lock_exclusive` for the flock (see [[rust-git-lock]])
- a civil-date crate (`jiff` or `time`) — `YYYY-MM-DD` in both UTC
  (new-ticket) and local (claim/merge-task) flavors
- dev-deps: `assert_cmd`, `tempfile` for the CLI/integration tests

## Acceptance

- [ ] `Cargo.toml` + `src/` module skeleton compiles; `.gitignore` ignores
  `target/`
- [ ] `planr --help` lists the six subcommands; each `planr <cmd> --help`
  renders with the positionals above
- [ ] `PLANR_TRUNK` / `PLANR_DIR` env vars resolve with the documented
  defaults and are overridable per command
- [ ] Shared error helper: stderr message + exit 1 on user errors; no panic
  path for expected failures
- [ ] Dependency choices recorded (one line each: what + why)
- [ ] `cargo build && cargo test` green

## Validation

All acceptance criteria verified in worktree at
`/home/exfed/projects/wt-rust-scaffold`:

1. **Cargo.toml + src/ module skeleton** — `cargo build` succeeds with 9 deps
   compiled. Module stubs for all 11 modules exist under `src/`. `.gitignore`
   ignores `target/`.
2. **planr --help** — lists all six subcommands (board, lint, new-ticket,
   claim, review, merge-task). Each subcommand `--help` renders correct
   positional args matching the TS interface:
   - `new-ticket <KIND> <SLUG> <TITLE> [PARENT_SLUG]`
   - `claim <SLUG> [WORKTREE] [TRUNK_OVERRIDE]`
   - `merge-task <SLUG> [WORKTREE] [TRUNK_OVERRIDE]`
   - `board [REF]`, `lint [REF]`, `review <SLUG>`
3. **PLANR_DIR / PLANR_TRUNK** — clap reads env vars with defaults `.plan`
   and `main`, shown in `--help` as `[env: PLANR_DIR=] [default: .plan]`.
4. **Shared error helper** — `src/main.rs` exports `pub fn fail(msg: &str)
   -> !` which prints to stderr and exits 1. Used in all six stub
   implementations. No panic paths for user-facing errors.
5. **Dependency choices** — recorded in `Cargo.toml` comments with rationale
   per crate: clap (derive), serde_yaml, regex, fs2, jiff. Dev-deps:
   assert_cmd, tempfile.
6. **cargo build && cargo test** — both green. No warnings.

All acceptance boxes checked.

## Notes

- 2026-08-05 created. Keep the binary name `planr`; the subcommand names
  (`new-ticket`, `merge-task`) are hyphenated to match the README usage block.

## Review

verdict: approved
reviewer: The Clanker
date: 2026-08-05

Re-checked every acceptance criterion from scratch in worktree
`/home/exfed/projects/wt-rust-scaffold`:

1. **Cargo.toml + src/ skeleton** — verified 11 modules under `src/`
   (`main.rs`, `parse.rs`, `ticket.rs`, `git.rs`, `lock.rs`, `lint.rs`,
   `board.rs`, `review.rs`, `new_ticket.rs`, `claim.rs`, `merge_task.rs`).
   `.gitignore` contains `target/`. `cargo build` completes without warnings.
2. **planr --help** — ran `cargo run -- --help`: lists all six subcommands
   (board, lint, new-ticket, claim, review, merge-task). Each subcommand's
   `--help` was invoked individually:
   - `board [REF]` — optional ref, no-arg = working tree
   - `lint [REF]` — optional ref, omit = working tree
   - `new-ticket <KIND> <SLUG> <TITLE> [PARENT_SLUG]`
   - `claim <SLUG> [WORKTREE] [TRUNK_OVERRIDE]` — worktree default `../wt-<slug>`
   - `review <SLUG>` — required slug
   - `merge-task <SLUG> [WORKTREE] [TRUNK_OVERRIDE]`
   Positionals match the spec exactly.
3. **PLANR_DIR / PLANR_TRUNK** — help shows `[env: PLANR_DIR=] [default: .plan]`
   and `[env: PLANR_TRUNK=] [default: main]`. Tested with env vars set:
   `PLANR_DIR=custom_dir PLANR_TRUNK=custom_trunk` — help displayed the
   overridden values. Overridable per command via `-D` / `-t` global flags.
4. **Shared error helper** — `src/main.rs` contains `pub fn fail(msg: &str)
   -> !` that calls `eprintln!` then `process::exit(1)`. Verified by running
   `cargo run -- board` (stub) — printed "board: not yet implemented" to stderr
   and exited with code 1. No panic/stack trace paths.
5. **Dependency choices** — `Cargo.toml` comments document each choice:
   clap (derive) for CLI; serde_yaml for YAML frontmatter; regex for text
   patterns; fs2 for flock; jiff for civil dates. Dev-deps: assert_cmd,
   tempfile. All choices are justified.
6. **cargo build && cargo test** — both succeeded with zero warnings. Release
   build (`cargo build --release`) also succeeds.

No issues found. All acceptance criteria satisfied.
