---
id: allow-parentless-tickets
aliases: [allow-parentless-tickets]
kind: task
parent: port-scripts
title: "Allow parentless stories/tasks as intentional roots (explicit parent: null)"
status: todo
assignee: null
created: 2026-07-30
updated: 2026-07-30
tags: [data-model, parent, lint, docs]
depends_on: [cleanup-and-docs]
---

## Goal

Relax the parent requirement so a story or task can be an intentional root:
`parent: null` (explicit) is allowed and means "this ticket is the head of
its own tree." Absent parent stays a lint error (likely typo). A story/task
with `parent: null` shows on the board with `-` parent and participates in
roll-up as a root.

## Context

Today `lint.sh` errors on any story/task whose `parent` is missing OR `null`,
and `new-ticket.sh` makes the parent arg mandatory for stories/tasks. The
rationale was "roll-up is derived by scanning children, so a parentless child
would vanish." But the data model doesn't require epics as the only roots:
`board.sh` renders by directory (a parentless node shows with `-` parent, it
doesn't disappear), and derivation can head at "the head of the tree" — an
epic OR an explicit-`null` root — rather than only at epics. The strictness
is convention/guardrail, not a hard constraint.

**Key guardrail to keep:** distinguish *explicit* `parent: null`
(intentional root → allowed) from an *absent* `parent` field (likely typo →
stays a loud error). This is a one-branch change in the lint parent check,
not a removal of the check.

This is a **behavior change**, not part of the behavior-preserving port.
[[port-lint]] ports the current strict behavior (null and absent both error)
faithfully; this task relaxes it on the ported foundation. Sequenced behind
[[cleanup-and-docs]] (the port's keystone) so it edits the final TS
`lint.ts` / `new-ticket.ts` and the already-rewritten docs — no bash touched
twice, no doc-merge race. `cleanup-and-docs` transitively covers
[[port-lint]] and [[port-new-ticket]], so this single dep covers all code
and doc edits. `board.ts` needs **no** change: it renders by directory, so
parentless nodes already appear with `-` parent.

## Acceptance

- [ ] `src/lint.ts` (`checkBacklog`): for a story/task, an **absent**
  `parent` field is still an error (`<f>: a <kind> must name a parent slug
  (or parent: null for an intentional root)`); an explicit `parent: null`
  is **accepted** (no error, no warning) — the ticket is an intentional
  root. Epic-with-parent stays an error. Dangling non-null parent stays an
  error; wrong-kind parent stays a warning.
- [ ] `src/cli/new-ticket.ts`: the parent arg is **optional** for stories
  and tasks. Omitted → writes `parent: null` into the new file. Provided →
  validated as today (must exist, any kind). Epics: parent arg ignored (as
  today). The template substitution renders `null` for an omitted parent
  (not an empty string).
- [ ] `TICKET-FORMAT.md` `parent` row: "ABSENT for epics. For stories &
  tasks, the parent slug, **or `null` to mark an intentional root**
  (derivation heads at the root, not only at epics); omitting the field is
  a lint error." Status lifecycle note: a root story/task rolls up as its
  own head.
- [ ] `PROCESS.md` "Dependencies"/roll-up note: derivation heads at the
  head of the tree — an epic or an explicit-`null` root story/task — not
  only at epics. `board.sh`/`lint.sh` behavior unchanged for parentless
  roots (board shows `-` parent; lint accepts explicit null).
- [ ] `run-tests.sh`: (a) a story with `parent: null` → `lint.sh` clean
  (exit 0, no parent error); (b) a story with no `parent` field → lint
  errors (exit 1); (c) `new-ticket.sh story rooty "Root story"` (no parent
  arg) creates `.plan/stories/NN-rooty.md` with `parent: null`, stdout one
  line; (d) a task with `parent: null` and a child task pointing at it →
  the child resolves and lint is clean (proves null-root roll-up works).
- [ ] No change to `board.ts`, `claim.ts`, `merge-task.ts`, `review.ts`, or
  `depends_on` handling — confirm by grep that no parent-strictness remains
  outside `lint.ts` / `new-ticket.ts`.

## Notes

- 2026-07-30 created. Behavior change riding the port foundation; depends
  on [[cleanup-and-docs]] (transitively [[port-lint]] + [[port-new-ticket]]
  - the doc rewrite). `board.ts` is untouched — it already renders by
  directory.
- If the port stalls and parentless tickets are needed sooner, a bash
  stopgap under [[supplementary-tooling]] is the escape hatch (edit `lint.sh` +
  `new-ticket.sh` directly); default is port-time to avoid touching the
  bash the port deletes.
