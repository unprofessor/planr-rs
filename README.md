# planr

Trunk-based, markdown-formatted backlog CLI for solo and concurrent
development. Provides automated backlog management: ticket creation, branch
claiming, structural linting, board summaries, review briefs, and merge gating.

## Installation

The fastest way to install `planr` is via crates.io (Rust package registry):

```bash
cargo install planr
```

### Prebuilt binaries

Static binaries are available on the
[GitHub Releases page](https://github.com/unprofessor/planr-rs/releases)
&mdash; download the archive for your platform and extract the `planr` binary into
your `$PATH`.

### Building from source

```bash
cargo install --git https://github.com/unprofessor/planr-rs.git
# or
git clone https://github.com/unprofessor/planr-rs.git
cargo install --path planr-rs
# or
git clone https://github.com/unprofessor/planr-rs.git
cd planr-rs
cargo build --release
cp target/release/planr ~/.local/bin/
```

## Usage

Run `planr --help` for a full list of subcommands and options.

## Environment variables

| Variable | Default | Description |
|----------|---------|-------------|
| `PLANR_TRUNK` | `main` | Default trunk branch for claim/close/lint operations |
| `PLANR_DIR` | `.plan` | Directory containing the plan tickets |

### Abandoning a ticket

Use the separate `abandon` command when a ticket is overtaken by events (OBE)
or intentionally will not be done. Provide a free-text message explaining why:

```bash
planr abandon task obsolete-task "OBE — requirement dropped"
planr abandon story postponed-story "Won't do: deferred to Q3 planning"
```

If the message is omitted or `-` is passed, the message is read from stdin
(like `git commit`):

```bash
planr abandon task obsolete-task <<EOF
OBE — the feature was replaced by the new search API.
EOF
```

The command writes `status: abandoned`, a refreshed `updated` date into the
frontmatter, and appends a `## Reason Abandoned` section with your message.
It does not require a worker validation or review verdict. An existing
`plan/<slug>` branch is treated as active work: `abandon` refuses and leaves
the branch and worktree untouched, so cleanup is an explicit human decision.

An abandoned ticket does **not** satisfy `depends_on`; only `status: done`
unblocks a dependency. Update the dependency relationship or abandon the
dependent ticket separately.

## Versioning

`planr` embeds its version at build time from `git describe` via the
[`semvertag-shell`](https://crates.io/crates/semvertag-shell) crate. The
version follows SemVer monotonic ordering:

| State | `planr --version` | Notes |
| --- | --- | --- |
| Tagged release at HEAD | `0.2.0` | Exact tag, no suffix |
| 3 commits past `v0.2.0` | `0.2.1-dev.3+g<hash>` | Patch bump, dev prerelease |
| Dirty worktree at tag | `0.2.0+dirty` | Build metadata, not a prerelease |
| No git / shallow clone | `0.2.0` (from Cargo.toml) | Fallback, never breaks the build |

CI runs [`cargo-semvertag check`](https://crates.io/crates/cargo-semvertag) on
every push and PR to validate that the Cargo.toml version is a legal successor
to the latest git tag &mdash; preventing version regressions and missed bumps.

## Compatibility

`planr` uses **in-process flock** via the `fs2` crate, locking the same file
(`<git-common-dir>/planr.lock`) that the legacy TS/bash planr tooling locks
via `flock(1)`. This means Rust and TS planr commands can run concurrently on
the same repository during transition &mdash; they serialize on the same kernel
lock.

All ticket files are standard Markdown with YAML frontmatter. The format is
identical to what the TS tooling produces and consumes.

## Repository layout

```
.plan/                 # Backlog tracked as ticket files
  epics/               # Epic tickets
  stories/             # Story tickets
  tasks/               # Task tickets
src/                   # Rust source
  main.rs              # CLI entry point (clap)
  parse.rs             # Frontmatter parsing
  ticket.rs            # Ticket types
  git.rs               # Git porcelain wrappers
  lock.rs              # In-process flock guard
  lint.rs              # Three-pass lint engine
  board.rs             # Board renderer
  review.rs            # Review brief generator
  new_cmd.rs           # Ticket creation
  claim.rs             # Claim workflow
  abandon.rs           # Abandon workflow (OBE/won't-do)
  close_cmd.rs         # Close workflow (task/story/epic)
templates/             # Embedded ticket templates
tests/                 # Integration tests
  planr-e2e.rs         # End-to-end suite
```

## Development

```bash
cargo test              # Run all tests (unit + e2e)
cargo build             # Debug build
cargo build --release   # Release build with LTO
```

All commands run against a repository with a `.plan/` directory. See the
existing backlog in `.plan/` for examples.
