---
id: milestone-verification-docs
aliases: [milestone-verification-docs]
kind: story
parent: milestone-scoped-backlog
title: End-to-end coverage and user documentation for milestones
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [tests, docs, milestones]
depends_on: []
---

## Goal

Make milestone behavior regression-safe and document the release-cycle model
for leaders, workers, reviewers, and users operating without VCS integration.

## Acceptance

- [ ] Tests cover creation, activation, placement, completion, board views,
  linting, and completed-milestone history.
- [ ] Tests cover a repository with no milestones and a repository with several
  planned/completed milestones.
- [ ] Documentation distinguishes milestone assignment from archival.
- [ ] Documentation states that VCS move/commit semantics belong to the user.

## Tasks

- [[milestone-e2e]]
- [[milestone-docs]]
