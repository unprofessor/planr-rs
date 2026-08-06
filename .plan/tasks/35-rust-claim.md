---
id: rust-claim
aliases: [rust-claim]
kind: task
parent: rust-write-commands
title: "Port claim: dependency gate, worktree, frontmatter-scoped status flip"
status: in_progress
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

## Notes

- 2026-08-05 created
