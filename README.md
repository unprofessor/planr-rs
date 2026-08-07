# planr-cli

Trunk-based, markdown-formatted backlog CLI for solo and concurrent
development. Provides automated backlog management: ticket creation, branch
claiming, structural linting, board summaries, review briefs, and merge gating.

## Installation

### From source

```bash
cargo install --git https://github.com/unprofessor/planr-cli.git
# or
git clone https://github.com/unprofessor/planr-cli.git
cargo install --path planr-cli
# or
git clone https://github.com/unprofessor/planr-cli.git
cd planr-cli
cargo build --release
cp target/release/planr ~/.local/bin/
```

### Prebuilt binaries

Available on the [GitHub Releases page](https://github.com/unprofessor/planr-cli/releases)
&mdash; download the archive for your platform and extract the `planr` binary into
your `$PATH`.

### Dependencies

- **Rust** (1.70+, ideally the latest stable) for source builds.
- **git** (any modern version) &mdash; all planr commands shell out to git.
- **flock** (util-linux) is **not** required &mdash; the Rust binary uses in-process
  `flock` via the `fs2` crate on `<git-common-dir>/planr.lock`. During
  transition, the lock file is shared with the legacy TS/bash planr tooling.

### Binary size

A release build with LTO and symbol stripping is approximately **2 MB**
(stripped). Build with:

```bash
cargo build --release
ls -lh target/release/planr
```

## Usage

```bash
planr new <kind> <slug> <title> [parent]     # Scaffold a ticket file
planr board                                    # Backlog + in-flight board
planr lint [ref]                               # Structural checks (working tree or ref)
planr claim <slug> [worktree]                  # Create worktree, set in_progress
planr review <slug>                            # Brief a reviewer
planr close task <slug>                        # Gate check -> done -> merge
planr close story <slug>                       # Gate children -> done -> commit
planr close epic <slug>                        # Gate stories -> done -> commit
planr --version                                # Print version
planr --help                                   # Full help
```

### Subcommand details

| Subcommand | Description |
|------------|-------------|
| `new` | Create an epic, story, or task file from an embedded template. Exclusive lock on planr.lock for prefix allocation. |
| `board` | Render the full board: epics, stories, tasks, in-flight branches, and a status summary. |
| `lint` | Three-pass structural checker: per-file, cross-ref (parents, deps, wiki-links), cycle detection. Exit 1 on errors. |
| `claim` | Dependency-gate check, `git worktree add`, status flip to `in_progress`. Shared lock. |
| `review` | Print a review brief: acceptance criteria, validation notes, diff, and reviewer guidance. |
| `close task` | Guards (status=review + approved verdict), done flip on branch, `git merge --no-ff`, cleanup. Exclusive lock. |
| `close story` | Child-task gate (all must be done), done flip on trunk, commit. Exclusive lock. |
| `close epic` | Child-story gate (all must be done), done flip on trunk, commit. Exclusive lock. |

## Environment variables

| Variable | Default | Description |
|----------|---------|-------------|
| `PLANR_TRUNK` | `main` | Default trunk branch for claim/close/lint operations |
| `PLANR_DIR` | `.plan` | Directory containing the plan tickets |

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
  close_cmd.rs         # Close workflow (task/story/epic)
templates/             # Embedded ticket templates
tests/                 # Integration tests
  planr-e2e.rs         # End-to-end suite (15 tests)
```

## Development

```bash
cargo test              # Run all tests (unit + e2e)
cargo build             # Debug build
cargo build --release   # Release build with LTO
```

All commands run against a repository with a `.plan/` directory. See the
existing backlog in `.plan/` for examples.
