---
id: ticket-abandonment
aliases: [ticket-abandonment]
kind: story
parent: hotcell-firewall-hardening
title: Abandon tickets without review while preserving dependency safety
status: todo
assignee: null
created: 2026-08-11
updated: 2026-08-11
tags: []
depends_on: []
---

## Goal

Provide an explicit, auditable path for tickets that are overtaken by events
(OBE) or intentionally will not be done, without pretending they passed code
review or unblocking work that still depends on them.

## Context

The normal task lifecycle requires a claimed `plan/<slug>` branch, worker
validation, and an independent review before `planr close task` can merge it.
That is the right gate for implemented work, but it is the wrong workflow for
work that should be abandoned. The new path uses a separate `abandon` command,
records an `abandoned` terminal state plus a short reason, and deliberately
keeps abandoned dependencies blocking.

## Acceptance

- [ ] `planr abandon <kind> <slug> --reason <obe|wont-do>` is documented and
  works for tasks, stories, and epics without requiring a review verdict.
- [ ] Abandonment is auditable in frontmatter (`status: abandoned`,
  `reason: obe|wont-do`, refreshed `updated` date) and creates a trunk commit.
- [ ] An existing `plan/<slug>` branch is never merged or discarded silently;
  the command refuses and tells the user to clean it up first.
- [ ] Abandoned tickets are visible on the board and do not satisfy
  `depends_on`; dependent tasks remain unclaimable until their dependency is
  changed or they are abandoned too.
- [ ] Existing `close` review gates remain unchanged, and lint accepts the new
  status.
- [ ] CLI, lifecycle, dependency, active-branch, and documentation coverage
  is tested.

## Notes

- 2026-08-11 created
