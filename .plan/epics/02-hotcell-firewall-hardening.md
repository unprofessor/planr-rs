---
id: hotcell-firewall-hardening
aliases: [hotcell-firewall-hardening]
kind: epic
title: Harden planr from the hotcell firewall retro findings
status: todo
assignee: null
created: 2026-07-30
updated: 2026-07-30
tags: [retro, hardening]
depends_on: [port-scripts-to-typescript]
---

## Goal

Feed the lessons from the first real planr run (the hotcell `network-firewall`
story, 4 tasks; retro at `../hotcell/.plan/retros/01-planr-hotcell-firewall.md`)
back into planr as concrete improvements to the merge gate, worker
resumption, review rigor, waiver handling, and self-tests.

## Scope

- **Merge-gate verification** — a per-project `verify` hook the merge gate
  runs after merge, so "reviewer approved" can no longer ship a red trunk.
- **Worker resumption** — a `resume.sh` that reconstructs in-flight state
  from the task file + git, plus a worker discipline of committing findings
  to the task file during investigation (so an interrupt is recoverable).
- **Reviewer flakiness guidance** — run suspected-flaky/network tests N
  times; "transient" is not an allowed verdict basis without evidence.
- **Formal done-with-waiver** — a `## Waiver` block convention (greppable,
  not buried in story notes) + a lint check + a board cue, so `done` stays
  meaningful when an acceptance box is honestly unchecked.
- **Self-test coverage** — close planr's own script self-test gaps (the
  `board.sh` `+`-prefix regression recurred after a skill rebuild because
  nothing tested it).

## Out of scope

- **Subagent cost rollup** (retro item 7) — harness concern, not planr.
  Noted in the retro; not tasked here.
- **Re-litigating the data model / role separation.** The retro found these
  sound (conflict-free merges, independent review caught a real fmt miss,
  honest partial completion). No changes.
- **Running the hotcell firewall story again.** This epic hardens the tool,
  it does not re-execute the mission that produced the retro.
- **The retrospective phase itself** — that's a separate epic,
  [[plan-retrospectives]], since it's a process addition rather than a
  fix to a specific finding.

## Notes

- 2026-07-30 created. Source: `../hotcell/.plan/retros/01-planr-hotcell-firewall.md`.
- **Sequencing:** this epic depends on [[port-scripts-to-typescript]]. The
  hardening lands on the ported TS foundation, not on the bash scripts it
  would otherwise delete. The port's closing task [[cleanup-and-docs]] is the
  keystone: every retro task that touches `PROCESS.md` / `SKILL.md` /
  `TICKET-FORMAT.md` / a ported `*.ts` gates on it, so no retro edit races a
  port doc rewrite. The highest-priority fix (the verify hook) is therefore
  sequenced behind the port by design — acceptable because no active mission
  is at risk (planr is not currently running the hotcell firewall); if the
  port stalls, reconsider shipping the verify hook directly to the bash
  `merge-task.sh` as a stopgap.
- Two headline failures from the retro, for easy reference: (1) the merge
  gate trusts the reviewer, not the build — [[verify-hook]]; (2) no
  resumption protocol for interrupted workers — [[resume-script]] +
  [[incremental-progress-guidance]].
- Within-story task ordering is set by `depends_on` where two tasks would
  otherwise edit the same doc file (PROCESS.md / SKILL.md) and conflict at
  merge. Sequencing upfront avoids wasted rebase work; it is a planning
  decision, not a missed-parallelism signal.
