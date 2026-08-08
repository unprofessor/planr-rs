---
id: git-workflow-adapter
aliases: [git-workflow-adapter]
kind: task
parent: git-adapter
title: Implement Git branch, worktree, merge, and mutation providers
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [vcs, git, workflow]
depends_on: [vcs-workflow-contract, git-source-adapter, command-workflow-injection, workspace-lock-abstraction]
---

## Goal

Put current Git claim/review/close workflow operations behind the provider
capabilities.

## Acceptance

- [ ] Branch creation, worktree setup, status flips, commits, merges, and
  cleanup implement the provider contract.
- [ ] Existing review approval and merge conflict behavior is preserved.
- [ ] Provider operations use the new lock abstraction.
- [ ] No Git operation is duplicated in milestone filesystem commands.
- [ ] Failures preserve the current recovery guidance where applicable.
