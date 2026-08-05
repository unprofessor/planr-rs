---
id: rust-e2e-release
aliases: [rust-e2e-release]
kind: story
parent: rust-port
title: End-to-end test harness and release packaging
status: todo
assignee: null
created: 2026-08-05
updated: 2026-08-05
tags: [rust, e2e, release, packaging]
depends_on: [rust-read-commands, rust-write-commands]
---

## Goal

Prove the ported binary end-to-end in throwaway git repos (covering every
check class the TS `tests/run-tests.sh` exercises, plus flock interop with
the TS tooling), then package v0.1.0: README, `--version`, release profile,
string audit, tag.

## Context

Parent epic: [[rust-port]]. [[rust-e2e]] depends on all six subcommands and
is the gate for [[rust-release]]. The release task also performs the
user-facing string audit (`scripts/*.sh` → `planr <subcommand>` in refusal
and guidance messages) — deliberately deferred so the port lands
byte-compatible first.

## Acceptance

- [ ] [[rust-e2e]] done: full e2e suite green via `cargo test`, coverage
  parity with `run-tests.sh`, flock-interop test included
- [ ] [[rust-release]] done: README install/usage complete, `--version`,
  release profile, no user-facing `scripts/` references, v0.1.0 tagged
- [ ] `cargo test` green on main

## Notes

- 2026-08-05 created
