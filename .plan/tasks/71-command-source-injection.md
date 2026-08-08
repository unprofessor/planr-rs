---
id: command-source-injection
aliases: [command-source-injection]
kind: task
parent: vcs-command-integration
title: Route catalog and read commands through a source provider
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [vcs, catalog, commands]
depends_on: [vcs-source-contract, catalog-reader-integration]
---

## Goal

Inject the source provider into board, lint, review, and other read-side
commands instead of letting them call Git directly.

## Acceptance

- [ ] Working-tree and snapshot reads use the provider boundary.
- [ ] Board/lint can enumerate milestone paths from any supported source.
- [ ] Existing CLI output and root-only behavior remain unchanged for the Git
  provider.
- [ ] Unit tests can run with a fake source and no Git executable.
- [ ] Provider errors are surfaced with command context.
