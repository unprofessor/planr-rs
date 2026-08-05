---
id: precommit-hook
aliases: [precommit-hook]
kind: task
parent: utility-scripts
title: Add pre-commit hook template for lint.sh
status: todo
assignee: null
created: 2026-07-30
updated: 2026-07-30
tags: []
depends_on: [port-lint]
---

## Goal

Add a `scripts/install-hook.sh` script that installs a pre-commit git hook to run `lint.sh` on staged `.plan/` file changes, preventing dangling parents, missing depends_on targets, and dependency cycles from reaching trunk.

## Context

Parent story: [[utility-scripts]] under [[supplementary-tooling]]. The [[port-scripts-to-typescript]] epic notes a pre-commit hook as out-of-scope for the port, but it's a valuable guardrail: a leader editing frontmatter on trunk can accidentally introduce a dangling reference or cycle, and only remembers to run lint after committing. A pre-commit hook catches this at commit time.

The `install-hook.sh` script is a thin bash wrapper (it just writes git hooks), but the hook itself invokes the TS `dist/cli.js lint` rather than the old bash lint.sh. Depends on port-lint being done so the TS lint exists.

## Acceptance

- [ ] `scripts/install-hook.sh` creates/overwrites `.git/hooks/pre-commit` with the hook script
- [ ] The hook runs `scripts/lint.sh` on staged `.plan/` files only
- [ ] If no `.plan/` files are staged, the hook silently passes (doesn’t slow down code commits)
- [ ] If `.plan/` files are staged and `lint.sh` reports errors, the commit is aborted with the lint errors printed
- [ ] `lint.sh` warnings do NOT block the commit (exit 0 with warnings is OK)
- [ ] The hook chdirs to the repo root so relative script paths work
- [ ] Documented in SKILL.md (scripts table or a note in the Leader workflow section about running `install-hook.sh`)

## Notes

- 2026-07-30 created
- `depends_on: [port-lint]` — the hook invokes the TS lint, so lint must be ported first
- `install-hook.sh` is a thin bash script (writes git hooks); the hook itself runs `dist/cli.js lint`
- Use `git diff --cached --name-only -- .plan/` to find staged `.plan/` files
- The hook is minimal: check for staged .plan files, run `dist/cli.js lint`, exit with its status
- `install-hook.sh` should be idempotent (safe to run multiple times)
- Consider also adding `scripts/install-hook.sh --uninstall` to remove the hook
