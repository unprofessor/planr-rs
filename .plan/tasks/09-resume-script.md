---
id: resume-script
aliases: [resume-script]
kind: task
parent: worker-resumption
title: Add resume.sh to reconstruct in-flight worker state
status: todo
assignee: null
created: 2026-07-30
updated: 2026-07-30
tags: [retro, resumption, read-only]
depends_on: [cleanup-and-docs]
---

## Goal

Add a read-only `resume.sh <slug>` that reconstructs how far an in-flight
task got — branch, worktree, last commit, uncommitted changes, current
status, and `## Validation` progress vs `## Acceptance` — so a re-dispatched
worker or the leader can pick up a dead/interrupted worker cleanly
without manually inspecting git.

## Context

Retro finding #2: the first `loopback-only-net` attempt died mid-sentence
with zero commits; the worktree sat at `in_progress` with no progress
record. `resume.sh` reads the task file off the branch (`## Notes` /
`## Validation`) plus git state and prints a "resume brief." It is the
read-only counterpart to `review.sh` — same git-wrapper pattern
(`showRef`, `worktreeList`, `branchList`, `diffRefs`, plus `git log -1` and
`git status -s` run in the worktree).

The brief should answer, at a glance:
- Is there a worktree for `plan/<slug>`? Where? Is it clean?
- What is the task's `status` on the branch (`in_progress` / `review`)?
- Last commit on the branch: subject + relative date.
- Uncommitted changes in the worktree (`git status -s`).
- `## Validation` so far: which `## Acceptance` boxes appear addressed
  (heuristic: a validation line mentioning the criterion) vs still open.
- The tail of `## Notes` (last ~15 lines) — the worker's running log, which
  is where an interrupted investigation's conclusions live.

If no `plan/<slug>` branch exists, say so and exit 1 (nothing to resume).

See [[port-review]] for the read-only git/parser pattern this reuses. Pair
with [[incremental-progress-guidance]] (the worker discipline that makes
this brief useful).

## Acceptance

- [ ] `src/cli/resume.ts` (+ `scripts/resume.sh` shim) prints, for an
  in-flight `plan/<slug>`: branch, worktree path (or "(none)"), worktree
  dirty/clean, task `status` on the branch, last commit subject + date,
  `git status -s` if dirty, a `## Validation` progress summary vs
  `## Acceptance`, and the tail of `## Notes`. Read-only — no mutations,
  no checkout.
- [ ] Missing `plan/<slug>` branch → exit 1 with `no such branch: plan/<slug>`.
- [ ] Missing worktree (branch exists, no worktree) → still prints the
  branch-level info (status, last commit, validation summary) and notes the
  worktree is gone.
- [ ] `run-tests.sh` gains a resume test: claim a task, make a commit +
  an uncommitted edit on the branch, run `./scripts/resume.sh <slug>`,
  assert the last-commit subject, the dirty marker, and a `## Validation`
  / `## Notes` line appear; assert exit 1 for a non-existent branch.
- [ ] SKILL.md scripts table gains a `resume.sh` row (who: leader/worker;
  purpose: reconstruct in-flight state).

## Notes

- 2026-07-30 created. Depends on [[cleanup-and-docs]] (adds a SKILL.md row
  after the port's doc rewrite; reuses the ported git wrappers + parser).
