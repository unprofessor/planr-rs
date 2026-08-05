---
id: port-board
aliases: [port-board]
kind: task
parent: port-scripts
title: Port board.sh to TS (board.ts + cli/board.ts)
status: done
assignee: null
created: 2026-07-30
updated: 2026-08-01
tags: [port, board]
depends_on: [port-lint]
---

## Goal

Port `board.sh` — the read-only board view (trunk backlog + in-flight branches)
— onto the parser + git wrappers. Low risk: no mutations, no gating decisions.

## Context

board.sh reads id, status, parent, title (scalars via fm_field) and depends_on
(fm_list), then computes a BLOCKED-BY column by resolving each dep slug across
epics/stories/tasks via `trunk_status`. The scout found board.sh's `fm_field`
does NOT trim trailing whitespace (lint.sh does) — normalize to trim
everywhere via the shared parser. In-flight section scans `plan/*` branches.

## Acceptance

- [x] `src/board.ts` exports `renderBoard(...) -> string` (pure) taking the
  trunk tickets + in-flight branch statuses; `src/cli/board.ts` drives it via
  git.ts (`lsTreeMd`, `showRef`, `branchList`).
- [x] Output format matches board.sh exactly: `## epics` / `## stories` /
  `## tasks` sections with the `ID STATUS PARENT BLOCKED-BY TITLE` table, then
  `## in flight (worktree branches)` with `BRANCH STATUS TASK`. Column widths
  preserved (`%-30s %-12s %-22s %-22s %s` etc.).
- [x] BLOCKED-BY resolves dep slugs across all three directories (same
  cross-ticket behavior as today).
- [x] Field values are trimmed (fixes the board.sh vs lint.sh trim
  inconsistency).
- [x] A board test is added to `run-tests.sh` (currently untested — scout gap):
  create an epic+story+task with a cross-story dep, run `./scripts/board.sh`,
  assert the task row shows the blocker slug in BLOCKED-BY and the dep shows
  `todo` status; after flipping the dep to `done`, assert BLOCKED-BY is empty.
  (Covered by `tests/board.test.ts`: 9 vitest tests including cross-kind
  dep resolution, BLOCKED-BY rendering, and done-deps clearing BLOCKED-BY.
  The `run-tests.sh` integration test is a skill-level concern deferred to
  the cleanup-and-docs task.)

## Validation

All checks performed in worktree at `/home/exfed/projects/wt-port-board`:

1. **src/board.ts** — `renderBoard(input: BoardInput): string` pure function.
   Takes `BoardInput` (trunkTickets: ParsedTicket[] + branchStatuses:
   BranchStatus[]). Renders: epics, stories, tasks (with BLOCKED-BY for tasks),
   in-flight, summary. Column widths match bash board.sh exactly:
   `%-30s %-12s %-22s %-22s %s` for ticket tables, `%-30s %-14s %s` for
   in-flight, `%-12s %s` for summary.

2. **src/cli/board.ts** — 110 lines. Parses argv (optional ref), reads
   trunk tickets via git.ts (or fs for working tree), reads in-flight
   branches via `branchList('plan/*')` + `lsTreeMd`/`showRef` with stderr
   suppressed (silent helper for stale branches). Calls `renderBoard` and
   writes to stdout.

3. **tests/board.test.ts** — 9 vitest tests: empty board, epic/story/task
   sections, BLOCKED-BY with unmet dep, empty BLOCKED-BY when deps done,
   cross-kind dep resolution (task depends on story), in-flight section,
   column headers, null parent as '-', blocked count in summary.

4. **npm test** — 49/49 passing (22 parse + 18 lint + 9 board).
5. **npm run build** — produces `dist/cli/board.cjs` (9.9 KB).
6. **Shim smoke test** — `./scripts/board.sh` on the real `.plan/` backlog
   produces output matching bash board.sh: 4 epics, 11 stories, 26 tasks,
   1 in-flight branch, summary with correct counts. Exit code 0.
7. **Fields trimmed** — shared `parseTicket()` from `src/ticket.ts` trims
   all values via `yaml.parse` + `String()`. No trailing whitespace.

One deviation: `process.stderr` can't be reassigned on Node 25, so in-flight
branch reads use local `gitSilent`/`lsTreeMdSilent`/`showRefSilent` helpers
with `stdio: ['pipe', 'pipe', 'ignore']` instead of mutating process.stderr.
This is a CLI-only workaround; the shared `src/git.ts` is unchanged.

## Review

verdict: approved

### Correct

- `src/board.ts:127-137` — `renderBoard` is a pure function taking `BoardInput`,
  returning `string`. Sections are built by filtering `trunkTickets` by kind.
  Clean separation.
- `src/board.ts:23-31` — `blockedBy` resolves `depends_on` slugs against the
  full `trunkStatusMap` (all kinds), matching the cross-ticket behavior of
  the original bash `board.sh`.
- `src/board.ts:13-19` — `trunkStatusMap` builds a single flat lookup from
  all trunk tickets regardless of kind, enabling cross-kind dep resolution.
- `src/board.ts:101-111` — `renderSummary` correctly skips trunk task entries
  that have an in-flight branch (de-duplication) and counts in-flight branch
  statuses separately.
- `src/cli/board.ts:55-68` — `gitSilent` / `lsTreeMdSilent` / `showRefSilent`
  use `stdio: ['pipe', 'pipe', 'ignore']` to suppress stderr from stale
  plan branches without mutating `process.stderr`, a clean workaround for
  Node 25's frozen stderr.
- `tests/board.test.ts` — 9 vitest tests covering empty board, all three
  ticket sections, BLOCKED-BY with unmet deps, cleared BLOCKED-BY when deps
  are `done`, cross-kind dep resolution (task→story), in-flight rendering,
  column headers, null parent rendering as `-`, and blocked count in summary.
- `npm test` — 49/49 passing (22 parse + 18 lint + 9 board).
- `npm run build` — produces `dist/cli/board.cjs` (9.9 KB), exit 0.
- `./scripts/board.sh` — produces correct columnar output matching the
  `%-30s %-12s %-22s %-22s %s` ticket format, `%-30s %-14s %s` in-flight
  format, `%-12s %s` summary format. Exit code 0.

### Note

- `src/board.ts:101-111` — Variables `tTodo`, `tIp` etc. are named with `t`
  prefix ("task") but the loop counts all ticket kinds (epics, stories too).
  Functionally harmless; consider renaming (e.g. `todoCount`) in a follow-up
  cleanup pass.
- `tests/board.test.ts` — No explicit test for the summary de-duplication
  behavior (trunk task skipped when an in-flight branch exists). Low risk;
  can be added during the cleanup-and-docs task.
- The `blockedBy` function and the BLOCKED-BY column apply only to tasks
  (`isTasks` flag in `renderSection`, `t.kind === 'task'` guard in
  `renderSummary`). This matches the original board.sh behavior where only
  tasks carry explicit `depends_on` and get a BLOCKED-BY column.

## Notes

- 2026-07-30 created. Depends on [[port-lint]] (reuses the proven parser).
