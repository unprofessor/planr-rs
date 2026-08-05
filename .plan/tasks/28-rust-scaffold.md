---
id: rust-scaffold
aliases: [rust-scaffold]
kind: task
parent: rust-foundation
title: "Cargo scaffold: six clap subcommands, env config, error conventions"
status: todo
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

## Notes

- 2026-08-05 created. Keep the binary name `planr`; the subcommand names
  (`new-ticket`, `merge-task`) are hyphenated to match the README usage block.
