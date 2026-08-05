---
id: port-scripts
aliases: [port-scripts]
kind: story
parent: port-scripts-to-typescript
title: Port the six scripts onto the TS foundation
status: done
assignee: null
created: 2026-07-30
updated: 2026-08-01
tags: [port, scripts]
depends_on: [cli-scaffolding]
---

## Goal

Move all six scripts' logic from bash/awk onto the typed parser + git wrappers,
bottom-up by risk: lint (pure logic, highest parsing surface) → board + review
(read-only) → new-ticket (template write) → claim + merge-task (mutating git,
highest risk). Each task keeps `run-tests.sh` green and is independently
mergeable.

## Context

The scout inventory is the spec: every awk/sed site maps to a parser call.
Six load-bearing silent-failure modes get fixed by construction (real frontmatter
parser, scoped writes, trimmed values, insert-if-absent) — call them out in each
task's acceptance. The block-style `depends_on` lint check is DELETED (a real
YAML lib parses block-style correctly); the inline-only docs convention goes
too. Two bash inconsistencies to normalize: `board.sh`'s `fm_field` didn't trim
trailing whitespace (lint.sh did) — trim everywhere; mutations were
replace-only (silent no-op if field absent) — insert-if-absent.

## Acceptance

- [ ] All six scripts ported per their tasks; `run-tests.sh` passes 40/40
  throughout (each task lands green).
- [ ] The block-style `depends_on` error class is removed from lint AND from
  `run-tests.sh` (the test that asserts the error is deleted, replaced by a
  test asserting block-style parses correctly and gating still works).
- [ ] No `fm_field`/`fm_list`/inline-awk frontmatter parser remains anywhere in
  `skills/planr/` (grep-clean).
- [ ] `SKILL.md` and `references/TICKET-FORMAT.md`/`PROCESS.md` updated:
  script table notes TS implementation, the inline-only `depends_on`
  convention is removed, the "block-style is not parsed" warning is removed.

## Notes

- 2026-07-30 created. Depends on [[cli-scaffolding]]. The seven tasks under
  this story are ordered by `depends_on` and by risk; don't skip the ordering.
