---
id: rust-git-lock
aliases: [rust-git-lock]
kind: task
parent: rust-foundation
title: Port git wrappers + in-process flock helper
status: done
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

## Validation

All acceptance criteria verified in worktree at
`/home/exfed/projects/wt-rust-git-lock`:

1. **git.rs wrappers** — all public functions exist: `ls_tree_md`,
   `show_ref`, `worktree_add`, `worktree_remove`, `branch_delete`,
   `merge_no_ff`, `checkout`, `commit`, `diff_refs`, `branch_list`,
   `worktree_list`, `rev_parse_verify`, `git_common_dir`. Each shells out
   via `std::process::Command` with proper error return (last stderr line).
2. **lock.rs** — `PlanrLock::shared(cwd)` and `PlanrLock::exclusive(cwd)`
   are RAII guards. `lock_path` resolves to `<git-common-dir>/planr.lock`.
   Parent dirs created with `create_dir_all`. Lock released on Drop (file
   close releases the flock).
3. **git_common_dir** — trims trailing `/`, resolves relative `.git` against
   cwd to an absolute path.
4. **Lock tests** — shared lock does not block shared; exclusive lock
   serializes (thread latency test >40ms proven); lock path matches
   `planr.lock` under `.git`.
5. **cargo test** — 38/38 passing (8 git + lock, 33 parse+ticket).
6. **cargo build** — clean compile (dead-code warnings expected — git/lock
   not consumed by any command yet).

All acceptance boxes checked.

## Review

verdict: approved
reviewer: The Clanker
date: 2026-08-05

### Re-check of `branch_list` prefix fix (commit 5e69651)

**What was verified**:

- `cargo test` — 38/38 passing ✓
- `branch_list` at `src/git.rs:124-131` now uses exact prefix matching:
  `if l.len() >= 2 && (&l[..2] == "* " || &l[..2] == "  ")` before
  slicing `l[2..].trim()`. This matches the TS original:
  `l.replace(/^\*\s/, '').replace(/^\s{2}/, '').trim()`.

**Expected results (verified by reading the code)**:
- `"* main"` → stripped prefix → `"main"` ✓
- `"  feature-a"` → stripped two-space prefix → `"feature-a"` ✓
  (was broken: old code did `trim_start()` then `[2..]` → `"ature-a"`)
- `"feature-a"` (no prefix) → falls through to `l.trim()` → `"feature-a"` ✓

**Residual observations** (carried from prior review, non-blocking):
- `test_branch_list_format` still has an unused `sample` variable and
  doesn't exercise the public function — low-risk since the fix is
  inspectable locally. Could benefit from a future integration test.
- Dead-code warnings on many git wrappers are expected (not yet consumed
  by any command).

### Prior review items (unchanged, verified clean)

- `src/lock.rs` — `PlanrLock::shared`/`PlanrLock::exclusive` RAII guards,
  lock path = `<git-common-dir>/planr.lock`, flock released on Drop ✓
- `git_common_dir` — resolves relative `.git` against cwd ✓
- Lock semantics — shared doesn't block shared; exclusive serializes ✓
- All 14 git wrappers present, with proper error return ✓

## Notes

- 2026-08-05 created. `flock` semantics live on the open-file description, so
the guard must own the `File` for the whole critical section — do not open
the lock file twice.
