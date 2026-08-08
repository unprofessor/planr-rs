---
id: filesystem-capability-errors
aliases: [filesystem-capability-errors]
kind: task
parent: filesystem-vcs-mode
title: Handle unavailable workflow capabilities clearly
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [vcs, filesystem, errors]
depends_on: [filesystem-source-adapter, command-workflow-injection]
---

## Goal

Make no-VCS behavior explicit for commands that need branches, worktrees,
review branches, or merges.

## Acceptance

- [ ] `claim`, branch-backed `review`, and branch-backed `close` report the
  missing capability and a useful next step.
- [ ] Read-only and filesystem-only commands continue to work.
- [ ] No command creates fake branch state or claims work silently.
- [ ] Error messages identify the required capability rather than naming an
  unavailable implementation detail.
- [ ] Tests cover each unsupported workflow operation.
