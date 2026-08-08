---
id: git-regression-suite
aliases: [git-regression-suite]
kind: task
parent: git-adapter
title: Preserve Git CLI behavior behind the adapter
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [vcs, git, tests]
depends_on: [git-workflow-adapter]
---

## Goal

Run the existing Git-backed CLI and end-to-end suite against the adapter and
add provider conformance coverage.

## Acceptance

- [ ] Existing Rust unit and end-to-end tests pass with the adapter enabled.
- [ ] Board, lint, new, claim, review, and close retain their documented
  behavior and output gates.
- [ ] Tests cover missing branches, merge conflicts, worktree cleanup, and
  unsupported/malformed snapshots.
- [ ] A Git-specific test fixture documents the adapter contract without
  leaking Git assumptions into core logic.
