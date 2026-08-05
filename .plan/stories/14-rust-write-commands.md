---
id: rust-write-commands
aliases: [rust-write-commands]
kind: story
parent: rust-port
title: Port write commands (new, claim, close)
status: todo
assignee: null
created: 2026-08-05
updated: 2026-08-05
tags: [rust, new, claim, close, flock]
depends_on: [rust-foundation]
---

## Goal

Port the three mutating subcommands — `new`, `claim`, `close` —
to Rust, replacing the TS pattern of spawning `flock … node -e <script>`
children with the flock held in-process across each critical section.

## Context

Parent epic: [[rust-port]]. Lock modes must match the TS/bash tooling
exactly, since both serialize on the same `<git-common-dir>/planr.lock`
during transition: `new` and `close task <slug>` (the branch-backed path) take
the **exclusive** lock; `claim` takes the **shared** lock. Each command has
a strict stdout contract (created path / worktree path / success line —
exactly one line) with all diagnostics on stderr; the informational lint
that `new` and `claim` run becomes an in-process engine call (the TS spawns
a `lint.cjs` subprocess). The TS port's equivalent tasks are
[[port-new-ticket]], [[port-claim]], [[port-merge-task]].

## Acceptance

- [ ] [[rust-new-ticket]] done: `new` subcommand with guards, embedded
  templates, exclusive-flock prefix allocation, one-line stdout
- [ ] [[rust-claim]] done: `claim` subcommand with dependency gate (exact
  blocker format), worktree creation, frontmatter-scoped status flip under
  shared lock
- [ ] [[rust-close-cmd]] done: `close <kind> <slug>` subcommand with three
  routing paths (task=bra nch-backed; story/epic=trunk-local), guards,
  exclusive-flock merge, conflict guidance, tolerant cleanup
- [ ] `cargo test` green on main

## Notes

- 2026-08-05 created
