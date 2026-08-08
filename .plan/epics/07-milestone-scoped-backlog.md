---
id: milestone-scoped-backlog
aliases: [milestone-scoped-backlog]
kind: epic
title: Milestone-scoped backlog and release lifecycle
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [milestones, releases, backlog, filesystem]
---

## Goal

Add release milestones as first-class filesystem scopes for the backlog. The
existing root `epics/`, `stories/`, and `tasks/` directories remain the
unplanned backlog; milestone directories group work committed to a release.
A completed milestone becomes the archive view without moving its files again.

## Scope

- `.plan/milestones/<kebab-id>/milestone.md` documents milestone metadata,
  lifecycle, goals, and exit criteria.
- Milestone membership is inferred from directory placement, not copied into
ticket frontmatter.
- Milestones move complete epic hierarchies using filesystem operations only.
- At most one milestone is `in_progress`; planned milestones may coexist.
- Board and lint operate across unplanned, active, planned, and completed
  milestone scopes.
- A central catalog supplies path and milestone context to current commands and
  the future ticket graph.

## Out of scope

- Git, jj, or other VCS rename/commit semantics; the user owns those operations.
- Requiring milestone assignment before claiming or closing work. Unplanned
  work remains allowed as an external-discipline choice.
- Automatic migration of the existing backlog until the later history-aware
  migration story.

## Stories

- [[milestone-schema-lifecycle]] — milestone document format and lifecycle
- [[workspace-ticket-catalog]] — scope-aware ticket discovery and validation
- [[milestone-placement]] — filesystem-only hierarchy placement operations
- [[milestone-views-validation]] — board, lint, and command behavior
- [[milestone-verification-docs]] — end-to-end coverage and documentation

## Notes

- 2026-08-08 created from the release-cycle/milestone design discussion.
