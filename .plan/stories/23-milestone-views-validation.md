---
id: milestone-views-validation
aliases: [milestone-views-validation]
kind: story
parent: milestone-scoped-backlog
title: Board, lint, and CLI behavior for milestone scopes
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [board, lint, cli, milestones]
depends_on: []
---

## Goal

Expose milestone scopes in the board and structural checks without treating
completed milestones as active clutter.

## Context

The default board should show the current milestone and unplanned backlog,
with compact upcoming/completed summaries. Explicit selectors should reveal a
specific milestone or all scopes. Lint must still inspect every scope, even
when the board hides completed milestone contents.

## Acceptance

- [ ] Board selection distinguishes unplanned, current, planned, and completed
  milestone scopes.
- [ ] Lint validates every milestone and cross-scope relationship.
- [ ] Exactly one `in_progress` milestone is accepted at most.
- [ ] Cross-milestone dependencies remain resolvable and done dependencies
  continue to satisfy claim gates.
- [ ] CLI help and errors explain scope selection and lifecycle behavior.

## Tasks

- [[milestone-board-views]]
- [[milestone-lint-rules]]
