---
id: plan-retrospectives
aliases: [plan-retrospectives]
kind: epic
title: Add a retrospective phase to the plan process
status: todo
assignee: null
created: 2026-07-30
updated: 2026-07-30
tags: [retro, process]
depends_on: [port-scripts-to-typescript]
---

## Goal

Make the kind of artifact that produced [[hotcell-firewall-hardening]] — a
structured retro under `.plan/retros/` — a first-class part of the plan
process, with a template, a scaffolder, and process documentation on when
to run one and how to feed improvements back as new epics.

## Scope

- **Retro template** — `templates/retro.md` with the structure that worked
  on the hotcell retro (within/without-control split, strengths, weaknesses
  priority-ordered, net).
- **Scaffolder** — `scripts/new-retro.sh` (shim over the ported CLI) that
  creates `.plan/retros/<NN>-<slug>.md`; `.plan/retros/` is not scanned by
  lint (retros aren't tickets).
- **Process docs** — a PROCESS.md "Retrospective" section (when/who/what/
  feed-back) + a SKILL.md mention, referencing the hotcell retro as the
  worked example.

## Out of scope

- **Hardening the tool from retro findings.** That's [[hotcell-firewall-hardening]];
  this epic only adds the *practice* of writing retrospectives, not the
  findings themselves.
- **Automating retro generation.** A retro is a leader-written artifact,
  not agent-generated; the scaffolder just creates the file.

## Notes

- 2026-07-30 created. Depends on [[port-scripts-to-typescript]]: the
  scaffolder reuses the ported `new-ticket.sh` pattern, and the doc edits
  (SKILL.md, PROCESS.md) overlap with the port's [[cleanup-and-docs]]
  rewrite — gating avoids doc-merge conflicts.
- The hotcell retro (`../hotcell/.plan/retros/01-planr-hotcell-firewall.md`)
  is the canonical example; [[hotcell-firewall-hardening]] is the worked
  example of "improvements feed back as a hardening epic."
