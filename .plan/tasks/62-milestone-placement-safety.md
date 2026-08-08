---
id: milestone-placement-safety
aliases: [milestone-placement-safety]
kind: task
parent: milestone-placement
title: Add preflight, dry-run, and rollback safeguards
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [milestones, filesystem, safety]
depends_on: [milestone-hierarchy-move]
---

## Goal

Make multi-file milestone placement safe to review and recover from without
relying on VCS transactions.

## Acceptance

- [ ] Preflight reports every source/destination path and all blocking issues.
- [ ] A dry-run produces the exact planned move list without changing files.
- [ ] Files with `in_progress` or `review` status are rejected by default so
  active worker state is not relocated accidentally.
- [ ] A failed multi-file operation rolls back changes made by the operation or
  leaves an explicit recovery report.
- [ ] Completed milestones cannot be modified by ordinary placement commands.
- [ ] Tests cover collisions, active descendants, dry-run, and injected move
  failure.
