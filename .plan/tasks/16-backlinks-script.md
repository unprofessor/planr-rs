---
id: backlinks-script
aliases: [backlinks-script]
kind: task
parent: utility-scripts
title: Add backlinks.sh to find wiki-link references to a slug
status: todo
assignee: null
created: 2026-07-30
updated: 2026-07-30
tags: []
depends_on: [cli-scaffolding]
---

## Goal

Add a `scripts/backlinks.sh` utility that finds all `.plan/` files wiki-linking to a given slug. Currently the skill documents `grep -rn '\[\[slug\]' .plan/` as the manual method — this wraps that in a proper script with `PLANR_DIR` support.

## Context

Parent story: [[utility-scripts]] under [[supplementary-tooling]]. The SKILL.md says “Backlinks are derived, never stored — same philosophy as roll-up: `grep -rn '\[\[http-connect-proxy' .plan/`”. A dedicated TS command resolves `.plan/` path from env vars, filters out frontmatter false positives, and integrates into the CLI.

Built on the TS CLI layer (`src/cli/`), not a standalone bash script, so it reuses the parser and avoids the grep-frontmatter-filtering fragility. Follow the same CLI pattern as the ported scripts (board, review, etc.).

## Acceptance

- [ ] `scripts/backlinks.sh <slug>` prints every `.plan/` file whose body contains `[[<slug>]]` or `[[<slug>|...]]` or `[[<slug>#...]]`, one per line, with file path
- [ ] Does NOT match frontmatter (e.g., `aliases: [slug]`, `parent: slug`, `depends_on: [slug]`) — only body markdown
- [ ] `scripts/backlinks.sh <slug> -v` also prints the matching line for context
- [ ] Honors `PLANR_DIR` env var (default `.plan`)
- [ ] Exits 0 if any backlinks found, 1 if none
- [ ] Included in `tests/run-tests.sh` with a fixture test
- [ ] Documented in SKILL.md scripts table

## Notes

- 2026-07-30 created
- Implement as TS: iterate `ParsedTicket[]` from the parser and check each body for `[[slug]]`, `[[slug|...]]`, `[[slug#...]]`
- `depends_on: [cli-scaffolding]` — needs the parser, git wrappers, and CLI layer
