---
id: closed-epic-migration
aliases: [closed-epic-migration]
kind: task
parent: release-history-migration
title: Migrate closed epics into completed milestones
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [migration, milestones, filesystem]
depends_on: [release-history-audit, milestone-placement-commands, filesystem-source-adapter]
---

## Goal

Apply an explicitly reviewed release mapping by creating completed milestone
documents and moving only whole closed epic hierarchies into them.

## Acceptance

- [ ] The user supplies or approves the audit mapping before applying it.
- [ ] Milestone IDs are normalized to kebab-case (for example,
  `v0-1-0-release`).
- [ ] Only epics with all descendant stories/tasks done are moved.
- [ ] Moves use filesystem operations only; no `git mv`, commit, checkout, or
  branch operation is performed.
- [ ] Existing ticket contents, IDs, parent links, dependencies, and wiki-links
  remain unchanged.
- [ ] The operation supports dry-run and reports every path change.
- [ ] Unmapped or partially complete epics remain in the unplanned root.
