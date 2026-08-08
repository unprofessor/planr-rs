---
id: milestone-lifecycle-commands
aliases: [milestone-lifecycle-commands]
kind: task
parent: milestone-schema-lifecycle
title: Add create, list, start, and close milestone commands
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [milestones, cli]
depends_on: [milestone-state-rules, ticket-catalog]
---

## Goal

Expose milestone lifecycle operations through an explicit CLI surface.

## Context

Use commands equivalent to `planr milestone create`, `list`, `start`, and
`close`. These commands update milestone documents and directories only; they
do not commit, checkout, branch, merge, or invoke a VCS.

## Acceptance

- [ ] `create` validates a kebab-case ID, creates the directory tree, and
  writes `milestone.md` with status `planned`.
- [ ] `list` reports all milestones with status and useful counts.
- [ ] `start` enforces the single-active invariant.
- [ ] `close` enforces the child-epic completion gate and sets status `done`.
- [ ] Commands provide actionable errors for missing, malformed, or completed
  milestones.
- [ ] Command tests cover a no-VCS temporary workspace.
