---
id: rust-merge-task
aliases: [rust-merge-task]
kind: task
parent: rust-write-commands
title: "Port merge-task: review guards, exclusive-flock merge, conflict guidance"
status: todo
assignee: null
created: 2026-08-05
updated: 2026-08-05
tags: [rust, merge-task, flock]
depends_on: [rust-parse-core, rust-git-lock]
---

## Goal

Port `src/merge-task.ts` + `src/cli/merge-task.ts` (~335 LOC TS): gate on
`status: review` + an approved last-verdict, merge the `plan/<slug>` branch
into trunk `--no-ff` under an EXCLUSIVE flock, flip the task to `done`, and
clean up the worktree + branch — with the conflict path printing rebase
guidance and leaving everything intact.

## Context

Parent story: [[rust-write-commands]]. TS counterpart: [[port-merge-task]].
The TS `MERGE_SCRIPT` (embedded `node -e` child under `flock -x`) becomes
in-process code under the exclusive lock guard from [[rust-git-lock]].

Pre-guards (unlocked reads from the `plan/<slug>` branch blob — never the
worktree), exact messages:

- branch missing → `no such branch: plan/<slug>`
- task file missing (NN-regex `/[0-9]+-<escaped-slug>\.md$` on
  `ls_tree_md(branch, "<planDir>/tasks")`) → `no task file for '<slug>' on
  plan/<slug>`
- `status != review` → `refuse merge: task '<slug>' status is '<status>',
  must be 'review'.` + `the worker must self-validate against ## Acceptance
  (record ## Validation) and set status: review.`
- last `## Review` verdict (via `extract_last_review_verdict`, trimmed)
  `!= approved` → `refuse merge: no approved review verdict on '<slug>'
  (found: '<verdict|none>').` + `assign a reviewer: scripts/review.sh <slug>`
  — the `scripts/review.sh` wording is kept byte-identical for parity;
  [[rust-release]] audits it.

Exclusive-flock critical section:

1. `git checkout <trunk>`.
2. `git merge --no-ff plan/<slug> -m "plan: merge <slug>"`. On failure:
   capture the merge output; list conflicted files with `git diff
   --name-only --diff-filter=U` BEFORE `git merge --abort`; abort; stderr =
   merge log + `merge conflict in: <files comma-joined | <unknown>>` + the
   rebase guidance block (`The worker must rebase onto fresh trunk and
   resolve:` / `  cd <worktree>` / `  git rebase <trunk>   # resolve
   conflicts, git rebase --continue` / `  # then re-run: scripts/merge-task.sh
   <slug>`); exit 1. Worktree and branch stay intact for the worker.
3. On success: frontmatter-scoped flip of the merged task file on trunk —
   `status: done`, `updated: <local YYYY-MM-DD>`, same replace-or-insert
   semantics as [[rust-claim]]; `git add <taskFile>` + `git commit -m "plan:
   mark <slug> done"`.
4. Cleanup tolerating failure (bash `|| true` semantics — raw command
   results, not the erroring helper): `git worktree remove <worktree>`,
   `git branch -d plan/<slug>`.
5. stdout: `merged plan/<slug> into <trunk>; <slug> done`.

## Acceptance

- [ ] Ported `merge-task.test.ts` cases green (all four guards, happy path,
  conflict path)
- [ ] Conflict run: merge aborted, guidance printed, worktree + branch
  intact, exit 1
- [ ] Happy path: task file on trunk reads `status: done` with bumped
  `updated`, worktree removed, branch deleted, stdout is the success line
- [ ] `cargo test` green

## Notes

- 2026-08-05 created
