---
id: git-source-adapter
aliases: [git-source-adapter]
kind: task
parent: git-adapter
title: Implement Git snapshot and branch source providers
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [vcs, git, source]
depends_on: [vcs-source-contract, command-source-injection]
---

## Goal

Adapt the existing Git list/show/ref/branch discovery wrappers to the
VCS-neutral source contract.

## Acceptance

- [ ] Git can provide working-tree and named-ref plan snapshots.
- [ ] Existing branch/in-flight discovery is available through the provider.
- [ ] Git command failures are converted to provider errors with useful output.
- [ ] No milestone or catalog code imports Git-specific helpers directly.
- [ ] Existing board, lint, and review read tests pass through the adapter.
