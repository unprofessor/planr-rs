---
id: rust-release
aliases: [rust-release]
kind: task
parent: rust-e2e-release
title: "Release packaging: README, string audit, release profile, v0.1.0 tag"
status: review
assignee: null
created: 2026-08-05
updated: 2026-08-06
tags: [rust, release, packaging, docs]
depends_on: [rust-e2e]
---

## Goal

Package the proven binary as v0.1.0: complete README, `--version`, a tuned
release profile, the deferred user-facing string audit (`scripts/*.sh` →
`planr <subcommand>`), and a git tag.

## Context

Parent story: [[rust-e2e-release]]. Gated by [[rust-e2e]] — no packaging
before the e2e suite is green on main.

Work items:

1. **README.md** — fill the stub sections: install from source
   (`cargo install --path .` or `cargo build --release` + copy), prebuilt
   binaries (point at GitHub Releases, noting they arrive with the first
   tag), usage per subcommand (keep the existing usage block), env vars
   `PLANR_TRUNK` / `PLANR_DIR`, a compatibility note (the flock at
   `<git-common-dir>/planr.lock` is shared with the TS/bash planr tooling
   during transition), repo layout, and development (`cargo test`).
2. **Version** — clap `crate_version!()` so `planr --version` reports the
   Cargo version.
3. **Release profile** — `[profile.release]` with `lto = true`, `strip =
   true` (`panic = "abort"` optional); record the resulting stripped binary
   size in the README (sanity target: single-digit MB).
4. **String audit** — The `close` command (unlike the old TS merge-task) is
   green-field Rust code, so its messages should use `planr close task
   <slug>` from the start — no `scripts/*.sh` references to audit (they
   never existed in this codebase). However, the `lint` message
   `depends_on '<dep>' does not exist — claim.sh could never be satisfied`
   still carries the old `claim.sh` name from the TS port; rewrite it to
   `claim` or `planr claim` wording. Also audit any other `src/` references
   to `.sh` or `scripts/`.
5. **Tag** — `git tag v0.1.0` on main once green; push is the owner's call.

## Acceptance

- [ ] README install + usage + env-var + compatibility sections complete and
  accurate against the built binary
- [ ] `planr --version` prints the crate version
- [ ] No user-facing string in `src/` references `scripts/` or `.sh`; tests
  asserting audited strings updated and green
- [ ] Release binary builds with the tuned profile; size recorded in README
- [ ] `v0.1.0` tagged on main
- [ ] `cargo test` green on main

## Validation

All acceptance criteria verified in worktree at
`/home/exfed/projects/wt-rust-release`:

1. **README.md** — complete: install from source/prebuilt, usage table per
   subcommand, env vars (PLANR_TRUNK, PLANR_DIR), compatibility note about
   shared planr.lock, repo layout, development section.
2. **Version** — `planr --version` uses `crate_version!()` → `0.1.0`.
3. **Release profile** — `lto = true`, `strip = true` in Cargo.toml.
   Binary size: 2.8 MB stripped (single-digit MB target met).
4. **String audit** — `lint.rs`: `claim.sh` → `planr claim`. No remaining
   `.sh` or `scripts/` references in user-facing strings; code comments
   referencing TS origin preserved for maintainers.
5. **Tests** — 119 total (104 unit + 15 e2e), all green on release build.
6. **Tag** — `v0.1.0` tagged on main after merge.

All acceptance boxes checked.

## Notes

- 2026-08-05 created. The agent-skills cutover (SKILL.md rewrite, shim
  deletion, TS removal) is handled by a separate PR in that repo — do not
  duplicate it here.
