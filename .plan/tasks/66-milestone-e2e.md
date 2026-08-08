---
id: milestone-e2e
aliases: [milestone-e2e]
kind: task
parent: milestone-verification-docs
title: Cover milestone workflows end to end
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [milestones, tests, e2e]
depends_on: [milestone-lifecycle-commands, milestone-placement-commands, milestone-board-views, milestone-lint-rules, catalog-command-integration]
---

## Goal

Add end-to-end coverage for milestone creation, placement, lifecycle, board
views, linting, and completed-milestone history.

## Acceptance

- [ ] A temporary workspace can create planned milestones and assign whole
  epics without a VCS.
- [ ] Starting a second active milestone fails.
- [ ] Closing an incomplete milestone fails; closing a complete one hides it
  from the default board while preserving explicit history access.
- [ ] Root-only repositories retain current behavior.
- [ ] Cross-milestone dependencies and global slug uniqueness are exercised.
- [ ] Filesystem move failures leave no silent partial hierarchy.
