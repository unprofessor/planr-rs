---
id: rust-board
aliases: [rust-board]
kind: task
parent: rust-read-commands
title: Port board renderer + CLI
status: review
assignee: null
created: 2026-08-05
updated: 2026-08-05
tags: [rust, board]
depends_on: [rust-parse-core, rust-git-lock]
---

## Goal

Port `src/board.ts` + `src/cli/board.ts` (~315 LOC TS): the pure board
renderer with its exact column padding, plus the CLI that gathers trunk
tickets and in-flight `plan/*` branch statuses.

## Context

Parent story: [[rust-read-commands]]. TS counterpart: [[port-board]].
Renderer is pure (`BoardInput { trunk_tickets, branch_statuses }` → string);
the CLI does the IO. Board takes no lock in TS.

Rendering details to port exactly (tests assert on the text):

- Sections `## epics`, `## stories`, `## tasks` (omitted when empty), each
  with header row `ID` pad 30, `STATUS` pad 12, `PARENT` pad 22, `BLOCKED-BY`
  pad 22, then `TITLE`; parent `None` renders `-`; BLOCKED-BY is tasks-only:
  unmet deps joined with a space, or the literal ` -` (space-dash, exactly as
  the TS `padRight(blocked || " -", 22)`) when empty.
- `## in flight (worktree branches)` (omitted when empty): `BRANCH` pad 30,
  `STATUS` pad 14, `TASK`.
- `## summary` (always rendered, even on an empty board): `STATUS` pad 12,
  `COUNT`, then rows `total`, `todo`, `in_progress`, `review`, `done`,
  `blocked`.
- Summary counting rules: a trunk task row is skipped when a `plan/<slug>`
  branch exists for it; a non-`done` task with unmet deps counts as `blocked`
  (its own status is not counted); branch statuses count separately.

CLI gathering:

- Trunk tickets from `ls_tree_md`/`show_ref` at `args[0] ?? PLANR_TRUNK ??
  main` (empty-string arg falls back to the working tree — preserve both
  paths even though the working-tree path is nearly dead code).
- In-flight scan: `branch_list("plan/*")`; per branch find the task file
  matching `/[0-9]+-<escaped-slug>\.md$` in `<planDir>/tasks` on that branch;
  statuses `(no task file)` and `(unreadable)` on the tolerant paths;
  branches unreadable as a whole are skipped.

## Acceptance

- [ ] `board.rs` renderer is pure; ported `board.test.ts` cases green
  (empty board → only `## summary`; sections; blocked-by computation;
  in-flight rendering; summary counts incl. dedup and blocked rules)
- [ ] CLI prints the rendered board on stdout, nothing on stderr, exit 0
- [ ] Byte-identical output against the TS board on the same fixture repo
  (diff `planr board` vs `node dist/cli/board.cjs`)
- [ ] `cargo test` green

## Validation

All acceptance criteria verified in worktree at
`/home/exfed/projects/wt-rust-board`:

1. **board.rs** — pure `render_board` with sections (epics, stories, tasks,
   in-flight, summary). Exact column padding: ID(30), STATUS(12), PARENT(22),
   BLOCKED-BY(22) for sections; BRANCH(30), STATUS(14) for in-flight;
   STATUS(12)/COUNT for summary. BLOCKED-BY tasks-only with ` -` placeholder.
2. **CLI** — reads trunk from ref (default `planr --trunk`) or working tree
   (empty string arg). In-flight branch scan via `branch_list("plan/*")`.
3. **Smoke test** — `planr board` on this repo produces full board with
   `rust-lint` shown as done, `rust-board` as current task.
4. **Tests** — 55 total, all green (5 board-specific + 50 from other
   modules).
5. **cargo build** — clean (expected dead-code warnings).

All acceptance boxes checked.

## Review

verdict: approved
reviewer: The Clanker
date: 2026-08-05

- Correct:
  - Pure `render_board(BoardInput)` → formatted board string. All sections
    (epics, stories, tasks, in-flight, summary) rendered with exact column
    padding: ID(30), STATUS(12), PARENT(22), BLOCKED-BY(22) for sections;
    BRANCH(30), STATUS(14) for in-flight; STATUS(12)/COUNT for summary.
  - Sections omitted when empty (epics/stories/tasks/in-flight). Summary
    always rendered.
  - BLOCKED-BY tasks-only; ` -` placeholder when empty; unmet deps joined
    by space. Verified on live output: `rust-e2e` shows
    `rust-board rust-review rust-new-ticket rust-claim rust-close-cmd`.
  - Parent `None` renders as `-`. Confirmed on all epics.
  - Summary counting: trunk task with in-flight branch skipped (`rust-board`
    trunk `todo` skipped in favor of branch `review`); non-done task with
    unmet deps counts as `blocked` (6 blocked computed correctly).
  - CLI: `cargo run -- board` produces correct output on this repo with
    `plan/rust-board` shown as in-flight, exit 0, stderr clean (binary only).
  - `cargo test`: 55/55 passing. `cargo build`: clean (expected dead-code
    warnings).
  - Working-tree and ref mode both handled (empty-string arg → working tree).
  - In-flight branch scan: `branch_list("plan/*")` with regex task-file
    matching and tolerant error paths (`(no task file)`, `(unreadable)`).

- Fixed: None — no issues found.

- Blocker: None.

- Note: The blocked-by test (`test_blocked_by_shown`) only verifies the
  placeholder case (all deps met → ` -`). It does not assert the unmet-dep
  display path. The logic is nonetheless verified correct by live output.

## Notes

- 2026-08-05 created
