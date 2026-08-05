---
id: ts-project-setup
aliases: [ts-project-setup]
kind: task
parent: parser-foundation
title: Set up TS project scaffolding (package.json, tsconfig, esbuild)
status: done
assignee: null
created: 2026-08-01
updated: 2026-08-01
tags: [typescript, scaffolding, build]
depends_on: []
---

## Goal

Create `package.json`, `tsconfig.json`, and esbuild build config. Run
`npm install`. This is the minimal scaffold that [[parse-core]] and
[[git-wrappers]] both depend on — no source code beyond the config files.

## Context

Parent story: [[parser-foundation]]. The research pass chose `eemeli/yaml`
for frontmatter parsing, esbuild for bundling, vitest for testing. Pi's
node is v25.6.1 (supports `erasableSyntaxOnly`), but the shipped artifact
must be compiled JS since local-path skills don't run `npm install`.

## Acceptance

- [x] `package.json` exists at repo root with:
  - `devDependencies`: `typescript`, `esbuild`, `vitest` (latest)
  - `dependencies`: `yaml` (eemeli/yaml)
  - `scripts.test`: `vitest run --passWithNoTests`
  - `scripts.build`: `esbuild src/cli/*.ts --bundle --platform=node --format=cjs --outdir=dist/cli --external:yaml`
- [x] `tsconfig.json` sets `erasableSyntaxOnly: true`, `strict: true`,
  `module: nodenext`, `moduleResolution: nodenext`, `target: esnext`,
  `outDir: dist`, `rootDir: src`
- [x] `npm install` succeeds (no lockfile conflicts)
- [x] `npm test` runs without crashing (no tests yet, exits 0)
- [x] `npm run build` produces at least an empty `dist/` (or succeeds
  with a warning that there's nothing to bundle yet — the real test is
  in later tasks)

## Validation

All checks performed in the worktree at `/home/exfed/projects/wt-ts-project-setup`:

1. **package.json** — verified all fields: `type: module`, devDeps (typescript ^5.9.3,
   esbuild ^0.25.3, vitest ^3.2.7), deps (yaml ^2.7.1 = eemeli/yaml), scripts.test
   includes `--passWithNoTests` so vitest exits 0 with no test files.
2. **tsconfig.json** — verified: `erasableSyntaxOnly: true`, `strict: true`,
   `module: nodenext`, `moduleResolution: nodenext`, `target: esnext`,
   `outDir: dist`, `rootDir: src`, `declaration: true`, `sourceMap: true`,
   `skipLibCheck: true`.
3. **npm install** — 55 packages added, 0 vulnerabilities, no errors.
4. **npm test** — `vitest run --passWithNoTests` exits 0 (no test files).
5. **npm run build** — produces `dist/cli/placeholder.js` (780 bytes bundled CJS).
   Smoke test: `node -e "require('./dist/cli/placeholder.js')"` succeeds.

One deviation from the Notes (not Acceptance): a minimal `src/cli/placeholder.ts`
was created so the esbuild glob `src/cli/*.ts` has something to bundle. Without
it, esbuild exits with an error on an empty glob. This file will be replaced by
real CLI entries in the `cli-shims` task. No other source code was created.

## Notes

- 2026-08-01 created. No source code in this task — just config + install.
  The `--external:yaml` flag on esbuild is intentional: eemeli/yaml is the
  one runtime dep that ships with the skill; everything else is bundled.

## Review

verdict: approved

### Re-verified acceptance criteria

1. **package.json** — all fields confirmed:
   - `devDependencies`: typescript, esbuild, vitest present
   - `dependencies`: yaml present (eemeli/yaml, resolved as yaml@2.9.0)
   - `scripts.test`: `vitest run --passWithNoTests`
   - `scripts.build`: correct esbuild invocation with `--external:yaml`
2. **tsconfig.json** — `erasableSyntaxOnly: true`, `strict: true`,
   `module: nodenext`, `moduleResolution: nodenext`, `target: esnext`,
   `outDir: dist`, `rootDir: src` all present. Extras (`declaration: true`,
   `sourceMap: true`, `skipLibCheck: true`) are reasonable defaults.
3. **npm install** — `npm ls --depth=0` shows all 4 packages resolved
   (esbuild@0.25.12, typescript@5.9.3, vitest@3.2.7, yaml@2.9.0). No errors.
4. **npm test** — vitest exits 0 with `No test files found, exiting with code 0`.
5. **npm run build** — produces `dist/cli/placeholder.js` (780 bytes).
   Smoke test `require()` from `/tmp` succeeds.

### Findings

- **Correct**: All 5 acceptance criteria pass independently. Config is minimal
  and matches the declared intent.
- **Note**: A `src/cli/placeholder.ts` (exporting `{}`) was created so esbuild's
  glob has a file to bundle. This is documented as temporary and will be replaced
  in cli-shims. This is a pragmatic workaround, not a deviation from acceptance.
- **Note**: `package-lock.json` is untracked. Not required by acceptance but
  typical for reproducible installs. Can be committed separately if desired.
- **No blockers**.
