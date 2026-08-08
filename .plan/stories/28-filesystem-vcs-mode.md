---
id: filesystem-vcs-mode
aliases: [filesystem-vcs-mode]
kind: story
parent: vcs-adapter-boundary
title: Support working-tree operation without a VCS backend
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [vcs, filesystem, portability]
depends_on: [vcs-source-contract, command-source-injection, command-workflow-injection]
---

## Goal

Provide useful working-tree behavior when no VCS is installed or configured,
while making unsupported workflow operations explicit.

## Acceptance

- [ ] Board, lint, catalog, and filesystem-only milestone operations work from
  the local tree without Git.
- [ ] Commands requiring branches, worktrees, or merges return actionable
  capability errors.
- [ ] No command silently pretends a VCS mutation succeeded.
- [ ] The provider boundary remains compatible with a future jj backend.

## Tasks

- [[filesystem-source-adapter]]
- [[filesystem-capability-errors]]
