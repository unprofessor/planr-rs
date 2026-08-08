---
id: vcs-command-integration
aliases: [vcs-command-integration]
kind: story
parent: vcs-adapter-boundary
title: Route planr commands through VCS capabilities
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [vcs, cli, architecture]
depends_on: [vcs-source-contract, vcs-workflow-contract]
---

## Goal

Make command orchestration consume the provider boundary instead of directly
assuming Git operations.

## Context

Catalog and milestone logic should work from a filesystem source. Commands
that require branch/worktree/merge semantics should request those capabilities
explicitly and fail clearly when a backend cannot provide them.

## Acceptance

- [ ] Read commands can be supplied a source provider.
- [ ] Workflow commands request only the capabilities they need.
- [ ] Direct Git calls are isolated behind the provider implementation.
- [ ] Existing Git-backed behavior remains available through dependency
  injection.

## Tasks

- [[command-source-injection]]
- [[command-workflow-injection]]
