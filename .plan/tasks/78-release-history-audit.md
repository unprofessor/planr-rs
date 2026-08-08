---
id: release-history-audit
aliases: [release-history-audit]
kind: task
parent: release-history-migration
title: Map closed epics to release tags and commits
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [migration, git, history, releases]
depends_on: [git-source-adapter, milestone-lifecycle-commands]
---

## Goal

Produce an auditable mapping from currently closed epics to the release tag or
historical milestone they belong to.

## Context

Use commit history, file history, completion commits, and available release
tags. The audit must distinguish evidence from inference. It should identify
ambiguous/tagless cases for explicit user review instead of assigning them
silently.

## Acceptance

- [ ] Every `done` epic is listed with its completion evidence.
- [ ] Candidate release tags and commits are reported for each epic.
- [ ] Ambiguous, tagless, or conflicting histories are called out separately.
- [ ] The report is reviewable before any file moves occur.
- [ ] The current repository's closed epics, including
  `port-scripts-to-typescript` and `rust-port`, are covered.
- [ ] The audit makes no filesystem or VCS mutations.
