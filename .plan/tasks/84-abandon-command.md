---
id: abandon-command
aliases: [abandon-command]
kind: task
parent: ticket-abandonment
title: Add the abandon command and abandoned ticket state
status: review
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

## Validation

- `cargo fmt --check` — passed.
- `cargo test` — passed: 110 unit tests and 18 end-to-end tests.
- `cargo clippy --all-targets --all-features` — passed with existing warnings
  in legacy date-range assertions and pre-existing e2e borrows; no warnings
  from the abandonment implementation.
- `git diff --check` — passed.

## Review

verdict: approved
reviewer: The Clanker (manual fallback)
date: 2026-08-11

Re-checked the acceptance criteria against the diff and ran the Rust unit and
end-to-end suites independently. The command accepts both ticket-level kinds,
records the requested reason/state, refuses active branches without cleanup,
keeps abandoned dependencies blocked, and leaves the existing close review
gates intact. README usage and board/lint behavior are covered. `cargo test`
passes (110 unit, 18 e2e); clippy exits successfully with only pre-existing
warnings outside the implementation.

## Notes

- 2026-08-11 created
- 2026-08-11 implemented `abandon` as a separate trunk-local workflow with
  `abandoned` status and `reason` frontmatter; active task branches are left
  untouched and reported as a refusal.
