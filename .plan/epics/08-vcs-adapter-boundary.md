---
id: vcs-adapter-boundary
aliases: [vcs-adapter-boundary]
kind: epic
title: VCS adapter boundary and Git backend
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [vcs, git, jj, architecture]
depends_on: [milestone-e2e]
---

## Goal

Separate planr's filesystem and backlog model from version-control-specific
operations. Define a VCS-neutral provider boundary, preserve current Git
behavior behind the first adapter, and leave room for jj or other backends.

## Scope

- A source provider for working-tree and committed/ref snapshots.
- Optional workflow capabilities for branches, worktrees, commits, merges, and
  other mutations that some backends may not support.
- A workspace-local or provider-neutral locking boundary.
- Git as the first provider, with regression coverage for current commands.
- A working-tree provider for read-only and filesystem-only operation without a
  VCS backend.
- A reviewed migration of closed epics into completed milestone directories,
  using Git history only to determine release mapping.

## Out of scope

- Implementing a jj backend in this epic.
- Making VCS rename detection part of milestone placement; moves remain ordinary
  filesystem operations.
- Guessing release assignments without an auditable history report.

## Stories

- [[vcs-provider-contract]] — provider interfaces, capabilities, and locking
- [[vcs-command-integration]] — inject providers into planr commands
- [[git-adapter]] — preserve existing behavior behind the Git adapter
- [[filesystem-vcs-mode]] — work without a VCS backend where possible
- [[release-history-migration]] — history-driven closed-epic migration
- [[vcs-adapter-docs]] — document the backend boundary and capabilities

## Notes

- 2026-08-08 created as a follow-on to [[milestone-scoped-backlog]].
- The first implementation target is a backend boundary plus Git adapter;
  additional VCS backends remain future work.
