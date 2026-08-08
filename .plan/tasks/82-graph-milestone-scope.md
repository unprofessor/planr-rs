---
id: graph-milestone-scope
aliases: [graph-milestone-scope]
kind: task
parent: graph-data-model
title: Carry milestone scope into the ticket graph
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [graph, milestones, catalog]
depends_on: [graph-construction, ticket-catalog, milestone-e2e]
---

## Goal

Extend the derived ticket graph to retain filesystem scope and represent the
implicit milestone-to-epic relationship.

## Context

Milestone membership is not stored in ticket frontmatter, so graph construction
must consume catalog records or an equivalent location-bearing input. The
existing ticket hierarchy and dependency edges remain intact.

## Acceptance

- [ ] Graph input can retain each ticket's optional milestone scope and source
  path without performing I/O or VCS operations.
- [ ] A milestone node/edge can represent the inferred milestone → epic parent
  relationship.
- [ ] Unplanned root epics have no artificial milestone parent.
- [ ] Cross-milestone dependency edges remain representable.
- [ ] Existing graph construction/query tests continue to pass.
- [ ] Construction remains pure once catalog records are supplied.
