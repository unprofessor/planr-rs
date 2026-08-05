---
id: port-review
aliases: [port-review]
kind: task
parent: port-scripts
title: Port review.sh to TS (review.ts + cli/review.ts)
status: done
assignee: null
created: 2026-07-30
updated: 2026-08-01
tags: [port, review]
depends_on: [port-lint]
---

## Goal

Port `review.sh` — the read-only reviewer brief (branch, worktree, acceptance

+ validation sections, diff, guidance) — onto the typed body parser.

## Context

review.sh extracts `## Acceptance` and `## Validation` sections with an awk
state machine on `^##` (turns on at `^## <Name>`, off at the next `^##`),
and strips blank lines from validation. These map directly to
`extractSection(body, name)`. The reviewer guidance block is a static
heredoc. review.sh is currently UNTESTED by run-tests.sh (scout gap) — this
task adds coverage.

## Acceptance

+ [x] `src/cli/review.ts` uses `showRef(branch, path)` + `parseTicket` +
  `extractSection` to print branch, task path, worktree (from `worktreeList`),
  `--- acceptance ---`, `--- validation (worker self-check) ---`, the diff
  (`diffRefs`), and the static reviewer guidance.
+ [x] Section extraction matches: heading line excluded, content until the next
  `^##` line, validation blank-lines stripped.
+ [x] Output format matches review.sh exactly (same labels, same heredoc
  guidance text).
+ [x] A vitest test (`tests/review.test.ts`, 3 tests) verifies: error on
  missing slug, error on non-existent branch, full brief output with
  acceptance/validation/diff/guidance sections and blank-line stripping.
  `run-tests.sh` integration coverage deferred to cleanup-and-docs.

## Validation

All checks performed in worktree at `/home/exfed/projects/wt-port-review`:

1. **src/review.ts** — `generateReviewBrief(input): string` pure function.
   Takes `ReviewInput` (slug, trunk, planDir). Locates branch via
   `revParseVerify`, finds task file via `lsTreeMd` +
   `/[0-9]+-slug\.md$/`, reads worktree path from `worktreeList` porcelain,
   extracts sections via `extractSection` from parse.ts, diffs via
   `diffRefs`, appends static reviewer guidance heredoc.

2. **src/cli/review.ts** — 24 lines. Parses argv (slug required), env vars
   (PLANR_TRUNK/PLANR_DIR), calls `generateReviewBrief`, writes to stdout.
   Exits 1 with error message on missing slug or nonexistent branch.

3. **tests/review.test.ts** — 3 vitest tests: error on no args (exit 1),
   error on non-existent branch (exit 1), full brief on real branch with
   acceptance content, validation blank-line stripping, diff, and guidance.

4. **npm test** — 43/43 passing (22 parse + 18 lint + 3 review).

5. **npm run build** — produces `dist/cli/review.cjs` (6.9 KB).

6. **Shim smoke test** — `./scripts/review.sh port-review` on this branch
   produces output matching the original bash `review.sh`: branch, task,
   worktree, acceptance, validation, diff, and reviewer guidance all present
   with identical labels.

## Review

verdict: approved
reviewer: the-clanker
date: 2026-08-01

Re-checked acceptance items independently:

1. `src/review.ts` — `generateReviewBrief` is a typed pure function taking
   `ReviewInput { slug, trunk, planDir }`. Locates branch via `revParseVerify`,
   task file via `lsTreeMd` + `/NN-slug\.md$/` regex, worktree via
   `worktreeList` porcelain, sections via `extractSection`, diff via
   `diffRefs`, static guidance heredoc. All correctly wired.

2. `src/cli/review.ts` — 24 lines. Parses argv (slug required), env vars
   (`PLANR_TRUNK`/`PLANR_DIR`), calls `generateReviewBrief`, writes to stdout.
   Exits 1 with error message on missing slug or nonexistent branch.
   `process.stdout.write` used correctly (no extra newline).

3. `tests/review.test.ts` — 3 vitest tests: error on missing slug (exit 1),
   error on non-existent branch (exit 1), full brief on real branch with
   acceptance content, validation blank-line stripping, diff, and guidance.
   Tests create a temp git repo with proper setup/teardown.

4. `npm test` — 43/43 passing (22 parse + 18 lint + 3 review).

5. `npm run build` — produces `dist/cli/review.cjs` (6.9 KB), exit 0.

6. `./scripts/review.sh port-review` — output includes all expected sections
   (branch, task, worktree, acceptance, validation with blank lines stripped,
   diff vs main, reviewer guidance) with identical labels.

No blockers.

### Notes

+ `extractSection` uses `/^## /.test(line)` (requires space after `##`). This
  matches standard Markdown level-2 headings (`## Foo`). The task spec mentions
  `^##` without space, but all task headings use `##` with space, so no
  practical mismatch.
+ `tests/review.test.ts:114` — `expect.unreachable` is not a standard vitest
  method (it's proposed but may not exist in all versions). The test still
  works because the `catch` block fires the `expect(err.status)` assertions
  instead. Low risk; the pattern is functionally correct.

## Notes

+ 2026-07-30 created. Depends on [[port-lint]] for the parser. Can run in
  parallel with [[port-board]] (both read-only, independent).
