---
id: port-scripts-to-typescript
aliases: [port-scripts-to-typescript]
kind: epic
title: Port plan skill scripts from bash/awk to TypeScript
status: done
assignee: null
created: 2026-07-30
updated: 2026-08-01
tags: [port, typescript, parser]
---

## Goal

Replace the fragile awk/sed/grep markdown+YAML parsing in `skills/planr/scripts/`
with a typed, tested TypeScript parser, while keeping the git orchestration thin
and the skill "copy the folder" portable. Eliminate the entire class of
silent-parsing-failure modes (--- toggling, block-style depends_on, unscoped
sed, untrimmed verdict) rather than patching each one.

## Scope

- New: `src/` (parser, lint logic, git wrappers, CLI), `tests/` (unit + fixtures),
  `dist/` (bundled output), `package.json`, `tsconfig.json`, build config.
- Ported: all six scripts (board, claim, lint, new-ticket, review, merge-task).
- Preserved: `templates/*.md` (unchanged), `SKILL.md`/`references/*` (minor
  edits to reflect new invocation + dropped block-style convention), the
  `tests/run-tests.sh` integration harness (kept green throughout).
- Out of scope: changing the ticket *format*, the process/roles, or the git
  workflow. Obsidian vault compatibility stays; we only stop relying on inline
  YAML.

## Out of scope

- A pre-commit hook for lint.sh (separate follow-up; tracked in
  [[port-scripts-to-typescript]] notes but not a port task).
- Publishing planr as an npm/git pi-package (the bundled dist already satisfies
  "copy the folder"; publishing is a later distribution choice).

## Notes

- Decision record (from the research pass): use `eemeli/yaml` for frontmatter
  (YAML 1.1+1.2, comment-preserving, ~25 kB gzip, no CVE), hand-roll body
  parsing (~50 lines: wiki-links, ## sections, verdict), esbuild-bundle to one
  `dist/cli.js`, ship compiled JS via `.sh` shims. Avoid gray-matter (stale
  since 2021, CVE-2025-64718) and markdown-it/remark (overkill, stale plugins).
- The port deletes an entire lint error class: block-style `depends_on` is
  parsed correctly by a real YAML lib, so the inline-only convention and the
  block-style check in lint.sh both go away. Update docs to match.
- Migration is bottom-up: parser foundation first (proven against fixtures
  incl. an Obsidian-reformatted file), then read-only scripts, then mutating
  git scripts last. `run-tests.sh` stays green at every step.
- Disagreement noted, not smoothed: the second researcher leaned toward
  js-yaml for the smaller bundle (~16 kB vs ~25 kB gzip); we chose eemeli/yaml
  for comment-preserving round-trip (useful when scripts write frontmatter
  back) and YAML 1.1 (closer to Obsidian). Revisit if bundle size matters.
