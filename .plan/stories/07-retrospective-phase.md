---
id: retrospective-phase
aliases: [retrospective-phase]
kind: story
parent: plan-retrospectives
title: Add a retrospective phase to the plan process
status: todo
assignee: null
created: 2026-07-30
updated: 2026-07-30
tags: [retro, process]
depends_on: []
---

## Goal

Make the kind of artifact that produced [[hotcell-firewall-hardening]] — a
structured retro under `.plan/retros/` — a first-class part of the plan
process, with a template, a scaffolder, and a PROCESS.md section on when to
run one and how to feed improvements back as new epics.

## Context

The hotcell retro (`../hotcell/.plan/retros/01-planr-hotcell-firewall.md`)
was written ad hoc against an early planr. Its structure worked well: What
went well / poorly split by within-control vs without-control, Strengths,
Weaknesses/suggested improvements (priority-ordered), Net. That structure is
the template. A retro is not a ticket — it has no `parent`, no
claim/review/done lifecycle — so it gets its own lightweight scaffolder
(`new-retro.sh`) and its own directory (`.plan/retros/`), not a `kind:` in
the ticket schema.

## Acceptance

- [ ] [[retro-template-and-script]] merged: `templates/retro.md` +
  `scripts/new-retro.sh` (or TS equivalent) scaffolding
  `.plan/retros/<NN>-<slug>.md`, with a SKILL.md script-table row.
- [ ] [[retro-process-docs]] merged: PROCESS.md "Retrospective" section
  (when: after an epic closes, or after a story with notable events; how:
  leader writes it, covers within/without control, priority-orders
  improvements) + SKILL.md mention, referencing the hotcell retro as the
  example.

## Notes

- 2026-07-30 created. Two tasks; [[retro-process-docs]] follows
  [[retro-template-and-script]] (both edit SKILL.md).
