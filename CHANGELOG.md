# Changelog

## [0.3.0] — 2026-08-12

### Changed

- **`planr abandon`** replaces `--reason obe|wont-do` with a free-text
  positional message argument that supports stdin (like `git commit`).
  The message is appended as a `## Reason Abandoned` prose section instead
  of being stored in frontmatter; the `reason:` frontmatter field is removed.

- **`planr claim --worktree`/`--no-worktree`** — the `--worktree` flag now
  accepts an optional path argument (`--worktree <path>`); passing the flag
  without a value uses `<plan-dir>/worktrees/wt-<slug>` as the default.
  The new `--no-worktree` flag skips worktree creation, returning
  `claimed: <slug>` for agents that manage their own workspace.
  The old positional worktree argument has been removed.

## [0.2.0] — 2026-08-11

### Added

- **`planr abandon` command** — a separate, explicit workflow for tickets
  that are overtaken by events (OBE) or will not be done, bypassing the
  review gate:
  - Supports all ticket kinds: `planr abandon task|story|epic <slug> --reason obe|wont-do`
  - Records `status: abandoned`, `reason: <obe|wont-do>`, and refreshed `updated` date
    in frontmatter; commits directly on trunk.
  - Refuses to abandon tickets with an active `plan/<slug>` branch or worktree —
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
