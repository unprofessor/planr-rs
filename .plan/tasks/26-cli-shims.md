---
id: cli-shims
aliases: [cli-shims]
kind: task
parent: cli-scaffolding
title: CLI entry stubs, .sh shims, build config, and smoke test
status: done
assignee: null
created: 2026-08-01
updated: 2026-08-01T07:30:00Z
tags: [cli, shims, build, distribution]
depends_on: [parse-core, git-wrappers]
---

## Goal

Wire up the distribution shape: 6 thin CLI entry stubs (`src/cli/*.ts`),
rewrite the 6 `scripts/*.sh` as node shims, update esbuild config to
produce `dist/cli/<name>.js`, and prove the shim → dist → node chain with
a smoke test.

## Context

Parent story: [[cli-scaffolding]]. Pi's node v25.6.1 runs `.ts` natively,
but other harnesses don't, and local-path skills don't run `npm install`.
So the shipped entrypoint is compiled `.js`, bundled by esbuild into
self-contained `dist/cli/<name>.js` (no `node_modules`). `.sh` shims keep
`./scripts/foo.sh` invocation unchanged — zero doc churn during the
migration.

## Acceptance

- [x] `src/cli/` has one thin entry per script:
  - `board.ts`, `claim.ts`, `lint.ts`, `new-ticket.ts`, `review.ts`,
    `merge-task.ts`
  - Each parses argv (positional + `PLANR_TRUNK`/`PLANR_DIR` env), calls
    library functions, prints, sets exit code
  - Each is <40 lines and does NO parsing logic itself (real logic
    lands in the per-script tasks under [[port-scripts]])
  - Stubs can `console.log` argv — they exist to prove the build +
    shim chain
- [x] `scripts/*.sh` are rewritten as shims (six files):

  ```bash
  #!/usr/bin/env bash
  exec node "$(dirname "$0")/../dist/cli/<name>.cjs" "$@"
  ```

  - The six existing script filenames are preserved
  - `chmod +x` on all six
  - Uses `.cjs` extension (not `.js`) because `package.json` has
    `"type": "module"` and the build targets CommonJS
- [x] `npm run build` (esbuild) produces `dist/cli/<name>.cjs` for each
  of the six entries, each bundled with the parser + git wrappers,
  `yaml` external, no `node_modules` required at runtime
- [x] Smoke test: all six shims run and print expected stubs. Bundled
  `.cjs` runs from `/tmp` (no `node_modules` nearby) proving the
  distribution model works.
- [x] The original bash script logic is NOT deleted — this task only
  adds the shim files, overwriting the existing `scripts/*.sh` (they
  are tracked in the skill source, not the project). Original bash
  lives at `~/.agents/skills/planr/scripts/`.

## Validation

All checks performed in worktree at `/home/exfed/projects/wt-cli-shims`:

1. **CLI stubs** — 6 files created in `src/cli/`:
   `board.ts` (14 lines), `claim.ts` (15 lines), `lint.ts` (13 lines),
   `new-ticket.ts` (13 lines), `review.ts` (13 lines), `merge-task.ts` (15 lines).
   All <40 lines. Each parses `process.argv` (positional) and
   `PLANR_TRUNK`/`PLANR_DIR` env vars. Stubs import from `../ticket.js` or
   `../git.js` and `console.log` their invocation. `placeholder.ts` removed.
2. **Shim scripts** — 6 files in `scripts/`: `board.sh`, `claim.sh`,
   `lint.sh`, `new-ticket.sh`, `review.sh`, `merge-task.sh`. All have
   `#!/usr/bin/env bash` + `exec node ...dist/cli/<name>.cjs "$@"`.
   All `chmod +x`. Original symlinks replaced with real files.
3. **npm run build** — esbuild produces `dist/cli/<name>.cjs` for all 6
   stubs (plus kept `git-stub.cjs`, 3.7 KB). stubs are 296–308 bytes each.
   Build exit code 0, 7ms.
4. **Smoke test** — All 6 shims run end-to-end:
   - `./scripts/board.sh arg1 arg2` → `[board] trunk=main planDir=.plan args=[arg1, arg2]`
   - `./scripts/claim.sh foo bar` → `[claim] trunk=main planDir=.plan args=[foo, bar]`
   - `./scripts/lint.sh` → `[lint] trunk=main planDir=.plan args=[]`
   - `./scripts/new-ticket.sh epic test "Test title"` → `[new-ticket] ...`
   - `./scripts/review.sh mytask` → `[review] ...`
   - `./scripts/merge-task.sh mytask` → `[merge-task] ...`
   All exit 0.
5. **Distribution independence** — `node -e "require('<repo>/dist/cli/board.cjs')"`
   runs successfully from `/tmp` (no `node_modules` nearby), proving the
   bundled CJS requires no runtime deps beyond `yaml` (external).
6. **npm test** — 22/22 parse tests still pass (no regression).

Deviations from acceptance as written:

- Shims reference `.cjs` not `.js` — required by `"type": "module"` in
  `package.json` + `--format=cjs` in esbuild. This was settled in the
  [[git-wrappers]] task review. The acceptance block above has been
  updated to reflect `.cjs`.
- `git-stub.ts` is kept as a barrel re-export (not referenced by any shim).
  Harmless; will be replaced when real CLI entries land in [[port-scripts]].

## Review

- Correct: All 6 CLI stubs (`src/cli/board.ts` 16L, `claim.ts` 15L,
  `lint.ts` 15L, `new-ticket.ts` 15L, `review.ts` 15L, `merge-task.ts`
  15L) are <40 lines, parse argv + PLANR_TRUNK/PLANR_DIR env vars,
  import from `../ticket.js` or `../git.js`, and have `#!/usr/bin/env
  node` shebangs for native TS runtimes.
- Correct: All 6 shim scripts (`scripts/*.sh`) are real files (not
  symlinks — `readlink -f` all resolve to the project directory, not
  `~/.agents/skills/planr/scripts/`). All `chmod +x`, all use
  `#!/usr/bin/env bash` + `exec node ...dist/cli/<name>.cjs "$@"`.
- Correct: `npm run build` exits 0 in ~6ms, produces 6 bundled
  `dist/cli/<name>.cjs` (304–316 bytes each) plus `git-stub.cjs` (3.7
  KB). `yaml` is external. No `node_modules` needed at runtime —
  verified by `require()` from `/tmp`.
- Correct: All 6 shims smoke-test cleanly with argv forwarding and env
  var resolution (PLANR_TRUNK=develop works). All exit 0.
- Correct: `~/.agents/skills/planr/scripts/` untouched — original bash
  scripts (2–7 KB each, Jul 30 timestamps) preserved.
- Correct: `npm test` — 22/22 parse tests still pass, no regression.
- Note: esbuild tree-shakes unused imports in stubs (e.g. `board.cjs`
  doesn't bundle `parseTicket` because the stub never calls it). This is
  harmless — real usage will come in [[port-scripts]].
- Note: `git-stub.ts` and `git-stub.cjs` are barrel re-exports not
  wired to any shim. Harmless; will be replaced or removed later.

verdict: approved

## Notes

- 2026-08-01 created. Depends on [[parse-core]] and [[git-wrappers]]
  (needs both as importable modules for the build). The CLI stubs are
  intentionally trivial — real logic lands in the [[port-scripts]] tasks.
  The shim overwrite is the first mutation of `scripts/`; the original
  bash is preserved in the planr skill source at
  `~/.agents/skills/planr/scripts/`.
