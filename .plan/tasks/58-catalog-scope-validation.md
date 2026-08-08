---
id: catalog-scope-validation
aliases: [catalog-scope-validation]
kind: task
parent: workspace-ticket-catalog
title: Validate milestone paths and hierarchy scope
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [catalog, lint, milestones]
depends_on: [ticket-catalog, milestone-state-rules]
---

## Goal

Add structural checks for the relationship between file placement, ticket
kind, and parent hierarchy.

## Acceptance

- [ ] Files under each recognized directory have the matching ticket kind.
- [ ] Milestone directory IDs match their `milestone.md` IDs and naming rules.
- [ ] An epic's stories and tasks share its milestone scope, or are all
  unplanned at the root.
- [ ] Parent lookup remains slug-based and may resolve across scopes for
  dependencies, while hierarchy split/orphan cases are errors.
- [ ] Global duplicate IDs, malformed milestone directories, and unrecognized
  plan paths produce actionable lint findings.
- [ ] Existing root-only lint fixtures remain valid.
