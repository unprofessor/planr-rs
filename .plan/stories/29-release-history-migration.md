---
id: release-history-migration
aliases: [release-history-migration]
kind: story
parent: vcs-adapter-boundary
title: Migrate completed epics using release history
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [migration, releases, history]
depends_on: [git-source-adapter, milestone-placement-commands]
---

## Goal

Use Git history and release tags to identify completed epics, then migrate only
those closed hierarchies into completed milestone directories.

## Context

The audit must report evidence before changing paths. The current repository
has completed epics such as [[port-scripts-to-typescript]] and [[rust-port]],
but their target release milestone should be determined from history rather
than guessed. Applying a reviewed mapping performs filesystem moves only; the
user owns the VCS commit/rename operation.

## Acceptance

- [ ] Every migrated epic has an auditable completion commit and release
  mapping.
- [ ] Ambiguous or tagless history is reported for review rather than guessed.
- [ ] Only whole, closed epics are moved; partial active hierarchies remain
  untouched.
- [ ] Resulting completed milestones pass catalog and lint validation.

## Tasks

- [[release-history-audit]]
- [[closed-epic-migration]]
- [[migration-validation]]
