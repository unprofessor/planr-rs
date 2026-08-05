---
id: done-with-waiver
aliases: [done-with-waiver]
kind: task
parent: review-and-waiver
title: Formalize the done-with-waiver path (## Waiver block + lint + board cue)
status: todo
assignee: null
created: 2026-07-30
updated: 2026-07-30
tags: [retro, waiver, lint, docs]
depends_on: [reviewer-flakiness-guidance]
---

## Goal

Make the honest partial-completion path first-class: a greppable `## Waiver`
block (reason, owner, follow-up) so a task can be `done` with an acceptance
box honestly unchecked without burying the waiver in story notes, plus a
lint warning and a board cue so `done` stays meaningful and waivers are
visible.

## Context

Retro: `firewall-tests` merged `done` with acceptance #2 unchecked (no real
API key). The path was proven (real Google 400 through the firewall + TLS)
and the reviewer agreed, but the waiver was ad hoc in story notes — so the
board's `done` was no longer a strict guarantee and the waiver wasn't
greppable. Formalize rather than punish honesty.

Design: a `## Waiver` body block (not a new status — keep `done` as the
lifecycle terminal). `lint.sh` warns when a task is `done` with an
unchecked `- [ ]` acceptance box AND no `## Waiver` block (the waiver is
the documented escape hatch; without it, an unchecked box at `done` is a
likely overclaim). `board.sh` marks a waived task's status `done*` so the
leader sees waivers at a glance.

## Acceptance

- [ ] TICKET-FORMAT.md documents the `## Waiver` block:
  ```markdown
  ## Waiver
  criterion: <the unchecked acceptance item, or its number>
  reason: <why it's waived>
  owner: <who follows up>
  follow-up: <slug of a follow-up task, or "none">
  ```
  Placed after `## Review`; one block per waived criterion.
- [ ] `lint.sh` / `src/lint.ts` warns (exit 0) when a task is `done`, has at
  least one `- [ ]` line in `## Acceptance`, and has no `## Waiver` block:
  `warning: <f>: done with unchecked acceptance but no ## Waiver block`.
  Does NOT error — a waiver is legitimate; this catches the accidental
  overclaim.
- [ ] `board.sh` / `src/board.ts` renders a `done` task that has a `##
  Waiver` block as `done*` (in the STATUS column) so waivers are visible on
  the board.
- [ ] PROCESS.md "Definition of done" notes the waiver path: `done` with an
  unchecked box requires a `## Waiver` block; the reviewer still must
  approve the waiver's reasoning.
- [ ] `run-tests.sh` gains: a `done` task with an unchecked box and a `##
  Waiver` block → lint clean (no warning); the same without `## Waiver` →
  lint warns (exit 0); board shows `done*` for the waived task.

## Notes

- 2026-07-30 created. Depends on [[reviewer-flakiness-guidance]] (both edit
  PROCESS.md; this follows it) and transitively on [[cleanup-and-docs]]
  (lint.ts/board.ts ported, docs rewritten by the port first).
