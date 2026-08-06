---
id: rust-e2e-release
aliases: [rust-e2e-release]
kind: story
parent: rust-port
title: End-to-end test harness and release packaging
status: done
assignee: null
created: 2026-08-05
updated: 2026-08-05
tags: [rust, e2e, release, packaging]
depends_on: [rust-read-commands, rust-write-commands]
---

## Goal

Prove the ported binary end-to-end in throwaway git repos (covering every
check class the TS `tests/run-tests.sh` exercises, plus flock interop with
the TS tooling), package v0.1.0, then reconcile the planr skill in
`agent-skills` to consume the shipped binary — removing the TS scaffolding
and updating all documentation for the new subcommand names and sequencing.

## Context

Parent epic: [[rust-port]]. [[rust-e2e]] depends on all six subcommands and
is the gate for [[rust-release]]. The [[rust-skill-handoff]] task is the
final integration step that wires the shipped binary into the agent-skills
tap, coordinating with the existing TS-cleanup PR and the v0.1.0 release
tag.

## Acceptance

- [ ] [[rust-e2e]] done: full e2e suite green via `cargo test`, coverage
  parity with `run-tests.sh`, flock-interop test included
- [ ] [[rust-release]] done: README install/usage complete, `--version`,
  release profile, no user-facing `scripts/` references, v0.1.0 tagged
- [ ] [[rust-skill-handoff]] done: agent-skills PR ready — SKILL.md,
  README, process docs updated; scripts and TS scaffolding removed;
  coordinated with v0.1.0
- [ ] `cargo test` green on main

## Notes

- 2026-08-05 created
