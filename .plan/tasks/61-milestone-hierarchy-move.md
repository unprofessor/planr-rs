---
id: milestone-hierarchy-move
aliases: [milestone-hierarchy-move]
kind: task
parent: milestone-placement
title: Move epic hierarchies with filesystem-only operations
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [milestones, filesystem, moves]
depends_on: [ticket-catalog, catalog-scope-validation]
---

## Goal

Implement the low-level operation that moves an epic and all descendant
stories/tasks between the unplanned root and a milestone directory.

## Acceptance

- [ ] Source and destination paths are computed from catalog records.
- [ ] Files are moved with ordinary filesystem operations; no VCS command,
  commit, checkout, branch, or merge is invoked.
- [ ] Filenames, frontmatter, bodies, IDs, dependencies, and wiki-links remain
  unchanged.
- [ ] Destination directories are created as needed and empty source
  directories are handled safely.
- [ ] A preflight prevents collisions and hierarchy splits before any move.
- [ ] Unit tests verify root-to-milestone, milestone-to-milestone, and
  milestone-to-root moves.
