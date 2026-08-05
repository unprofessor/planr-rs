---
id: worker-resumption
aliases: [worker-resumption]
kind: story
parent: hotcell-firewall-hardening
title: Resumption protocol for interrupted workers + incremental-progress discipline
status: todo
assignee: null
created: 2026-07-30
updated: 2026-07-30
tags: [retro, resumption, worker]
depends_on: []
---

## Goal

Make an interrupted worker recoverable without the leader manually
inspecting `git log` + worktree state. Pair a `resume.sh` that reconstructs
how far a task got with a worker discipline of committing findings to the
task file *during investigation* — so progress survives an interrupt and the
leader has in-flight visibility without disturbing the worker.

## Context

The first `loopback-only-net` attempt ended mid-sentence with zero commits;
the worktree sat at `in_progress` with no record of how far it got. The
re-dispatched worker *did* commit incrementally, which is why the second
attempt was recoverable — but that was the worker's discipline, not the
scheme's. The leader pre-seeded the design conclusion into `## Notes`
before re-dispatching, a technique worth formalizing.

This story also subsumes the retro's lower-priority "in-flight visibility"
item: the incremental `## Notes` commits + `board.sh`'s in-flight branch
scan are the heartbeat — no separate mechanism needed.

## Acceptance

- [ ] [[resume-script]] merged: `resume.sh` reports branch, worktree, last
  commit, uncommitted changes, current status, and how far `## Validation`
  got vs `## Acceptance`.
- [ ] [[incremental-progress-guidance]] merged: worker workflow instructs
  committing `## Notes` findings incrementally (even mid-investigation) and
  the leader re-dispatch technique (pre-seed the design conclusion into
  `## Notes` before re-dispatching).

## Notes

- 2026-07-30 created. Two tasks; [[incremental-progress-guidance]] follows
  [[resume-script]] (both edit SKILL.md).
