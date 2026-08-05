---
id: merge-gate-verify
aliases: [merge-gate-verify]
kind: story
parent: hotcell-firewall-hardening
title: Merge gate runs the project's verify command, not just the reviewer's verdict
status: todo
assignee: null
created: 2026-07-30
updated: 2026-07-30
tags: [retro, merge, verify]
depends_on: []
---

## Goal

Close the retro's biggest control failure: a broken test shipped to trunk
because `merge-task.sh` checked ticket state + an approved verdict but never
ran the project's verification. The merge gate must verify, not just trust.

## Context

The hotcell `loopback-only-net` reviewer approved despite seeing a test
failure (calling it "transient"); the leader merged without re-running
the suite on trunk. The failure was deterministic (5/5 on `main` after
merge). "Approved" was not sufficient — the flakiness masked a real bug, and
it wasn't caught until the next task ran the full suite.

The fix is a per-project verify hook the merge gate calls *after* the merge
commits but *before* the `done` flip / worktree cleanup, so a red verify
rolls the merge back and leaves the branch intact for the worker. planr is
project-agnostic, so the hook is opt-in: if a verify command is configured
(env `PLAN_VERIFY` or a `.plan/verify.sh`), it is mandatory; if not, the
gate falls back to verdict-only (backwards compatible) and the docs
recommend configuring one for code projects.

## Acceptance

- [ ] [[verify-hook]] merged: `merge-task.ts` runs the configured verify
  command post-merge, rolls back on failure, and the docs recommend it.

## Notes

- 2026-07-30 created. One task — see [[verify-hook]].
