---
id: milestone-placement
aliases: [milestone-placement]
kind: story
parent: milestone-scoped-backlog
title: Assign and move epic hierarchies between scopes
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [milestones, filesystem, moves]
depends_on: []
---

## Goal

Let the leader assign an epic and its stories/tasks to a milestone or return
it to the unplanned root using filesystem-only operations.

## Context

Files remain flat within each `epics/`, `stories/`, and `tasks/` directory, so
moving an epic means discovering descendants through parent links and moving
each file to the corresponding destination directory. File contents and
identities remain unchanged.

## Acceptance

- [ ] Whole epic hierarchies can move between root and planned/active
  milestones.
- [ ] Placement never calls a VCS command or commits on the user's behalf.
- [ ] Destination paths preserve filenames, IDs, parent links, dependencies,
  and wiki-links.
- [ ] Completed milestones are not modified by ordinary placement commands.
- [ ] Failures do not leave a partially moved hierarchy.

## Tasks

- [[milestone-hierarchy-move]]
- [[milestone-placement-safety]]
- [[milestone-placement-commands]]
