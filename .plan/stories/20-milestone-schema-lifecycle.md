---
id: milestone-schema-lifecycle
aliases: [milestone-schema-lifecycle]
kind: story
parent: milestone-scoped-backlog
title: Milestone documents and lifecycle invariants
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [milestones, format, lifecycle]
depends_on: []
---

## Goal

Represent milestones as Markdown documents with YAML frontmatter and enforce
their release lifecycle independently of ticket status.

## Context

A milestone lives at `.plan/milestones/<kebab-id>/milestone.md`. Its directory
is the structural parent of the epics beneath it; ticket frontmatter does not
repeat a milestone field. The supported lifecycle is `planned`,
`in_progress`, and `done`. At most one milestone may be `in_progress`.

## Acceptance

- [ ] Milestone documents have a documented frontmatter schema and a Markdown
  body for goals, exit criteria, and release notes.
- [ ] Milestone IDs use kebab-case, including normalized version names such as
  `v2-0-release`.
- [ ] Lifecycle transitions and the single-active invariant are validated.
- [ ] Closing a milestone gates on all child epics being done.
- [ ] No rule requires unplanned root tickets to be assigned to a milestone.

## Tasks

- [[milestone-record-format]]
- [[milestone-state-rules]]
- [[milestone-lifecycle-commands]]
