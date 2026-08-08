---
id: vcs-workflow-contract
aliases: [vcs-workflow-contract]
kind: task
parent: vcs-provider-contract
title: Define optional branch, worktree, and mutation capabilities
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [vcs, architecture, workflow]
depends_on: [vcs-source-contract]
---

## Goal

Define optional workflow capabilities for claim, review, close, and other
operations that require branches or commits.

## Acceptance

- [ ] Capabilities cover branch/worktree creation and discovery, status
  mutation, commit, merge, and cleanup as needed by current workflows.
- [ ] Each capability has an explicit unsupported/error result.
- [ ] The contract does not require every backend to emulate Git worktrees.
- [ ] Filesystem-only milestone moves are explicitly excluded.
- [ ] Contract tests cover a fully capable provider and a read-only provider.
