---
id: retro-process-docs
aliases: [retro-process-docs]
kind: task
parent: retrospective-phase
title: Document the retrospective phase in PROCESS.md + SKILL.md
status: todo
assignee: null
created: 2026-07-30
updated: 2026-07-30
tags: [retro, process, docs]
depends_on: [retro-template-and-script]
---

## Goal

Document the retrospective phase: when to run one, who writes it, what it
covers, and how improvements feed back as new epics. Make the kind of
artifact that produced [[hotcell-firewall-hardening]] a normal part of the
process.

## Context

With [[retro-template-and-script]] providing the template + scaffolder, this
task writes the process prose. The hotcell retro is the canonical example to
reference. The key guidance from that retro: split findings by within-control
vs without-control (so the scheme owns its within-control failures and
doesn't over-claim credit for luck); priority-order improvements; each
improvement becomes a task under a hardening epic.

## Acceptance

- [ ] PROCESS.md gains a "Retrospective" section: **when** — after an epic
  closes, or after a story with notable events (a red trunk shipped, a
  worker died, a changes-requested cycle); **who** — the leader writes
  it (it's a leader artifact, like ticket creation); **what** — the
  template structure, emphasizing the within/without-control split; **how
  it feeds back** — priority-ordered improvements become tasks under a new
  hardening epic (cross-reference [[hotcell-firewall-hardening]] as the
  worked example).
- [ ] SKILL.md gains a short "Retrospective" subsection (under the workflow
  or as a peer to "Ticket format") pointing at `scripts/new-retro.sh` and
  the PROCESS.md section.
- [ ] The epic [[plan-retrospectives]] and its parent story
  [[retrospective-phase]] are referenced as the example of "improvements
  feed back as a hardening epic."
- [ ] No code changes; `run-tests.sh` stays green.

## Notes

- 2026-07-30 created. Depends on [[retro-template-and-script]] (both edit
  SKILL.md; this follows it and references the scaffolder it adds).
