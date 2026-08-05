---
id: cleanup-and-docs
aliases: [cleanup-and-docs]
kind: task
parent: port-scripts
title: Remove dead bash parsers, delete block-style convention, update docs
status: done
assignee: null
created: 2026-07-30
updated: 2026-08-01
tags: [cleanup, docs]
depends_on: [port-merge-task]
---

## Goal

Close out the port: delete every remaining awk/sed frontmatter parser, remove
the now-obsolete block-style `depends_on` convention from docs and tests, and
update SKILL.md/references to reflect the TS implementation.

## Context

With all six scripts ported, nothing should depend on inline-only YAML. The
block-style lint error and its test go away (a real YAML lib parses block-style
correctly). SKILL.md's script table and the "block-style is not parsed"
warnings in TICKET-FORMAT.md/PROCESS.md are stale. The "Extracting this skill"
section should note the build step (`npm run build` produces `dist/`) while
reaffirming copy-folder portability (dist is committed/bundled, no npm install
needed at the target).

## Acceptance

- [x] `grep -rn 'fm_field\|fm_list\|awk .*---\|perl -i -pe' skills/planr/` is
  clean — no bash frontmatter parser remains anywhere.
- [x] The block-style `depends_on` error class is gone from lint.ts and from
  `run-tests.sh` (test rewritten in [[port-lint]]; confirmed — rewritten to
  assert block-style lints clean).
- [x] TICKET-FORMAT.md: remove "always write the inline `[a, b]` form —
  block-style YAML is not parsed" from the `depends_on` row; note any valid
  YAML list form works.
- [x] PROCESS.md: remove the block-style bullet from the lint list; keep the
  dangling-slug/cycle/duplicate checks.
- [x] SKILL.md: script table notes the TS implementation (scripts are shims
  over `dist/cli/*.js`); "Extracting this skill" notes `npm run build` is
  dev-time and the shipped `dist/` is self-contained.
- [x] `run-tests.sh` passes 40/40 (or the new total after test additions); no
  test references the old bash parsers.
- [x] A final `npm run build && ./scripts/lint.sh && ./scripts/board.sh`
  smoke test in a clean clone (no `node_modules`) confirms the bundled dist is
  self-contained.

## Validation

All checks performed in worktree at `/home/exfed/projects/wt-cleanup-and-docs`:

1. **grep-clean** — `grep -rln 'fm_field\|fm_list' skills/planr/` returns
   nothing (exit 1). All six `skills/planr/scripts/*.sh` converted to shims
   (`exec node "$(dirname "$0")/../dist/cli/<name>.cjs" "$@"`); `_lock.sh`
   deleted (TS handles locking via spawned flock).
2. **Build fix (latent bug)** — the build had `--external:yaml` so real ported
   CLIs would `require('yaml')` at runtime (only stub CLIs passed the old
   smoke test). Removed `--external:yaml`; yaml is now bundled into each CLI
   (~270KB each). Verified no `require('yaml')` in `skills/planr/dist/cli/*.cjs`.
3. **Outdir moved into skill folder** — `npm run build` now outputs to
   `skills/planr/dist/cli/` so the shipped skill folder is self-contained
   (scripts + dist together). Root `scripts/*.sh` shims point at
   `../skills/planr/dist/cli/<name>.cjs`. `.gitignore` `dist/` pattern already
   covers `skills/planr/dist/`.
4. **Clean clone smoke test (no node_modules)** — copied `skills/planr/` to
   a throwaway dir (git init + .plan/ from trunk), removed all node_modules:
   `./skill/scripts/board.sh` exits 0 with correct epics/stories/tasks/summary
   sections; `./skill/scripts/lint.sh` exits 1 with the same 1 error + 19
   warnings as the in-repo run. Self-containment proven.
5. **Tests** — `npm test` 105/105 passing (unit), `run-tests.sh` 49/49
   passing (integration) including the rewritten block-style test
   ("block-style depends_on lints clean"). CLI test paths updated from
   `dist/cli/` to `skills/planr/dist/cli/` (claim/merge-task/review tests).
6. **Cleanup** — removed leftover `src/cli/git-stub.ts` (obsolete scaffolding
   barrel; real CLIs import git.ts directly) and its built artifact.
7. **Docs** — TICKET-FORMAT.md depends_on row now says any valid YAML list
   form works; PROCESS.md block-style bullet removed; SKILL.md script table
   notes TS implementation + self-contained bundled dist, "Extracting this
   skill" documents the dev-time build step.

## Review

verdict: approved

- 2026-08-01 The Clanker: independently re-ran all acceptance checks in the
  worktree — npm test 105/105, npm run build OK (yaml bundled, no
  `require('yaml')` in dist), run-tests.sh 49/49 (block-style test asserts
  lint-clean), grep-clean (no fm_field/fm_list/awk/perl in skills/planr/),
  docs edits match acceptance (TICKET-FORMAT/PROCESS/SKILL), and a fresh
  throwaway no-node_modules smoke test (board exit 0, lint exit 1 with the
  same 1 error + 19 warnings as the in-repo run. `_lock.sh` and
  `git-stub.ts` are gone; CLI test paths point at `skills/planr/dist/cli/`.
  Approved.

## Notes

- 2026-08-01 completed. Per leader decision (confirmed with developer):
  `dist/` stays gitignored (build-time artifact, not committed); the build
  outputs are packaged with the skill so the copied folder is self-contained.
- 2026-07-30 created. Depends on [[port-merge-task]] (last script). This is
  the gate to closing [[port-scripts-to-typescript]].
