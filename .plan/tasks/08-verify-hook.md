---
id: verify-hook
aliases: [verify-hook]
kind: task
parent: merge-gate-verify
title: Add a per-project verify hook to the merge gate
status: todo
assignee: null
created: 2026-07-30
updated: 2026-07-30
tags: [retro, merge, verify]
depends_on: [cleanup-and-docs]
---

## Goal

Make `merge-task.ts` run the project's verification command after the merge
commits but before the `done` flip, so a red verify rolls the merge back and
"reviewer approved" can no longer ship a broken trunk.

## Context

Retro finding #1: `merge-task.sh` checks `status: review` + `verdict:
approved` but not build/test green — that's how a deterministic test failure
shipped to `main` on the hotcell run. The merge is the leader's gate;
approving is not the same as verifying.

planr is project-agnostic, so the hook is opt-in per project:
- **Discovery order:** `PLAN_VERIFY` env var (a shell command string) → else
  a executable `.plan/verify.sh` in the repo → else no hook (verdict-only,
  backwards compatible, but `merge-task.sh` prints a one-line notice that no
  verify hook is configured and the merge trusts the verdict alone).
- **When/where:** after `git merge --no-ff <branch>` succeeds on trunk (the
  merge commit already exists, working tree is the merged trunk), run the
  hook with cwd = repo root. Non-zero exit → roll back the merge
  (`git reset --hard HEAD~1` on trunk, restoring the pre-merge tip), refuse,
  print the verify stderr + guidance ("fix the failure on the task branch
  and re-set `review`, or waive via `## Waiver` if the failure is
  out-of-scope"), leave the worktree + branch intact for the worker. Do NOT
  flip to `done` or remove the worktree.
- **Env passthrough:** the hook runs with the worktree's build env; document
  that `PLANR_TRUNK`/`PLANR_DIR` are NOT passed to the hook (it's the project's
  own command, not planr's).

See [[port-merge-task]] for the ported `merge-task.ts` this builds on; see
[[done-with-waiver]] for the waiver path a verify failure may route to.

## Acceptance

- [ ] `src/cli/merge-task.ts` discovers the verify command (`PLAN_VERIFY` →
  `.plan/verify.sh` → none) and, when present, runs it post-merge on trunk
  before the `done` flip; on non-zero exit, rolls back the merge
  (`git reset --hard HEAD~1`), refuses, prints stderr + guidance, leaves
  worktree + branch.
- [ ] When no verify command is configured, the merge proceeds on
  verdict-only and `merge-task.sh` prints a one-line notice to stderr that
  no verify hook is configured (backwards compatible; exit 0 on success).
- [ ] The hook runs with cwd = repo root; `PLANR_TRUNK`/`PLANR_DIR` are not
  injected into the hook's env.
- [ ] `run-tests.sh` gains a merge-task verify test: a task with
  `status: review` + `verdict: approved` and a `.plan/verify.sh` that
  `exit 1`s → merge refused, trunk unchanged (the merge commit is gone),
  worktree + branch preserved; with a `.plan/verify.sh` that `exit 0`s →
  merges and flips to `done`.
- [ ] PROCESS.md "Integration" section documents the verify hook
  (discovery, when it runs, rollback, the notice when absent) and recommends
  configuring one for code projects. SKILL.md `merge-task.sh` row notes the
  hook.

## Notes

- 2026-07-30 created. Highest-priority retro fix. Depends on
  [[cleanup-and-docs]] so it lands on the ported `merge-task.ts` and the
  already-rewritten PROCESS.md/SKILL.md, not on bash it would then delete.
