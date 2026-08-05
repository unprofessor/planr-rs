---
id: rust-git-lock
aliases: [rust-git-lock]
kind: task
parent: rust-foundation
title: Port git wrappers + in-process flock helper
status: in_progress
assignee: null
created: 2026-08-05
updated: 2026-08-05
tags: [rust, git, flock]
depends_on: [rust-scaffold]
---

## Goal

Port `src/git.ts` (~120 LOC TS) to a `git.rs` shell-out module, and replace
the TS "spawn `flock … node -e <script>` child" pattern with a `lock.rs`
helper that holds the kernel flock in-process via `fs2`.

## Context

Parent story: [[rust-foundation]]. TS sources: `skills/planr/src/git.ts`,
plus `gitCommonDir`/`lockPath` duplicated in `claim.ts`, `merge-task.ts`,
`new-ticket.ts`.

Git wrappers (all via `std::process::Command`, utf-8 stdout, non-zero exit →
`Err` carrying captured stderr so callers can print git's last line; default
cwd = `git rev-parse --show-toplevel`):

- `ls_tree_md(ref, dir)`, `show_ref(ref, path)`
- `worktree_add(path, branch, ref)` — adds `-b` when the branch doesn't exist
- `worktree_remove(path, force)`, `branch_delete(branch, force)`
- `merge_no_ff(branch)`, `checkout(branch)`, `commit(msg, files)`
- `diff_refs(ref1, ref2)`
- `branch_list(pattern)` — strips the `* `/`  ` prefix per line
- `worktree_list()` — raw `worktree list --porcelain` lines
- `rev_parse_verify(ref)`; internal `branch_exists(branch)`

Lock helper:

- `git_common_dir(cwd)`: `git rev-parse --git-common-dir`, trim trailing
  `/`, and when git returns a relative path (`.git`) resolve it against the
  given cwd — matches the TS resolution exactly.
- Lock file: `<git-common-dir>/planr.lock` — the SAME path the TS/bash
  tooling uses, so Rust and TS serialize against each other on a shared repo
  during transition. Create the parent dir (TS does `mkdirSync recursive`).
- API: an RAII guard — `PlanrLock::shared(cwd)` / `PlanrLock::exclusive(cwd)`
  mapping to `fs2::FileExt::lock_shared` / `lock_exclusive` on the opened
  file, released on drop.
- Lock modes per command (must match TS): `new-ticket` exclusive, `claim`
  shared, `merge-task` exclusive; `board`/`lint`/`review` do not lock.
- Why in-process: in TS the whole claim/merge/new-ticket critical section is
  serialized into an embedded CommonJS string and run as
  `flock -s|-x <lock> node -e <script>` with inputs marshalled through env
  vars — only because the lock must be held by one process for the whole
  operation. In Rust we simply hold the `File` (and its flock) across the
  critical section. No child process, no env marshalling, no embedded script.

## Acceptance

- [ ] All wrappers above exist and are used by at least one unit or
  integration test (a throwaway `git init` repo via `tempfile`)
- [ ] `git_common_dir` resolves relative `.git` against cwd (test from a
  subdirectory and from a linked worktree)
- [ ] Lock guard test: two contenders taking the exclusive lock serialize
  (e.g. interleaved writes to a counter file stay consistent), and a shared
  lock does not block another shared lock
- [ ] Lock file path asserted to be exactly `<git-common-dir>/planr.lock`
- [ ] `cargo test` green

## Notes

- 2026-08-05 created. `flock` semantics live on the open-file description, so
  the guard must own the `File` for the whole critical section — do not open
  the lock file twice.
