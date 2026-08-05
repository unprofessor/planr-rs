---
id: git-wrappers
aliases: [git-wrappers]
kind: task
parent: cli-scaffolding
title: Implement typed git wrappers (src/git.ts)
status: done
assignee: null
created: 2026-08-01
updated: 2026-08-01
tags: [git, wrappers, typescript]
depends_on: [ts-project-setup]
---

## Goal

Implement `src/git.ts` — typed wrapper functions around `execFileSync('git',
…)`. No simple-git, exact current behavior, worktree support intact.

## Context

Parent story: [[cli-scaffolding]]. Every ported script that touches git
(board, claim, merge-task, review) calls these instead of raw
`execFileSync`. The wrappers add types, error handling, and a single place
to change git invocation behaviour later.

## Acceptance

- [x] `src/git.ts` exports these typed functions, all via
  `execFileSync('git', …)`:
  - `lsTreeMd(ref: string, dir: string): string[]` — `git ls-tree -r
    --name-only <ref> -- <dir>` filtered to `*.md`
  - `showRef(ref: string, path: string): string` — `git show
    <ref>:<path>`
  - `worktreeAdd(path: string, branch: string, ref?: string): void`
  - `worktreeRemove(path: string, force?: boolean): void`
  - `branchDelete(branch: string, force?: boolean): void`
  - `mergeNoFf(branch: string): void` — `git merge --no-ff <branch>`
  - `checkout(branch: string): void`
  - `commit(message: string, files?: string[]): void`
  - `diffRefs(ref1: string, ref2: string): string` — `git diff
    <ref1>..<ref2>`
  - `branchList(pattern?: string): string[]` — `git branch --list`
  - `worktreeList(): string[]` — `git worktree list --porcelain`
  - `revParseVerify(ref: string): string` — `git rev-parse --verify
    <ref>`
- [x] All functions throw on non-zero exit (default `execFileSync`
  behavior), with the git stderr message preserved
- [x] `src/git.ts` imports nothing except `execFileSync` from
  `node:child_process`
- [x] `npm run build` succeeds with `src/git.ts` included (the esbuild
  config may need a minor update — see [[cli-shims]])

## Validation

All checks performed in worktree at `/home/exfed/projects/wt-git-wrappers`:

1. **src/git.ts** — 12 functions exported: `lsTreeMd`, `showRef`,
   `worktreeAdd`, `worktreeRemove`, `branchDelete`, `mergeNoFf`,
   `checkout`, `commit`, `diffRefs`, `branchList`, `worktreeList`,
   `revParseVerify`. All via `execFileSync` from `node:child_process`.
   Single import only.
2. **Native TS smoke** — `node --experimental-strip-types` imports all
   functions; each exercised against this repo:
   - `lsTreeMd('HEAD', '.plan')` → 41 files
   - `showRef('HEAD', '.plan/tasks/25-git-wrappers.md')` → frontmatter
   - `revParseVerify('HEAD')` → SHA prefix `c8685fda`
   - `branchList('plan/*')` → 2 branches
   - `worktreeList()` → 9 lines
   - `diffRefs('HEAD~1', 'HEAD')` → 401 chars
3. **Bundled CJS smoke** — `require('./dist/cli/git-stub.cjs')` exports
   all 12 functions; `lsTreeMd` and `revParseVerify` work correctly.
4. **npm test** — vitest exits 0 (no test files, `--passWithNoTests`).
5. **npm run build** — produces `dist/cli/git-stub.cjs` (3.7 KB) and
   `dist/cli/placeholder.cjs` (780 bytes).

### Deviations

- **`--out-extension:.js=.cjs`** added to the build script and
  `package.json` updated. The package has `"type": "module"`, so `.js`
  output is treated as ESM by Node and `module.exports` fails with
  `ReferenceError`. The `.cjs` extension forces CommonJS regardless of
  package type. The [[cli-shims]] task must point shims at `.cjs`.
- **`src/cli/git-stub.ts`** created as a named re-export barrel so
  esbuild's `src/cli/*.ts` glob picks up `src/git.ts`. This stub will
  be replaced by the real `src/cli/merge-task.ts` (etc.) in later tasks.

## Notes

- 2026-08-01 created. Depends on [[ts-project-setup]] for the build
  infrastructure and `src/` directory structure. May be developed in
  parallel with [[parse-core]] since they touch different files.
- `--out-extension:.js=.cjs` is now part of the build; the cli-shims
  task must point `.sh` shims at `.cjs` instead of `.js`.

## Review

- **Criterion 1 — 12 exports**: All 12 functions exported from
  `src/git.ts` (lines 24–126): `lsTreeMd`, `showRef`, `worktreeAdd`,
  `worktreeRemove`, `branchDelete`, `mergeNoFf`, `checkout`, `commit`,
  `diffRefs`, `branchList`, `worktreeList`, `revParseVerify`. Re-exported
  via `src/cli/git-stub.ts` (lines 3–14). Present in bundled CJS output
  `dist/cli/git-stub.cjs` (line 23: `0 && (module.exports = { … })` lists
  all 12 names).
- **Criterion 2 — throw on non-zero exit**: All functions delegate to the
  private `git()` helper (`src/git.ts:5–10`) which calls `execFileSync`
  with no `stdio` override. `execFileSync` throws by default on non-zero
  exit and attaches `.stderr` to the thrown `Error`. No try/catch
  suppression anywhere in the helper. The only caught error is in the
  internal `branchExists()` (`src/git.ts:129–136`), which uses the
  exception for a boolean check — correct and not exported.
- **Criterion 3 — single import**: `src/git.ts:1` imports **only**
  `{ execFileSync }` from `"node:child_process"`. No other imports
  anywhere in the file.
- **Criterion 4 — npm run build**: `esbuild src/cli/*.ts --bundle
  --platform=node --format=cjs --outdir=dist/cli
  --out-extension:.js=.cjs --external:yaml` produces `dist/cli/git-stub.cjs`
  (3.7 KB) and `dist/cli/placeholder.cjs` (780 B) with exit code 0.
- **Deviations verified acceptable**:
  - `--out-extension:.js=.cjs` in `package.json` scripts: necessary
    because `"type": "module"` would interpret `.js` as ESM, breaking
    `module.exports`. Correctly documented.
  - `src/cli/git-stub.ts`: barrel re-export needed so esbuild glob
    `src/cli/*.ts` picks up `src/git.ts`. Will be replaced by real CLI
    entries per [[cli-shims]]. Non-breaking placeholder.
- **No blockers.**

verdict: approved
