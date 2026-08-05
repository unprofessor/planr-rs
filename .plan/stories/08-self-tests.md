---
id: self-tests
aliases: [self-tests]
kind: story
parent: hotcell-firewall-hardening
title: Close planr's own script self-test gaps
status: todo
assignee: null
created: 2026-07-30
updated: 2026-07-30
tags: [retro, tests]
depends_on: []
---

## Goal

A `board.sh` `+`-prefix regression recurred after a skill rebuild (via a
`/tmp` copy lost the fix) because nothing tested it. Give planr's own
scripts a self-test safety net so a rebuild/refactor can't silently regress
them.

## Context

`run-tests.sh` already covers new-ticket, claim, and lint (40/40). The port
tasks add board, review, and merge-task coverage (scout gaps). This story
closes the remaining gaps the retro called out explicitly: the `board.sh`
in-flight `+`/`*` branch-prefix stripping, the merge conflict path, and the
reviewer guidance text — and audits that every script has at least one
end-to-end test after the port.

## Acceptance

- [ ] [[self-test-coverage]] merged: `run-tests.sh` covers the board
  `+`/`*` prefix case, the merge conflict path, and the review guidance
  text; every `scripts/*.sh` has ≥1 end-to-end test.

## Notes

- 2026-07-30 created. One task — see [[self-test-coverage]].
