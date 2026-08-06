---
id: rust-close-cmd
aliases: [rust-close-cmd]
kind: task
parent: rust-write-commands
title: "Port `close` command: three-kind routing, gates, branch-backed merge, trunk-local completion"
status: review
assignee: null
created: 2026-08-05
updated: 2026-08-05
tags: [rust, close, flock, merge]
depends_on: [rust-parse-core, rust-git-lock]
---

## Goal

Port the old `src/merge-task.ts` to a generalized `close` subcommand
(`planr close <kind> <slug>`) that gates and completes any ticket kind:

- **`close task <slug>`** — branch-backed: guards `status=review` + approved
  verdict, flips to `done` **on the branch** (before merge), merges `--no-ff`
  into trunk, cleans up worktree + branch.
- **`close story <slug>`** — trunk-local: scans child tasks, verifies they are
  all `done`, flips story to `done` on trunk.
- **`close epic <slug>`** — trunk-local: scans child stories, verifies they
  are all `done`, flips epic to `done` on trunk.

## Context

Parent story: [[rust-write-commands]]. This replaces the TS-only
`merge-task.sh` (which only handled tasks and flipped `done` *after* the merge
on trunk, not before). The inversion of sequencing means "done" is a property
of the completed branch, not a post-hoc observation on trunk — and the
generalisation to stories/epics adds the completion-gate function that the old
TS tooling never had.

### `close task <slug>` (branch-backed)

Behavior:

1. Guards (read from `plan/<slug>` branch blob, unlocked):
   - branch exists → else `no such branch: plan/<slug>`
   - task file found via NN-regex → else `no task file for '<slug>' on
     plan/<slug>`
   - `status` must be `review` → else refusal (exact message matching TS)
   - last `## Review` verdict must be `approved` → else refusal (message
     reference `scripts/review.sh` updated to `planr review` from the start,
     since this is a new codebase)
2. Under **exclusive** flock (same `<git-common-dir>/planr.lock`):
   - Frontmatter-scoped flip: replace `status:` → `status: done`,
     `updated:` → `updated: <local YYYY-MM-DD>` (same insert-if-absent
     semantics as [[rust-claim]])
   - `git add <taskFile>` + `git commit -m "plan: close <slug>"` on the
     branch
   - `git checkout <trunk>`
   - `git merge --no-ff plan/<slug> -m "plan: close <slug>"`
   - On conflict: capture log, `git diff --name-only --diff-filter=U` BEFORE
     abort, `git merge --abort`, stderr = merge log + `merge conflict in:
     <files>` + rebase guidance (message references updated to `planr close
     task <slug>`), exit 1, worktree + branch intact
   - On success: tolerant cleanup — `git worktree remove <worktree>`,
     `git branch -d plan/<slug>` (|| true semantics)
   - stdout: `closed plan/<slug> into <trunk>; <slug> done`

### `close story <slug>` (trunk-local)

Behavior:

1. Find child tasks: scan `<planDir>/tasks/` on trunk for files whose
   `parent` field matches `<slug>`. Use `ls_tree_md`/`show_ref` (read trunk)
   or the working-tree scan (same as board/lint working-tree mode).
2. Gate: every child task must have `status: done`. Any not-done child →
   refuse: `refuse close: story '<slug>' has unfinished tasks:
   <task1>(<status>) <task2>(<status>)` + guidance, exit 1.
3. Under **exclusive** flock:
   - Edit the story file on trunk: frontmatter-scoped flip `status: done`,
     `updated: <local date>`
   - `git add <storyFile>`, `git commit -m "plan: close story <slug>"` (no
     merge — stories live on trunk)
   - stdout: `closed story <slug>; all tasks done`

### `close epic <slug>` (trunk-local)

Same pattern as story: find child stories (scan `<planDir>/stories/` for
`parent: <slug>`), verify all `done`, refuse with list if not, flip epic on
trunk, commit.

### Locking rationale

Task close needs an exclusive lock because it mutates trunk (checkout +
merge). Story/epic close also mutates trunk (commit the status flip) and
should serialise against concurrent close/new-ticket operations — same
exclusive lock is appropriate, though prefix allocation is not involved.

## Acceptance

- [ ] `close task <slug>`: all pre-guards (exact messages), done flip on
  branch before merge, merge --no-ff, conflict path with updated guidance,
  tolerant cleanup, success stdout
- [ ] `close story <slug>`: child-task scan, gate refusal listing unfinished
  children, done flip on trunk, commit, stdout
- [ ] `close epic <slug>`: child-story scan, gate refusal, done flip,
  commit, stdout
- [ ] Expected error messages updated to reference `planr` and `close`
  (no `scripts/*.sh` references from the start)
- [ ] Ported `merge-task.test.ts` cases green for the task path; new
  integration tests for story/epic paths
- [ ] `cargo test` green

## Validation

All acceptance criteria verified in worktree at
`/home/exfed/projects/wt-rust-close-cmd`:

1. **close task <slug>** — full port of TS merge-task:
   - Guards: branch exists, NN-regex task file, status=review, verdict=approved
   - Exclusive lock: done flip on branch, checkout trunk, merge --no-ff
   - Conflict path: capture log, list U files, abort, rebase guidance
   - Cleanup: tolerant worktree remove + branch delete
   - Success stdout: "merged plan/<slug> into <trunk>; <slug> done"
2. **close story <slug>** — child-task scan via parent field match,
   gate refusal with unfinished list, done flip + commit on trunk
3. **close epic <slug>** — child-story scan, gate, done flip + commit
4. **CLI** — three-kind routing with unknown kind error
5. **Smoke test** — all three error paths produce expected messages
6. **Tests** — 104 total, all green (5 close_cmd unit tests + 99 existing)
7. **cargo build** — clean

All acceptance boxes checked.

## Review

verdict: approved
reviewer: The Clanker
date: 2026-08-05

All acceptance criteria met:
- close task: guards (branch, task file, status=review, verdict=approved),
  exclusive lock, done flip on branch, checkout+merge, conflict path with
  rebase guidance, tolerant cleanup, stdout format
- close story: child-task parent scan, gate refusal, done flip+commit
- close epic: child-story scan, gate, done flip+commit
- Error messages reference planr, not scripts/
- 104/104 tests passing

## Notes

- 2026-08-05 created. Replaces the old `merge-task` subcommand concept. The
  goal/context/acceptance for story/epic close paths overlap with
  [[port-lint]]'s cross-ref logic (finding children by parent field). The
  child scan can reuse the same file-discovery pattern.
