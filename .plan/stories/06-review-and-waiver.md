---
id: review-and-waiver
aliases: [review-and-waiver]
kind: story
parent: hotcell-firewall-hardening
title: Reviewer flakiness guidance + formal done-with-waiver path
status: todo
assignee: null
created: 2026-07-30
updated: 2026-07-30
tags: [retro, review, waiver]
depends_on: []
---

## Goal

Two retro findings that weaken the meaning of `review` and `done`:
a reviewer mischaracterized a deterministic failure as "transient," and a
task merged `done` with an acceptance box honestly unchecked but the waiver
recorded ad hoc in story notes. Fix both so the verdict and `done` stay
trustworthy.

## Context

The `loopback-only-net` reviewer's "transient" was deterministic (5/5 fail
on trunk). The review process needs suspected-flaky/network tests run N
times with the repetition recorded, or the merge gate re-running them;
"transient" must not be an allowed verdict basis without evidence.

The `firewall-tests` task merged with acceptance #2 unchecked (no real API
key). The path was proven (real Google 400 through the firewall + TLS) and
the reviewer agreed, but the waiver was buried in story notes — so the
board's `done` was no longer a strict guarantee and the waiver wasn't
greppable. Formalize a `## Waiver` block (reason, owner, follow-up) so
waivers are first-class and visible, and add a lint check so `done` with an
unchecked acceptance box and no `## Waiver` is flagged.

## Acceptance

- [ ] [[reviewer-flakiness-guidance]] merged: reviewer guidance requires
  running suspected-flaky/network tests N=3 times, recording the repetition
  in `## Review`; "transient" is not an allowed verdict basis without
  evidence.
- [ ] [[done-with-waiver]] merged: `## Waiver` block convention in
  TICKET-FORMAT.md, lint warning on `done` + unchecked acceptance + no
  waiver, board cue for waived tasks.

## Notes

- 2026-07-30 created. Two tasks; [[done-with-waiver]] follows
  [[reviewer-flakiness-guidance]] (both edit PROCESS.md).
