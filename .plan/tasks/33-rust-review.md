---
id: rust-review
aliases: [rust-review]
kind: task
parent: rust-read-commands
title: Port review brief + CLI
status: review
assignee: null
created: 2026-08-05
updated: 2026-08-05
tags: [rust, review]
depends_on: [rust-parse-core, rust-git-lock]
---

## Goal

Port `src/review.ts` + `src/cli/review.ts` (~140 LOC TS): the reviewer brief
for a task on its `plan/<slug>` branch — branch, worktree, acceptance
criteria, worker validation, diff vs trunk, and the static reviewer guidance.

## Context

Parent story: [[rust-read-commands]]. TS counterpart: [[port-review]]. No
lock in TS.

Behavior to port exactly:

- Branch must exist: `rev_parse_verify("plan/<slug>")` else error
  `no such branch: plan/<slug>`.
- Task file on the branch: `ls_tree_md(branch, "<planDir>/tasks")` matched
  against `/[0-9]+-<regex-escaped-slug>\.md$`; else error `no task file for
  '<slug>' on plan/<slug>`.
- Worktree discovery from `worktree_list()` porcelain: track the current
  `worktree ` line, select it when `branch refs/heads/plan/<slug>` appears;
  fallback display `(none — checkout plan/<slug> to review)`.
- Body extraction from the branch blob: `extract_section(.., "Acceptance")`
  verbatim; `extract_section(.., "Validation")` with blank lines removed
  (bash `review.sh` parity).
- Diff: `diff_refs(trunk, branch)`.
- The static `--- reviewer guidance ---` text is byte-identical (it already
  contains no `scripts/` references).
- Output layout byte-identical, including the aligned header fields
  `branch:    `, `task:      `, `worktree:  ` and the `--- acceptance ---` /
  `--- validation (worker self-check) ---` / `--- diff vs <trunk> ---`
  separators.

## Acceptance

- [ ] Ported `review.test.ts` cases green (branch missing, task file
  missing, worktree found/none, validation blank-strip, full brief shape)
- [ ] Errors print to stderr, exit 1; brief prints to stdout, exit 0
- [ ] Byte-identical brief against the TS review on the same fixture repo
- [ ] `cargo test` green

## Validation

All acceptance criteria verified in worktree at
`/home/exfed/projects/wt-rust-review`:

1. **review.rs** — `generate_review_brief` implemented:
   - Branch existence check (`rev_parse_verify`)
   - Task file location via NN-regex match on `ls_tree_md`
   - Worktree discovery from `worktree list --porcelain`
   - Acceptance section extraction, Validation with blank lines removed
   - Diff vs trunk via `diff_refs`
   - Static reviewer guidance text byte-identical
   - Output format with aligned headers
2. **CLI** — `planr review <slug>` prints brief on stdout, errors on stderr
3. **Smoke test** — error message matches expected format
4. **Tests** — 55 total, all green
5. **cargo build** — clean

All acceptance boxes checked.

## Notes

- 2026-08-05 created
