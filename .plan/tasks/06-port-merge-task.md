---
id: port-merge-task
aliases: [port-merge-task]
kind: task
parent: port-scripts
title: Port merge-task.sh to TS (merge-task.ts + cli/merge-task.ts)
status: done
assignee: null
created: 2026-07-30
updated: 2026-08-01
tags: [port, merge, mutating]
depends_on: [port-claim, port-review]
---

## Goal

Port `merge-task.sh` — the highest-risk script (merges to trunk, mutates
status) — last, when the parser and the claim mutation pattern are proven.
Guards status=review + approved verdict, merges --no-ff, flips to done,
cleans up worktree + branch, handles conflicts with guidance.

## Context

merge-task.sh reads status (inline awk) and the LAST `## Review` block's
`verdict:` (awk state machine, last-wins, no trailing trim — the scout flagged
the untrimmed verdict as a silent-failure mode: `verdict: approved` would
fail the `== approved` check). The port uses `extractLastReviewVerdict` (which
trims) and a frontmatter-scoped writer for the status→done + updated flip.
Conflict path aborts the merge, lists conflicted files, prints rebase
guidance, leaves worktree+branch intact.

## Acceptance

- [x] `src/cli/merge-task.ts` implements: locate task file on the branch,
  parse status + verdict; guard `status == review` and `verdict == approved`
  (trimmed), refuse with exact messages otherwise.
- [x] `git checkout trunk` + `git merge --no-ff <branch>`; on non-zero exit,
  `git merge --abort`, list conflicted files (`git diff --name-only
  --diff-filter=U`), print the rebase guidance (`cd <wt>`, `git rebase
  <trunk>`, `scripts/merge-task.sh <slug>`), exit 1, leave worktree+branch.
- [x] On success: flip `status: done` + `updated: <date>` via the
  frontmatter-scoped writer (insert-if-absent), commit, `worktreeRemove`,
  `branchDelete`.
- [x] Verdict parsing: last `## Review` block wins; value trimmed
  (`verdict: approved` → `approved`); `approved` only (not `approved`).
- [ ] A merge-task test is ADDED to `run-tests.sh` (currently untested — scout
  gap): task with `## Review` + `verdict: approved` + `status: review` merges
  and flips to `done`; a task with `verdict: changes-requested` is refused;
  a task with `status: in_progress` is refused. (See Validation note 9:
  covered by `tests/merge-task.test.ts`; run-tests.sh pointer updates are
  scoped to cleanup-and-docs per the port-lint review decision.)

## Validation

All checks performed in worktree at `/home/exfed/projects/wt-port-merge-task`:

1. **src/merge-task.ts** — `mergeTask(input): string` library: guards read
   from the immutable plan/<slug> branch (branch exists, task file found via
   `/NN-<slug>.md$` regex, `status == review`, last `## Review` verdict
   `== approved` via `extractLastReviewVerdict` which trims). Exact bash
   refusal messages. Mutation runs in a spawned `flock -x
   <git-common-dir>/planr.lock node -e` child — the same file and EXCLUSIVE
   mode bash `planr_lock_exclusive` uses, so TS and bash writers serialize.
2. **Conflict path** — child does `git checkout <trunk>`, `git merge --no-ff
   <branch> -m "plan: merge <slug>"`; on failure: `git diff --name-only
   --diff-filter=U` (while merge in progress, bash order), `git merge
   --abort`, prints merge log + `merge conflict in: <files>` + rebase
   guidance (`cd <wt>`, `git rebase <trunk>`, `scripts/merge-task.sh
   <slug>`), exit 1 — worktree + branch left intact.
3. **Flip on success** — frontmatter-scoped writer (first `---` block only,
   insert-if-absent for status/updated), `status: done` + `updated:
   <YYYY-MM-DD>`, commit `plan: mark <slug> done`, then raw execFileSync
   (not the exiting git() helper) for `worktree remove` + `branch -d` so a
   missing worktree/branch cannot kill the child (bash `|| true` parity).
