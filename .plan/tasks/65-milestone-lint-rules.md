---
id: milestone-lint-rules
aliases: [milestone-lint-rules]
kind: task
parent: milestone-views-validation
title: Lint all milestone scopes and cross-scope references
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [milestones, lint, validation]
depends_on: [catalog-scope-validation, catalog-reader-integration, milestone-state-rules]
---

## Goal

Extend structural linting to cover milestone documents, placement rules, and
relationships across scopes.

## Acceptance

- [ ] All milestone documents and their ticket trees are linted, including
  completed milestones hidden by the board.
- [ ] More than one active milestone is an error.
- [ ] Directory/kind mismatches, milestone ID mismatches, hierarchy splits,
  duplicate IDs, dangling parents, dependencies, and links are reported.
- [ ] Cross-milestone `depends_on` references are valid and a done dependency
  remains satisfied.
- [ ] Lint does not require a VCS or a milestone assignment for root tickets.
- [ ] Existing lint warnings and root-only fixtures remain compatible.
