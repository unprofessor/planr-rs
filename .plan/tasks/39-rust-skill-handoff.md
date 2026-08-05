---
id: rust-skill-handoff
aliases: [rust-skill-handoff]
kind: task
parent: rust-e2e-release
title: Reconcile planr SKILL.md and agent-skills integration with the shipped CLI
status: todo
assignee: null
created: 2026-08-05
updated: 2026-08-05
tags: [rust, skill-integration, agent-skills, docs]
depends_on: [rust-release]
---

## Goal

Create a PR against the `agent-skills` tap that wires the shipped `planr`
binary into the planr skill — replacing the TS/scripts workflow, updating
docs for the new subcommand names and the new sequencing (done-before-merge,
story/epic gates), and removing the TS dev scaffolding.

## Context

Parent story: [[rust-e2e-release]]. The CLI is a standalone binary at
`~/projects/planr-cli`. The skill (`skills/planr/SKILL.md` + `scripts/` +
`references/` + `templates/` in the `agent-skills` repo) is how agents
discover and invoke planr.

Currently the skill references `scripts/*.sh` which exec the TS
`dist/cli/*.cjs`. After the handoff:

- Every command invocation changes from `scripts/<name>.sh <args>` to
  `planr <subcommand> <args>`.
- The `merge-task` concept is replaced by `close <kind> <slug>` with
  three routing paths.
- Stories and epics gain a completion gate (close refuses unless all
  children are `done`).
- The imperative sequence changes: done flip happens **on the branch
  before merge**, not after.
- The TS dev scaffolding (`src/`, `dist/`, `package.json`, `tsconfig.json`,
  `node_modules`) is removed.

This task is scoped entirely to the `skills/planr/` subtree of
`agent-skills` — no other skills or the tap root README are touched.

## Acceptance

- [ ] **SKILL.md** — all command invocations updated from `scripts/board.sh`
  etc. to `planr board` etc. No `.sh` references for the six commands.
  Quick-start and reference tables reflect the six subcommands (board,
  lint, new, claim, review, close).
- [ ] **Process documentation** — SKILL.md and `references/PROCESS.md`
  updated for the new sequencing: done is flipped on the branch before
  merge (not after); `close` gates stories/epics on child completion;
  three-kind routing for `close`.
- [ ] **README.md** (in `skills/planr/`) — Quick start, Scripts table, and
  Project layout updated for the single `planr` binary and the new
  subcommand names. The `Building from source` section is removed (no
  TS build).
- [ ] **TICKET-FORMAT.md** — `merge-task.sh` / `claim.sh` / `new-ticket.sh`
  references replaced with the corresponding `planr <subcommand>`.
- [ ] **Scripts** — `scripts/*.sh` either replaced with thin shims
  (`exec planr <subcommand> "$@"`) for backward compat, or removed
  entirely if the skill no longer references them.
- [ ] **TS dev scaffolding** — `dist/`, `src/`, `package.json`,
  `tsconfig.json`, `node_modules` deleted from the skill directory.
  The `tests/` directory and `tests/run-tests.sh` can stay or go
  depending on whether they still exercise anything (the Rust e2e suite
  replaces them).
- [ ] **Template file path change** — now that the binary embeds templates
  via `include_str!`, the `templates/` directory in the skill is no longer
  needed at runtime; confirm it's still present for reference or remove it
  with a note.
- [ ] **Coordinated with v0.1.0** — the skill PR is opened after (or
  alongside) the `v0.1.0` tag on `planr-cli` so agents can rely on that
  binary version existing.
- [ ] **Agent smoke test** — the updated skill is exercised by an agent in
  fresh context: at minimum `planr lint` passes on a test backlog, and
  `planr board`/`planr new`/`planr claim`/`planr review`/`planr close`
  produce expected output on a throwaway repo.

## Notes

- 2026-08-05 created. An existing PR in `agent-skills` aims to clean out
  the TS source — this task should coordinate with that PR to avoid
  conflicts (they may overlap on the `dist/`/`src/`/`package.json` removal
  and the SKILL.md rewrites).
- The `tests/run-tests.sh` harness in the skill exercises the TS CLIs; once
  the skill points at the Rust binary, that file either needs updating or
  removal. The Rust e2e suite (`tests/` in `planr-cli`) replaces it.
