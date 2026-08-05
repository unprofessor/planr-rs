---
id: retro-template-and-script
aliases: [retro-template-and-script]
kind: task
parent: retrospective-phase
title: Add a retro template + new-retro.sh scaffolder
status: todo
assignee: null
created: 2026-07-30
updated: 2026-07-30
tags: [retro, template, scaffolding]
depends_on: [cleanup-and-docs]
---

## Goal

Add a `templates/retro.md` starter and a `scripts/new-retro.sh` scaffolder
that creates `.plan/retros/<NN>-<slug>.md`, so a retro is a one-command
artifact with a consistent structure.

## Context

The hotcell retro (`../hotcell/.plan/retros/01-planr-hotcell-firewall.md`)
is the prototype. Its structure worked: **What went well** split
within-control vs without-control, **What went poorly** split the same way,
**Strengths**, **Weaknesses / suggested improvements** (priority-ordered),
**Net**. That structure is the template.

A retro is not a ticket: no `parent`, no `kind`, no claim/review/done
lifecycle. So it gets its own scaffolder (not `new-ticket.sh` with a new
kind) and its own directory `.plan/retros/`. `new-retro.sh` allocates the
next `NN` within `retros/`, copies the template with `<slug>`/`<title>`/`<date>`
substituted, prints the path, and runs `lint.sh` informationally (lint
should ignore `retros/` — retros aren't tickets; confirm `lint.sh`'s
`list_files` only scans epics/stories/tasks).

## Acceptance

- [ ] `skills/planr/templates/retro.md` has the structure: frontmatter
  (`id`, `aliases`, `title`, `created`, `updated`, `tags`, a `retro_for:`
  field naming the epic/story it covers) + body sections: `## Context`,
  `## What went well — within control`, `## What went well — without
  control`, `## What went poorly — within control`, `## What went poorly —
  without control`, `## Strengths`, `## Weaknesses / suggested improvements`,
  `## Net`.
- [ ] `scripts/new-retro.sh` (shim over `src/cli/new-retro.ts`): allocates
  next `NN` in `.plan/retros/`, slug-validated (same kebab regex as
  `new-ticket.sh`), substitutes placeholders, writes
  `.plan/retros/<NN>-<slug>.md`, prints the path (one line, stdout), runs
  `lint.sh` informationally on stderr. Refuses an existing slug.
- [ ] `lint.sh` / `src/lint.ts` does NOT scan `.plan/retros/` (retros are
  not tickets); confirm via a test that a retro file with a `[[bad-link]]`
  and no frontmatter `kind` produces no lint errors/warnings.
- [ ] `run-tests.sh` gains: `new-retro.sh hotcell-firewall "hotcell firewall
  story"` creates `.plan/retros/01-hotcell-firewall.md`, stdout is one line,
  bad slug refused, duplicate slug refused.
- [ ] SKILL.md scripts table gains a `new-retro.sh` row.

## Notes

- 2026-07-30 created. Depends on [[cleanup-and-docs]] (new SKILL.md row +
  reuses the ported scaffolding pattern from [[port-new-ticket]]).
