---
id: self-test-coverage
aliases: [self-test-coverage]
kind: task
parent: self-tests
title: Audit run-tests.sh and fill script self-test gaps
status: todo
assignee: null
created: 2026-07-30
updated: 2026-07-30
tags: [retro, tests, self-test]
depends_on: [cleanup-and-docs]
---

## Goal

Close the self-test gaps the retro called out: the `board.sh` in-flight
`+`/`*` branch-prefix stripping, the merge conflict path, and the reviewer
guidance text — and audit that every `scripts/*.sh` has at least one
end-to-end test after the port, so a rebuild/refactor can't silently
regress them.

## Context

Retro: a `board.sh` `+`-prefix regression recurred after a skill rebuild
(via a `/tmp` copy lost the fix) because nothing tested it. The port tasks
add board/review/merge-task coverage; this task fills the specific cases
the retro named and audits the whole surface.

`board.sh` strips branch-list prefixes with `sed 's/^[*+ ]*//'` — a `*`
(current branch) or `+` (checked-out-in-worktree) prefix must not survive
into the `plan/<slug>` slug extraction. The merge conflict path in
`merge-task.sh` (abort, list conflicted files, rebase guidance) is
currently untested. The reviewer guidance text is asserted by
[[reviewer-flakiness-guidance]]; this task confirms the baseline guidance
text is covered too.

## Acceptance

- [ ] `run-tests.sh` has a board in-flight test that creates a worktree
  branch (so `git branch --list 'plan/*'` shows it with a `+` prefix from
  the worktree), runs `./scripts/board.sh`, and asserts the branch's row
  appears with the correct slug + status (proving `+`/`*` prefix stripping
  works).
- [ ] `run-tests.sh` has a merge-conflict test: two task branches editing
  the same code line; merge the first; the second's `merge-task.sh` aborts,
  lists the conflicted file, prints the rebase guidance, exits 1, and
  leaves the worktree + branch intact.
- [ ] `run-tests.sh` asserts the `review.sh` guidance block contains the
  core instructions ("Run the acceptance checks yourself", "Edit ONLY the
  task file") — baseline coverage complementing
  [[reviewer-flakiness-guidance]].
- [ ] Audit note in `## Notes`: a table of every `scripts/*.sh` (board,
  claim, lint, new-ticket, review, merge-task, resume, new-retro) → the
  test(s) covering it. No script is uncovered.

## Notes

- 2026-07-30 created. Depends on [[cleanup-and-docs]] (the port's
  run-tests.sh additions land first; this fills the remaining retro-named
  gaps on the final test surface). Coordinate with [[resume-script]] and
  [[retro-template-and-script]] which add their own tests — this task's
  audit confirms they're present.
