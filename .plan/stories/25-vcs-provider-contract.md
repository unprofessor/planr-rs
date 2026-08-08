---
id: vcs-provider-contract
aliases: [vcs-provider-contract]
kind: story
parent: vcs-adapter-boundary
title: Define the VCS-neutral provider boundary
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [vcs, architecture, interfaces]
depends_on: [milestone-e2e]
---

## Goal

Define the capabilities planr may request from a VCS without baking Git names
or assumptions into the milestone and catalog layers.

## Context

Read-only source access and workflow mutations have different capability
requirements. The interface must support working-tree and snapshot reads while
allowing a backend to report that branches, worktrees, commits, or merges are
unavailable.

## Acceptance

- [ ] Source and workflow capabilities are distinct and independently
  testable.
- [ ] Interfaces use planr domain types and explicit capability errors rather
  than Git-specific command names.
- [ ] The milestone filesystem move remains outside the VCS interface.
- [ ] A future jj adapter can implement the boundary without changing catalog
  or milestone code.

## Tasks

- [[vcs-source-contract]]
- [[vcs-workflow-contract]]
- [[workspace-lock-abstraction]]
