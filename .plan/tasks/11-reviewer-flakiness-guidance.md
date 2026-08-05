---
id: reviewer-flakiness-guidance
aliases: [reviewer-flakiness-guidance]
kind: task
parent: review-and-waiver
title: "Reviewer guidance: run flaky/network tests N times, no 'transient' without evidence"
status: todo
assignee: null
created: 2026-07-30
updated: 2026-07-30
tags: [retro, review, docs]
depends_on: [cleanup-and-docs]
---

## Goal

Stop a reviewer from hand-waving a failure as "transient." Require
suspected-flaky / network tests to be run N=3 times with the repetition
recorded in `## Review`; "transient" is not an allowed verdict basis
without that evidence.

## Context

Retro: the `loopback-only-net` reviewer's "transient" was deterministic
(5/5 fail on trunk). The review process let a single observed failure be
discounted. The fix is guidance text in `review.sh`'s reviewer brief +
PROCESS.md's review section — no script logic (the merge gate's verify hook
in [[verify-hook]] is the backstop, but the reviewer should not produce a
misleading verdict in the first place).

## Acceptance

- [ ] `review.sh` / `src/cli/review.ts` reviewer-guidance block adds: for
  any test that fails or that you suspect is flaky/network-dependent, run
  it N=3 times; record the repetition count and outcomes in `## Review`
  (e.g. `ran networked_agent_reaches_loopback_proxy x3: 3/3 pass`); a
  failure may NOT be dismissed as "transient" without repetition evidence.
- [ ] PROCESS.md "Review" section mirrors this: N=3 repeats for
  suspected-flaky/network tests; "transient" requires recorded evidence.
- [ ] `run-tests.sh` asserts the review guidance text contains the
  "run it N=3 times" / "not be dismissed as transient" phrasing (grep on
  `./scripts/review.sh <slug>` output).

## Notes

- 2026-07-30 created. Depends on [[cleanup-and-docs]] (review.ts is ported
  by [[port-review]]; PROCESS.md rewritten by the port first).
