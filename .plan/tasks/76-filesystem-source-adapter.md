---
id: filesystem-source-adapter
aliases: [filesystem-source-adapter]
kind: task
parent: filesystem-vcs-mode
title: Operate on the working tree without a VCS backend
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [vcs, filesystem, portability]
depends_on: [vcs-source-contract, command-source-injection]
---

## Goal

Implement the source provider for local filesystem operation when no VCS is
available.

## Acceptance

- [ ] Board, lint, catalog, milestone creation, placement, and lifecycle work
  against a plain directory tree.
- [ ] The provider has no Git executable or repository requirement.
- [ ] Reads and writes use the same path and parse validation as the Git-backed
  working-tree path.
- [ ] Tests run in temporary directories without initializing a repository.
- [ ] Ref/branch operations are reported as unavailable rather than emulated.
