---
id: rust-port
aliases: [rust-port]
kind: epic
title: Port planr CLIs to a single Rust binary
status: todo
assignee: null
created: 2026-08-05
updated: 2026-08-05
tags: [rust, port, cli]
---

## Goal

Replace the six TypeScript planr CLIs with one self-contained Rust binary,
`planr`, exposing six subcommands — `board`, `lint`, `new-ticket`, `claim`,
`review`, `merge-task` — per the usage block in this repo's README.md. The
port preserves the stdout/stderr contracts, user-facing messages, exit codes,
and flock semantics pinned by the TS test suite, so behavior carries over
byte-for-byte; the node runtime, esbuild bundle, and `scripts/*.sh` shims all
disappear.

## Scope

- New: Cargo project at the root of this repo producing a single `planr`
  binary (clap subcommands, templates embedded via `include_str!`, no runtime
  path resolution, no node).
- Ported: parser core (`parse.ts` + `ticket.ts`), git wrappers (`git.ts`),
  lint engine, board renderer, review brief, and the three writers
  (`new` / `claim` / `close`) with their `flock` serialization — held
  **in-process** instead of the TS `flock … node -e <script>` child pattern.
- Preserved: the ticket format, env vars (`PLANR_TRUNK`, `PLANR_DIR`),
  positional-arg CLI shapes, all user-facing strings (initially
  byte-identical, including the TS quirks flagged in each task's Context),
  and the `<git-common-dir>/planr.lock` file location + lock modes so TS/bash
  and Rust tooling serialize against each other on a shared repo during
  transition.
- Tests: unit tests ported from vitest, plus an end-to-end integration suite
  in throwaway git repos replacing `tests/run-tests.sh`.

## Out of scope

- Changes to the `agent-skills` repo — TS removal, SKILL.md/scripts cutover,
  and shim deletion are handled by an existing PR there.
- Changing the ticket format, the process/roles, or the git workflow.
- New features beyond parity. Documented TS quirks are ported as-is and
  flagged in each task's Context; fixing them is follow-up work, not port
  scope.
- GitHub Releases automation; the first release is a manual tag.

## Context

Source of truth: `/home/exfed/projects/agent-skills/skills/planr/src/*.ts`
(~1,550 LOC across 9 core modules + 6 CLI entries) with contracts pinned by
`tests/*.test.ts` (~1,950 LOC vitest) and `tests/run-tests.sh` (e2e in a
throwaway git repo). Target interface: this repo's README.md usage block.

Stories: [[rust-foundation]] (scaffold, parser, git+flock) →
[[rust-read-commands]] (lint, board, review) and [[rust-write-commands]]
`(`new`, claim, `close`) in parallel → [[rust-e2e-release]].

## Notes

- 2026-08-05 created. Planned against the TS port at agent-skills `main`
  (fa11e80); predecessor epic: [[port-scripts-to-typescript]].
- Sequencing: [[rust-scaffold]] first; then [[rust-parse-core]] and
  [[rust-git-lock]] in parallel; read commands and most write commands
  parallelize after that; [[rust-e2e]] gates [[rust-release]].
- Compat note: a ticket title containing `": "` breaks YAML frontmatter
  parsing (nested-mapping error) unless quoted — the two affected titles in
  this backlog are quoted; the Rust parser must not regress here either
  (see [[warn-on-parse-failures]] for the silent-skip failure mode).
