---
id: ticket-catalog
aliases: [ticket-catalog]
kind: task
parent: workspace-ticket-catalog
title: Build a filesystem catalog with derived milestone scope
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [catalog, filesystem, milestones]
depends_on: [milestone-record-format]
---

## Goal

Create one catalog API that discovers tickets in the unplanned root and all
milestone scopes without requiring a VCS.

## Context

Each catalog record needs the ticket path, kind, parsed ticket, and optional
milestone ID. The catalog must recurse only through recognized `epics/`,
`stories/`, and `tasks/` directories, while discovering sibling
`milestone.md` documents separately.

## Acceptance

- [ ] A catalog can scan a working tree and return all active and
  milestone-scoped ticket records.
- [ ] Root records have no milestone; nested records carry the normalized
  milestone ID derived from their path.
- [ ] Ticket paths and milestone paths are retained for later moves and views.
- [ ] `milestone.md` and unrelated Markdown notes are excluded from ticket
  records.
- [ ] Duplicate ticket IDs are reported rather than silently overwritten.
- [ ] Unit tests cover empty, root-only, and multi-milestone workspaces.
