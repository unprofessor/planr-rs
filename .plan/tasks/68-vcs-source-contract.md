---
id: vcs-source-contract
aliases: [vcs-source-contract]
kind: task
parent: vcs-provider-contract
title: Define a VCS-neutral snapshot and source interface
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [vcs, architecture, source]
depends_on: [milestone-e2e]
---

## Goal

Define the read-side provider contract that supplies a working-tree or
committed/ref snapshot to the catalog and read commands.

## Acceptance

- [ ] The interface can list and read plan files from a working tree or named
  snapshot without exposing Git terminology.
- [ ] Source errors distinguish missing snapshots, unreadable files, and
  unsupported operations.
- [ ] The catalog and milestone filesystem logic depend only on the contract,
  not a concrete VCS.
- [ ] An in-memory/fake source supports unit tests.
- [ ] Existing Git wrappers are not removed until the adapter tasks consume the
  contract.
