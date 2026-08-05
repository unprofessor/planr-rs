---
id: rust-read-commands
aliases: [rust-read-commands]
kind: story
parent: rust-port
title: Port read-only commands (lint, board, review)
status: todo
assignee: null
created: 2026-08-05
updated: 2026-08-05
tags: [rust, lint, board, review]
depends_on: [rust-foundation]
---

## Goal

Port the three read-only subcommands — `lint`, `board`, `review` — to Rust
with byte-identical output and exit codes.

## Context

Parent epic: [[rust-port]]. All three are pure reads over trunk refs and the
working tree — none of them takes the flock in the TS implementation, so they
need only [[rust-parse-core]] and [[rust-git-lock]]. The three tasks are
independent of each other and parallelize freely. The TS port's equivalent
tasks are [[port-lint]], [[port-board]], [[port-review]].

## Acceptance

- [ ] [[rust-lint]] done: engine + CLI, all message strings and exit codes
  byte-identical, ported lint tests green
- [ ] [[rust-board]] done: renderer (exact padding) + branch scan + summary,
  ported board tests green
- [ ] [[rust-review]] done: review brief format + worktree discovery +
  guidance text, ported review tests green
- [ ] `cargo test` green on main

## Notes

- 2026-08-05 created
