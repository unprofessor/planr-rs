---
id: milestone-state-rules
aliases: [milestone-state-rules]
kind: task
parent: milestone-schema-lifecycle
title: Validate milestone lifecycle and single-active invariant
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [milestones, lifecycle, validation]
depends_on: [milestone-record-format]
---

## Goal

Implement the lifecycle rules that make completed milestones an emergent
archive view and prevent multiple concurrent release cycles.

## Context

Supported states are `planned`, `in_progress`, and `done`. Starting a
milestone is an explicit decision; no state is inferred from child ticket
statuses. At most one milestone may be `in_progress`. Closing gates on all
child epics being `done`; unplanned tickets are not subject to a lifecycle
gate.

## Acceptance

- [ ] State transition validation supports planned → in_progress → done.
- [ ] Starting a milestone fails when another milestone is already active.
- [ ] Closing a milestone fails when any direct child epic is unfinished.
- [ ] A `done` milestone is exposed as completed/archive metadata without adding
  an `archived` ticket status or moving its files.
- [ ] The invariant is checked across all discovered milestone documents.
- [ ] Unit tests cover valid transitions, invalid transitions, multiple active
  milestones, and unfinished child epics.
