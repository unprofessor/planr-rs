# Changelog

## [Unreleased]

### Added

- **Published schema for the v2 language**, served at
  `https://schemas.columnzero.com/planr/v1/planr.schema.json` and kept in-tree
  at `schemas/planr/v1/planr.schema.json` so validation never needs the
  network. One JSON Schema 2020-12 document covers all three artifacts: the
  root schema validates `.plan/schema.yml`, the `#ticket` anchor validates
  ticket frontmatter, and the `#commit` anchor validates a commit's `Planr-*`
  trailer block.
- **Schema validation in CI.** `cargo test` now meta-validates the schema
  document against draft 2020-12, checks that its `$id` still agrees with the
  path it is published at, confirms both anchors resolve, and runs a corpus of
  28 accept/reject fixtures under `tests/fixtures/schema/`. No new CI job --
  the existing `test` job covers it.

## [0.3.1]

### Fixed

- **CI release job** fixed two build failures: Windows target now uses
  `shell: bash` so `$TARGET` resolves correctly (was empty under
  PowerShell); aarch64 Linux now sets `CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER`
  so Cargo uses the cross-compiler instead of x86_64 `rust-lld`.
- **`cargo binstall` support**: `pkg-fmt` changed from the invalid value
  `"tar.gz"` to `"tgz"` (the correct format identifier). Archives are
  now published as `.tgz` files to match binstall expectations.

## [0.3.0]

### Changed

- **`planr abandon`** replaces `--reason obe|wont-do` with a free-text
  positional message argument that supports stdin (like `git commit`).
  The message is appended as a `## Reason Abandoned` prose section instead
  of being stored in frontmatter; the `reason:` frontmatter field is removed.

- **`planr claim --worktree`/`--no-worktree`** -- the `--worktree` flag now
  accepts an optional path argument (`--worktree <path>`); passing the flag
  without a value uses `<plan-dir>/worktrees/wt-<slug>` as the default.
  The new `--no-worktree` flag skips worktree creation, returning
  `claimed: <slug>` for agents that manage their own workspace.
  The old positional worktree argument has been removed.

### Docs

- **README** overhauled: `cargo install planr` promoted to primary install
  method; source build demoted to a subsection; prebuilt binaries section
  now honestly states none are published yet; Dependencies and Binary size
  sections removed as superfluous.
- **Usage section** condensed to `planr --help` reference; inline command
  list and Subcommand details table removed in favor of self-documenting
  CLI (no more drift).
- **Abandoning a ticket** narrative moved from README to a new
  [user guide](docs/user-guide.md) with a link from the Usage section.

### Internal

- **CI**: added `release` job that builds binaries for 5 targets
  (x86_64/aarch64 Linux, x86_64/aarch64 macOS, x86_64 Windows) on tag
  push and attaches them to the GitHub release. Archives match the
  `[package.metadata.binstall]` pattern so `cargo binstall planr` works
  out of the box.
- **ASCII-only sweep**: replaced em dashes and arrows with ASCII
  equivalents across CHANGELOG, user guide, and Rust source comments.
  Test harness strings with non-ASCII are preserved (they verify
  user-input passthrough).

## [0.2.0] -- 2026-08-11

### Added

- **`planr abandon` command** -- a separate, explicit workflow for tickets
  that are overtaken by events (OBE) or will not be done, bypassing the
  review gate:
  - Supports all ticket kinds: `planr abandon task|story|epic <slug> --reason obe|wont-do`
  - Records `status: abandoned`, `reason: <obe|wont-do>`, and refreshed `updated` date
    in frontmatter; commits directly on trunk.
  - Refuses to abandon tickets with an active `plan/<slug>` branch or worktree --
    never merges or discards work silently.
  - Rejects re-abandoning an already abandoned ticket.
  - `abandoned` is accepted by lint and counted by the board summary.
  - Abandoned dependencies intentionally remain blocking: only `status: done`
    unblocks a `depends_on` relationship.
  - Full unit and end-to-end test coverage.

- **`close task` now hints** when the parent story can also be closed (all
  sibling tasks done). Similarly, `close story` hints when the parent epic
  can be closed.

### Changed

- **Lint** accepts `abandoned` as a valid status in addition to the existing
  `todo`, `in_progress`, `review`, `done`, and `blocked`.

- **Board summary** renders a separate `abandoned` count row alongside the
  existing status counts.

- **Project layout** updated in README to reflect the new `abandon.rs` module.

### Fixed

- The `close story` and `close epic` gate messages no longer reference
  legacy script paths; they use the `planr close` command form consistently.

### Internal

- Migrated CI to GitHub Actions (fmt, clippy, tests on every push/PR).
- Integrated `semvertag-shell` for build-time git-derived versioning and
  `cargo-semvertag` for version-regression checks in CI.
- Removed all non-ASCII characters and applied `cargo clippy --fix` / `cargo fmt`.
- Licensed project under MIT.
