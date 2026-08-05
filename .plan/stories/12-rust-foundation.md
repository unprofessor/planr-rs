---
id: rust-foundation
aliases: [rust-foundation]
kind: story
parent: rust-port
title: Rust project foundation
status: done
assignee: null
created: 2026-08-05
updated: 2026-08-05
tags: [rust, scaffold, parser, git]
depends_on: []
---

## Goal

Stand up the Cargo project and port the three layers every subcommand needs:
the typed ticket parser, the git shell-out wrappers, and the in-process
flock helper.

## Context

Parent epic: [[rust-port]]. This is the Rust counterpart of the TS port's
[[parser-foundation]] + [[cli-scaffolding]] stories (tasks
[[ts-project-setup]], [[parse-core]], [[git-wrappers]], [[cli-shims]]).
Everything else in the epic builds on these three tasks; [[rust-scaffold]]
must land first, then [[rust-parse-core]] and [[rust-git-lock]] can run in
parallel.

## Acceptance

- [ ] [[rust-scaffold]] done: Cargo project, six clap subcommands, env
  config, error/exit conventions, dependency selection
- [ ] [[rust-parse-core]] done: parser core + fixtures + unit tests green
- [ ] [[rust-git-lock]] done: git wrappers + in-process flock helper with
  serialization test
- [ ] `cargo build && cargo test` green on main

## Notes

- 2026-08-05 created
