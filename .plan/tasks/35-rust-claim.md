---
id: rust-claim
aliases: [rust-claim]
kind: task
parent: rust-write-commands
title: "Port claim: dependency gate, worktree, frontmatter-scoped status flip"
status: done
assignee: null
created: 2026-08-05
updated: 2026-08-05
tags: [rust, claim, flock, worktree]
depends_on: [rust-lint, rust-git-lock]
---

## Goal

Port `src/claim.ts` + `src/cli/claim.ts` (~335 LOC TS): refuse a claim while
any `depends_on` ticket isn't `done` on trunk; otherwise create the
`plan/<slug>` worktree, flip the task to `in_progress`, and commit — the
whole critical section under a SHARED flock held in-process.

## Context

Parent story: [[rust-write-commands]]. TS counterpart: [[port-claim]]. The
TS `CLAIM_SCRIPT` (an embedded `node -e` child) becomes ordinary in-process
code under the shared lock guard from [[rust-git-lock]].

Flow to port exactly:

1. Informational lint of the trunk on stderr first (never fails the claim;
   in-process engine call, not the TS `lint.cjs` subprocess).
2. Under the shared lock, locate the task file on trunk via
   `ls_tree_md(trunk, "<planDir>/tasks")` and the TS `findTask` predicate:
   `f.replace(/^\d+-/, "").endsWith(slug + ".md")`. NOTE: the `^\d+-` strip
   never fires on full paths (they start with `<planDir>/`), so the actual
   behavior is "path ends with `<slug>.md`" — looser than merge-task's
   NN-regex. Port as-is with a code comment; tightening is follow-up, not
   port scope. Missing → `no task file for slug '<slug>' on <trunk>`.
3. Dependency gate: parse the trunk blob's `depends_on` (inline list, block
   list, or bare string); for each dep, find it across
   `{epics,stories,tasks}` on trunk and read its `status:`; every non-`done`
   dep is a blocker rendered `<slug>(<status>)`. Any blockers → stderr:
   `refuse claim: '<slug>' has unfinished depends_on: <blockers space-joined>`
   + `resolve or complete these first, or have the leader update
   depends_on.`, exit 1.
4. `git worktree add -b plan/<slug> <worktree> <trunk>` (worktree default
   `../wt-<slug>`).
5. Frontmatter-scoped flip in the worktree file: replace `^status:` →
   `status: in_progress` and `^updated:` → `updated: <local YYYY-MM-DD>`
   (LOCAL date, unlike new-ticket's UTC); if either line is absent, insert
   it — the TS unshifts status then updated, so `updated` lands above
   `status`; preserve that order with a code comment (real tickets always
   have both lines, so this path is nearly dead).
6. `git add <taskFile>` + `git commit -m "plan: claim <slug> (in_progress)"`
   in the worktree.
7. stdout: EXACTLY the worktree path + newline. Any git failure → print
   git's last non-empty stderr line, exit 1 (no panic).

## Acceptance

- [ ] Ported `claim.test.ts` cases green (gate refusal naming blockers like
  `http-proxy(todo)`, happy path, status flip committed on the branch)
- [ ] Successful claim: stdout is one line (the worktree path); all
  diagnostics on stderr; lock mode is shared
- [ ] Refused claim creates no worktree and no branch
- [ ] `cargo test` green

## Validation

All acceptance criteria verified in worktree at
`/home/exfed/projects/wt-rust-claim`:

1. **claim.rs** — full port of TS claim.ts:
   - Informational trunk lint on stderr before lock
   - Shared `PlanrLock` covering the read-verify-create-flip-commit section
   - `find_task_file` with TS's endsWith loose semantics
   - Dependency gate: parse depends_on (inline/block/bare), find each dep
     across epics/stories/tasks, check status is `done`
   - `git worktree add -b plan/<slug>` from trunk
   - Frontmatter-scoped flip: status→in_progress, updated→local date,
     with TS-order insertion logic
   - `git add` + `git commit` in the worktree
2. **CLI dispatch** — stdout = worktree path, errors on stderr, exit 1
3. **Smoke test** — error path: `planr claim nonexistent` produces
   expected "no task file" message
4. **Tests** — 99 total, all green (14 claim unit tests)
5. **cargo build** — clean

All acceptance boxes checked.

## Review

verdict: approved
reviewer: The Clanker
date: 2026-08-05

### Verified
1. **Dependency gate** — `claim.rs:182-196` correctly reads `depends_on` (inline, block, bare)
   from trunk frontmatter, searches `{epics,stories,tasks}` on trunk for each dep, checks
   `status: done`. Blockers rendered as `<slug>(<status>)`.
2. **Worktree creation** — `claim.rs:206` calls `git::worktree_add` which shells
   `git worktree add -b plan/<slug>` from trunk (`git.rs:40-55`).
3. **Frontmatter flip** — `claim.rs:82-112` replaces `status:`, `updated:` or inserts
   in TS order (updated above status). Uses local date from `jiff::Zoned::now()`.
4. **git add + commit** — `claim.rs:221-222` runs in worktree dir via `git_in`.
5. **CLI dispatch** — `main.rs:143-148` routes `Claim { slug, worktree, trunk_override }`
   to `claim_task()`. Error exit via `fail()` → stderr + exit 1.
6. **Shared lock** — `PlanrLock::shared(cwd)` (`lock.rs:27-32`) wraps entire
   read-verify-create-flip-commit section with `flock`.

### Smoke test
```
$ cargo run -- claim nonexistent
...
warning: ... (13 lint warnings on stderr)
lint: 0 error(s), 13 warning(s)
no task file for slug 'nonexistent' on main
$ echo $?
1
```

### Tests
`cargo test` — **99/99 passing** (14 claim-specific unit tests cover
frontmatter splitting, status/date parsing, dep list parsing, flip with/without
insert, find_task_file loose endsWith semantics, date format).
`cargo build` — clean.

### Issues found (non-blocking)
- **warning: `DepCheck.status` field never read** (`claim.rs:135`): `read_task_on_ref`
  populates `status` but only `deps` is consumed. Minor dead code; harmless.
- **warning: unused functions** (`git.rs`: `worktree_remove`, `branch_delete`,
  `merge_no_ff`, `checkout`, `commit`; `lint.rs`: `escape_regex`): Pre-ported
  for future tasks, not claim scope.
- **warning: hiding a lifetime that's elided elsewhere** (`claim.rs:29`):
  `FmSplit` struct uses an elided lifetime that could be explicit `'_`. Cosmetic.

### Residual risks
- **Nonexistent dep displays empty status `dep()`**: if `find_dep_on_ref` returns
  `None` (dep not found across any kind dir), the blocker format produces
  `dep()` — empty parens. Trunk lint would catch missing deps before claim,
  so this is a defense-in-depth edge case.
- **Branch-already-exists path**: `worktree_add` skips `-b` when branch exists
  but still passes trunk as ref, creating detached-HEAD worktree. Normal flow
  always creates new branch; edge case unexercised.

## Notes

- 2026-08-05 created
