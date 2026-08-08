---
id: milestone-board-views
aliases: [milestone-board-views]
kind: task
parent: milestone-views-validation
title: Render current, upcoming, and completed milestone scopes
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [milestones, board, cli]
depends_on: [catalog-reader-integration, milestone-state-rules]
---

## Goal

Make the board useful as a release-cycle view while retaining explicit access
to every milestone.

## Acceptance

- [ ] The default board shows the current active milestone and unplanned root
  backlog, plus compact upcoming/completed summaries.
- [ ] A milestone selector renders one requested milestone in full.
- [ ] An all-scopes mode renders all milestone contents explicitly.
- [ ] Completed milestone contents are hidden from the default view without
  changing ticket status or files.
- [ ] Existing in-flight/workflow reporting remains accurate for the selected
  scope.
- [ ] Board summaries distinguish unplanned, current, planned, and completed
  counts.
