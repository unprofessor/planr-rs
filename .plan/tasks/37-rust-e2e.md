---
id: rust-e2e
aliases: [rust-e2e]
kind: task
parent: rust-e2e-release
title: Port run-tests.sh end-to-end suite + flock interop test
status: in_progress
assignee: null
created: 2026-08-05
updated: 2026-08-06
tags: [rust, e2e, tests, flock]
depends_on: [rust-lint, rust-board, rust-review, rust-new-ticket, rust-claim, rust-close-cmd]
---

## Goal

Replace `tests/run-tests.sh` (252 LOC bash, 40+ checks in a throwaway git
repo) with a Rust end-to-end suite driving the real `planr` binary, plus a
flock-interop test proving Rust and TS tooling serialize on the same
`planr.lock`.

## Context

Parent story: [[rust-e2e-release]]. Harness: `assert_cmd` + `tempfile`;
each scenario builds a fresh repo (`git init -b main`, local user config,
empty initial commit). The suite is the release gate — it proves the
byte-contracts the unit tests assert piecemeal.

Check classes to port from `run-tests.sh` (every one):

- new-ticket guards: dangling parent (`create the parent first`), slug
  `Bad Slug!`, trailing hyphen `foo-`, double hyphen `foo--bar`
- happy path: epic → two stories → two tasks; `aliases: [<slug>]` filled;
  backlog committed
- cross-story `depends_on` gating: claim refused naming `http-proxy(todo)`;
  flip the dep to `done` → claim succeeds and stdout names the worktree
- lint classes: dangling `depends_on`; block-style `depends_on` lints clean;
  cycle (`depends_on cycle`); self-dep reported once and NOT as a cycle;
  dangling parent; wrong-kind parent is a warning (exit 0); duplicate slug;
  unresolved wiki-link warning (exit 0); invalid status
- ref-mode lint clean on the committed backlog
- new-ticket informational lint: pre-existing errors on stderr, stdout
  exactly one line (the created path), exit 0
- parallel `planr new-ticket` × 3: all exit 0, each stdout one line, three
  distinct sequential prefixes, lint clean afterwards
- board: `## summary` present, six status rows, expected `total`/`done`
  counts on the seeded backlog
- close-task end-to-end: claim → validate → review approve →
  `planr close task <slug>` flips done on branch then merges
  (this exercises [[rust-close-cmd]] beyond its unit tests)

Plus the interop test (env-gated, skipped with a note when
`PLANR_TS_DIST` is unset): point at an agent-skills checkout's
`skills/planr/dist/cli`, fire the TS `new-ticket.cjs` and Rust
`planr new-ticket` concurrently at the same repo, and assert distinct
sequential prefixes — proving the shared `<git-common-dir>/planr.lock`
semantics.

## Acceptance

- [ ] `cargo test` runs the whole e2e suite green, self-contained (no
  network; interop test skips gracefully without `PLANR_TS_DIST`)
- [ ] Coverage parity: every `run-tests.sh` check class above has a
  corresponding Rust test (name them so the mapping is greppable)
- [ ] Suite runtime stays under ~60s on this machine
- [ ] `cargo test` green on main

## Notes

- 2026-08-05 created. Keep `run-tests.sh` semantics, not its line count:
  several checks collapse naturally into one scenario.
