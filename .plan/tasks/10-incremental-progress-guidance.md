---
id: incremental-progress-guidance
aliases: [incremental-progress-guidance]
kind: task
parent: worker-resumption
title: "Worker guidance: commit findings to the task file during investigation"
status: todo
assignee: null
created: 2026-07-30
updated: 2026-07-30
tags: [retro, resumption, docs]
depends_on: [resume-script]
---

## Goal

Turn the worker discipline that saved the second `loopback-only-net` attempt
into scheme guidance: commit `## Notes` findings incrementally *during
investigation*, not just code — so an interrupt is recoverable and the tech
lead has in-flight visibility via [[resume-script]] + `board.sh` without
disturbing the worker. Also formalize the leader re-dispatch technique:
pre-seed the design conclusion into `## Notes` before re-dispatching.

## Context

Retro: the re-dispatched worker committed incrementally, which is why it was
recoverable — but that was the worker's discipline, not the scheme's
enforcement. The leader also pre-seeded the approach-B conclusion
(loopback-only namespace + Unix-socket bridge) into the task's `## Notes`
and committed it *before* re-dispatching; the fresh worker implemented
instead of re-deriving the design — a large save worth formalizing.

This task is docs only: PROCESS.md worker workflow + SKILL.md worker
workflow. It is the "in-flight visibility" retro item too: incremental
`## Notes` commits + `board.sh`'s in-flight branch scan are the heartbeat —
no separate mechanism.

## Acceptance

- [ ] PROCESS.md "Execution (worker)" section adds: commit `## Notes`
  findings incrementally during investigation (even before code is final),
  so an interrupt is recoverable by [[resume-script]]; do not batch all
  notes to the final `review` commit.
- [ ] PROCESS.md adds a "Re-dispatching an interrupted worker" note (edge
  cases or execution): the leader inspects `resume.sh` output, writes
  any reached design conclusion into `## Notes`, commits it on the task
  branch, then re-dispatches — so the fresh worker implements instead of
  re-deriving.
- [ ] SKILL.md "Worker workflow" adds the incremental-commit bullet.
- [ ] No code changes; `run-tests.sh` stays green.

## Notes

- 2026-07-30 created. Depends on [[resume-script]] (both edit SKILL.md;
  this follows it) and transitively on [[cleanup-and-docs]] (PROCESS.md /
  SKILL.md are rewritten by the port first).
