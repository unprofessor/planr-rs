---
id: workspace-lock-abstraction
aliases: [workspace-lock-abstraction]
kind: task
parent: vcs-provider-contract
title: Decouple plan locking from a Git common directory
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [vcs, locking, portability]
depends_on: [vcs-source-contract]
---

## Goal

Make planr's serialization lock available without requiring `.git` or another
specific VCS directory.

## Context

The current lock lives at `<git-common-dir>/planr.lock`. The replacement may
use a workspace-local lock or a provider-supplied lock, but must preserve the
shared/exclusive semantics needed by concurrent readers and writers.

## Acceptance

- [ ] Lock acquisition works in a plain filesystem workspace.
- [ ] Shared readers and exclusive writers retain current serialization
  behavior.
- [ ] Worktree-specific or backend-specific lock sharing remains possible for
  providers that need it.
- [ ] Existing Git lock interoperability is preserved or explicitly migrated.
- [ ] Tests cover lock path selection and concurrent acquisition.