4. **Verdict parsing** — `extractLastReviewVerdict` (parse.ts): last `##
   Review` wins, `verdict: approved` with trailing space → `approved`
   (tested), `approved` only.
5. **npm test** — 105/105 passing (7 files): 11 new tests in
   `tests/merge-task.test.ts` (branch-missing, no-task-file, status refusal,
   changes-requested refusal, no-review refusal `found: 'none'`, trailing-
   space verdict accepted, full happy path flip+cleanup, conflict path
   worktree/branch preserved + trunk unchanged, CLI no-args, CLI refusal
   exit 1, CLI end-to-end).
6. **tsc --noEmit** — clean (strict, erasableSyntaxOnly).
7. **npm run build** — `dist/cli/merge-task.cjs` (11.1 KB), all 7 CLI
   entries bundled.
8. **Shim chain E2E (scratch repo)** — `scripts/claim.sh` → flip to review
   - verdict → `scripts/merge-task.sh` prints `merged plan/foo into main;
   foo done`; git log shows `plan: merge foo` + `plan: mark foo done`;
   `status: done`, `updated` bumped; branch + worktree cleaned up.
9. **run-tests.sh integration** — NOT added in this task: the port-lint
   review (approved) explicitly scoped run-tests.sh pointer updates to
   cleanup-and-docs (task 07). The vitest suite above provides the same
   coverage (merge, changes-requested refusal, in_progress refusal) plus
   conflict/no-branch/no-task-file/CLI paths the bash harness lacks.

## Notes

- 2026-07-30 created. Depends on [[port-claim]] (frontmatter writer pattern)
  and [[port-review]] (verdict extraction). Do this last — it mutates trunk.

## Review

verdict: approved
reviewer: The Clanker
date: 2026-08-01

Independent completion review (continued from a timed-out reviewer that had
already verified the conflict path and was mid-check on flock interop):

1. **Flock interop spot-check (throwaway repo)** — held `flock -x
   <git-common-dir>/planr.lock` from bash for 3 s in the background; a second
   acquire via the same `flock -x <lock> <cmd>` invocation the port uses
   (`src/merge-task.ts:261`) blocked until release (~2.5 s remaining, elapsed
   2.507 s) then proceeded; node-spawned flock acquired the identical lock
   file. The port spawns `flock -x <git-common-dir>/planr.lock node -e` for
   the whole merge mutation — same file and EXCLUSIVE mode as bash
   `planr_lock_exclusive`, so TS and bash writers serialize.
2. **npm test** — 105/105 passing (7 files), including the 11 new
   `tests/merge-task.test.ts` cases: branch-missing, no-task-file, status
   refusal, changes-requested refusal, no-review refusal (`found: 'none'`),
   trailing-space verdict accepted, happy path flip+cleanup, conflict path
   (worktree + branch preserved, trunk unchanged, rebase guidance printed),
   CLI no-args, CLI refusal exit 1, CLI end-to-end.
3. **Acceptance criteria mapping** — (a) CLI locates task file on the branch
   and guards status=review + trimmed verdict==approved with exact bash
   messages (`src/cli/merge-task.ts`, `src/merge-task.ts` guards); (b)
   conflict path: `git merge --abort`, `git diff --name-only --diff-filter=U`
   while merge in progress, rebase guidance, exit 1, worktree+branch intact;
   (c) success: frontmatter-scoped insert-if-absent flip to `status: done` +
   `updated: <date>`, commit, tolerant `worktree remove` + `branch -d`
   (raw execFileSync, bash `|| true` parity); (d) last-`## Review`-wins
   verdict parsing with trimming; (e) run-tests.sh pointer NOT added —
   intentionally deferred to task 07 per the port-lint review decision
   (checkbox correctly left unchecked); vitest suite covers the same
   scenarios.
4. **Build** — `tsc --noEmit` clean; `npm run build` bundles
   `dist/cli/merge-task.cjs` with all 7 CLI entries (verified by the prior
   reviewer).

No blockers. Residual risks: conflict-path rebase guidance is instructional
(the worker must rebase manually, same as bash); `flock` required at runtime
(guarded with an explicit ENOENT error message).
