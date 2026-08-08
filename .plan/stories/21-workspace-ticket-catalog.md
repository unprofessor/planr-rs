---
id: workspace-ticket-catalog
aliases: [workspace-ticket-catalog]
kind: story
parent: milestone-scoped-backlog
title: Central catalog for active and milestone-scoped tickets
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [catalog, filesystem, milestones]
depends_on: []
---

## Goal

Replace duplicated root-directory scans with one catalog that discovers ticket
files in the unplanned backlog and every milestone scope.

## Context

The catalog derives a ticket's location from its path and exposes both parsed
ticket data and path context. It must scan the filesystem without requiring a
VCS, while leaving ref/branch acquisition to the later provider boundary.

## Acceptance

- [ ] Active root tickets and nested milestone tickets are discoverable through
  one API.
- [ ] `milestone.md` files are parsed as milestones, not tickets.
- [ ] Ticket IDs remain globally unique across all scopes.
- [ ] Parent and dependency lookup can cross milestone boundaries.
- [ ] Existing root-only repositories retain their current behavior.

## Tasks

- [[ticket-catalog]]
- [[catalog-scope-validation]]
- [[catalog-reader-integration]]
- [[catalog-command-integration]]
