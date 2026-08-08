---
id: catalog-reader-integration
aliases: [catalog-reader-integration]
kind: task
parent: workspace-ticket-catalog
title: Use the catalog for board and lint inputs
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [catalog, board, lint]
depends_on: [ticket-catalog]
---

## Goal

Refactor read-side board and lint input collection to consume the central
catalog rather than hard-coded root-directory scans.

## Context

This task is the read-path seam for milestone support. It should preserve the
existing root-only output until milestone-specific rendering is added, while
making all scopes available to downstream code.

## Acceptance

- [ ] Working-tree board and lint readers use catalog records.
- [ ] Ref/snapshot readers have a path-aware input shape that can represent
  milestone directories without embedding Git operations in the catalog.
- [ ] Existing root-only board and lint tests continue to pass.
- [ ] Catalog parse errors are surfaced consistently instead of silently
  dropping files.
- [ ] No VCS command is introduced into the catalog implementation.
