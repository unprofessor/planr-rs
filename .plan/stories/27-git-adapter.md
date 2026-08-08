---
id: git-adapter
aliases: [git-adapter]
kind: story
parent: vcs-adapter-boundary
title: Put current Git behavior behind the provider boundary
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [vcs, git, compatibility]
depends_on: [vcs-source-contract, vcs-workflow-contract, command-source-injection, command-workflow-injection]
---

## Goal

Implement the first provider using the existing Git wrappers and preserve the
current trunk, branch, worktree, review, and merge behavior.

## Acceptance

- [ ] Git snapshot and workflow operations implement the provider contracts.
- [ ] Existing commands retain their current output and safety gates.
- [ ] Git-specific paths and error handling are confined to the adapter.
- [ ] Existing end-to-end tests pass through the adapter.

## Tasks

- [[git-source-adapter]]
- [[git-workflow-adapter]]
- [[git-regression-suite]]
