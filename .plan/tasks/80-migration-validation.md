---
id: migration-validation
aliases: [migration-validation]
kind: task
parent: release-history-migration
title: Validate migrated milestones and references
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [migration, milestones, validation]
depends_on: [closed-epic-migration, milestone-lint-rules, milestone-board-views]
---

## Goal

Prove that the history-driven migration leaves a coherent backlog and useful
completed-milestone views.

## Acceptance

- [ ] Full catalog and lint scans pass after migration, aside from documented
  pre-existing warnings.
- [ ] Every migrated hierarchy remains internally scoped and globally unique.
- [ ] Parent, dependency, and wiki-link references resolve as before.
- [ ] Completed milestones appear in explicit history views and are absent from
  the default active board contents.
- [ ] A migration report records moved, unmapped, and intentionally untouched
  epics.
