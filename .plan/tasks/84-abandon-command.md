---
id: abandon-command
aliases: [abandon-command]
kind: task
parent: ticket-abandonment
title: Add the abandon command and abandoned ticket state
status: in_progress
assignee: null
created: 2026-08-11
updated: 2026-08-11
tags: []
depends_on: []
---

## Goal

Add an explicit `planr abandon` workflow for OBE and won't-do tickets. It must
bypass review without weakening the normal `close` gate or dependency safety.

## Context

The existing `close task` workflow is branch-backed and requires
`status: review` plus an independently recorded `verdict: approved`; stories
and epics close on trunk only after all children are `done`. Abandonment is a
separate, trunk-local operation for any ticket kind:

```text
planr abandon task obsolete-task --reason obe
planr abandon story postponed-story --reason wont-do
```

It records `status: abandoned` and a frontmatter `reason` value (`obe` or
`wont-do`), updates the date, and commits directly on trunk. It must not
satisfy `depends_on`: claim continues to require dependency status `done`, so
an abandoned dependency blocks its dependents. An existing `plan/<slug>`
branch is an active claim/review and must make the command refuse rather than
silently discard or merge work.

## Acceptance

- [ ] CLI accepts `abandon <kind> <slug> --reason <obe|wont-do>` for `task`,
  `story`, and `epic`; reason is required and invalid reasons fail clearly.
- [ ] Abandonment finds the ticket on trunk, requires no review or worktree,
  writes `status: abandoned`, `reason: <value>`, and `updated: <local date>`
  in frontmatter, and commits the change on trunk with a one-line success
  message.
- [ ] Existing `plan/<slug>` branches cause a refusal before any mutation;
  neither the branch nor its worktree is removed or merged.
- [ ] Re-abandoning an already abandoned ticket is rejected rather than
  silently rewriting its recorded reason.
- [ ] `close task` still requires `status: review` and an approved review;
  `close story`/`close epic` still require all children to be `done`.
- [ ] `abandoned` is a valid lint status and is rendered/countable on the
  board; `claim` continues to treat it as an unfinished dependency (only
  `done` unblocks).
- [ ] Unit and end-to-end tests cover all reason/kind paths, dependency
  blocking, active branches, and the unchanged review gate.
- [ ] README usage and command details document the abandon workflow and its
  non-destructive active-branch refusal.

## Notes

- 2026-08-11 created
