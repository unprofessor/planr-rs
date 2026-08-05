---
id: cli-scaffolding
aliases: [cli-scaffolding]
kind: story
parent: port-scripts-to-typescript
title: CLI + git wrapper layer and .sh shims
status: done
assignee: null
created: 2026-07-30
updated: 2026-08-01
tags: [cli, git, distribution]
depends_on: [parser-foundation]
---

## Goal

Build the thin CLI + git wrapper layer that the ported scripts sit on, plus the
`.sh` shims that keep invocation identical (`./scripts/board.sh …` unchanged in
docs and muscle memory). Establishes the distribution shape (bundled
`dist/cli.js`, copy-folder portable) before any script logic is ported.

## Context

Pi's node is v25.6.1 (runs `.ts` natively via type-stripping), but other
harnesses/node versions may not, and local-path skills do NOT run `npm install`
(confirmed in pi packages.md). So the shipped entrypoint is compiled `.js`,
bundled by esbuild into one self-contained `dist/cli.js` (no `node_modules`).
`.sh` shims (`exec node "$(dirname "$0")/../dist/cli.js" "$@"`) mean SKILL.md
and PROCESS.md keep showing `./scripts/foo.sh` — zero doc churn during the
migration. Git operations stay as `execFileSync('git', […])` — no simple-git,
exact current behavior, worktree support intact.

## Acceptance

- [ ] `src/git.ts` exports typed wrappers (all via `execFileSync('git', …)`):
  `lsTreeMd(ref, dir) -> string[]` (git ls-tree -r --name-only, filtered to
  `*.md`), `showRef(ref, path) -> string` (git show `ref:path`), `worktreeAdd`,
  `worktreeRemove`, `branchDelete`, `mergeNoFf`, `checkout`, `commit`,
  `diffRefs`, `branchList`, `worktreeList`, `revParseVerify`.
- [ ] `src/cli/` has one thin entry per script (`board.ts`, `claim.ts`,
  `lint.ts`, `new-ticket.ts`, `review.ts`, `merge-task.ts`) that parses argv
  (positional + `PLANR_TRUNK`/`PLANR_DIR` env), calls library functions, prints,
  and sets exit code. Each is <40 lines and does NO parsing logic itself.
- [ ] `scripts/*.sh` are rewritten as shims: `#!/usr/bin/env bash` +
  `exec node "$(dirname "$0")/../dist/cli/<name>.js" "$@"`. The six existing
  script filenames are preserved. `chmod +x` on all.
- [ ] `npm run build` produces `dist/cli/<name>.js` for each, bundled with the
  parser + git wrappers, no `node_modules` at runtime.
- [ ] A shim smoke test in `run-tests.sh` (or a new `tests/shim.test.ts`):
  `./scripts/board.sh` in a repo with an empty `.plan/` exits 0 and prints
  nothing, proving the shim → dist → node chain works end to end.

## Notes

- 2026-07-30 created. Depends on [[parser-foundation]] for the bundled parser.
  The CLI entries in this story are stubs (they can `console.log` argv); real
  logic lands in the per-script tasks.
