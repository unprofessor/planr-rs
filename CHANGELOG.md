# Changelog

## [Unreleased]

### Added

- **`planr board`** now prints a source header before the board, showing
  where the tickets were read from: the working-tree path, the resolved
  commit id, and the ref name (the current branch in parentheses, or the
  commit-ish you passed). In working-tree mode a trailing `dirty` marks
  uncommitted changes; ref mode omits it since it reads committed data.

### Changed

- **`planr board`** now defaults to the current on-disk working tree
  instead of trunk, matching `planr lint`. Pass a commit-ish to read a
  specific ref (e.g. `planr board main`, `planr board HEAD~2`, or a SHA);
  omit it to see the `.plan` files in whatever worktree or branch you have
  checked out.

### Fixed

- **A failed `planr claim` no longer hides a real directory from git.**
  The local ignore rule was written before `git worktree add` ran and
  nothing removed it when the worktree creation failed, so a typo'd
  `--worktree` path left that path permanently excluded -- invisible in
  `git status`, so nothing pointed at the cause. The rule is now written
  only after the worktree exists.

- **`planr close` keeps the ignore rule when it cannot remove the
  worktree.** The removal's error was discarded and the rule dropped
  regardless. `git worktree remove` without `--force` refuses whenever the
  worktree holds untracked or modified files -- a stray build artifact is
  enough -- which left the worktree in place and no longer hidden, exactly
  the gitlink corruption the rule exists to prevent.

- **Local ignore rules are anchored to the main working tree.** The
  pattern was resolved against the *invoking* worktree's root but written
  to the shared `.git/info/exclude`, so a rule added from a linked worktree
  matched a same-named directory in the main tree, and `close` run from a
  secondary worktree silently removed nothing. Paths are also resolved
  through symlinks now: a worktree reaching the repository via a symlinked
  path used to look like it lay outside the repository and got no rule at
  all (every `$TMPDIR` path on macOS takes that route).

- **`planr claim` resumes instead of refusing on a stale worktree
  record.** git keeps listing a worktree whose directory was deleted with
  `rm -rf` until something prunes it, so the refusal named a path that was
  not there and locked the task out permanently. A claim now refuses only
  when the worktree is really present, and prunes the stale record
  otherwise. A live holder is still refused.

- **Resuming a claim no longer rolls the branch back to `in_progress`.**
  Any status differing from the flip was rewritten, so re-claiming a task
  whose branch had reached `review` discarded the finished review and then
  made `close` refuse the task for having the wrong status. A claim now
  flips only a branch that has not started work.

- **`planr board` tells an invalid status apart from a missing ticket.**
  Both took the "no readable task file" warning, which sent the reader
  hunting for a file that was sitting where they left it; a typo'd status
  now says so and points at `planr lint`. The status list is shared with
  `lint` rather than duplicated, so the two cannot drift.

- **`planr board` no longer drops a task whose branch has no readable
  ticket.** The summary skipped a task that had an in-flight branch on the
  assumption the branch would supply its status, but a branch reporting
  `(no task file)` was counted nowhere -- so the ticket disappeared from
  every bucket and from `total`. Such a branch is a legitimate state (a
  renumbered file, an uncommitted ticket, a branch made by hand), so the
  count now falls back to the trunk status, and `planr board` writes a
  warning naming the branch to stderr -- a rename that detaches a branch
  from its ticket is otherwise invisible. Warnings go to stderr so the
  board on stdout stays parseable.

- **`planr board` marks a task whose status comes from an in-flight
  branch.** `claim` flips the status on the worktree branch and leaves
  trunk alone, so the tasks table reported a claimed task as `todo` for the
  whole life of the work while the summary counted it as `in_progress` --
  the two halves of the board disagreed. The STATUS column now shows the
  branch value with a trailing `*`, and a legend under the table says where
  it came from. A branch that has no readable task file keeps its
  placeholder in the in-flight section only, rather than inventing a ticket
  status.

- **`planr board` now shows the in-flight section from trunk.** The
  branch scan parsed the decorated output of `git branch --list`, stripping
  only the `* ` (current branch) and two-space markers. Git marks a branch
  that is checked out in a linked worktree with `+ ` instead, which is what
  every `planr claim` produces, so each name came back as `+ plan/<slug>`,
  the ref lookup failed, and the row was dropped without a word. The leader
  on trunk saw no `## in flight` section and a summary counting every
  claimed task as `todo`; the section appeared only when the board was run
  from inside the worktree itself. The scan now asks git for
  `%(refname:short)` rather than parsing decoration.

- **`planr claim <slug>` without `--worktree` now does the full claim**
  ([#4]). Since 0.3.0 an omitted `--worktree` was treated as
  `--no-worktree`, so a bare `claim` printed `claimed: <slug>` and exited 0
  having created no branch, no worktree, no status flip, and no commit --
  an agent that trusted that output would start editing on trunk, and two
  agents could each "claim" the same task without either showing as
  `in_progress`. Omitting the flag now uses the documented default path
  `<plan-dir>/worktrees/wt-<slug>`, the same as passing `--worktree` with
  no value. `--no-worktree` remains the only way to opt out.
  `claim` also now adds an ignore rule to `.git/info/exclude` for any
  worktree that lands inside the repository -- the default location and an
  explicit `--worktree` path alike. A worktree inside the working tree is
  an embedded repo, so without a rule `git add` staged it as a gitlink (a
  bogus submodule that a fresh clone cannot resolve) and trunk read dirty
  until someone ran `git rm --cached`. The rule is local because a worktree
  is local, so nothing new appears for the leader to commit; a worktree
  outside the repository gets no rule. `planr close` drops the rule again
  when it removes the worktree -- a stale rule would silently hide whatever
  is created at that path later. The shared rule for the default location
  covers a directory planr reuses for every claim, so it survives.

- **`planr claim` on a task that is already claimed** now refuses with
  `refuse claim: task '<slug>' is already claimed; its worktree is at
  <path>` instead of surfacing git's `fatal: '<path>' already exists`,
  which told an agent nothing about what went wrong.

- **`planr claim` can resume a claim whose worktree was removed.**
  `worktree_add` passed the trunk ref as the commit-ish even when the
  branch already existed, so rebuilding the worktree died with
  `fatal: '<trunk>' is already used by worktree at ...`. An existing branch
  is now its own starting point, and the status flip -- a no-op on a branch
  that already reads `in_progress` -- no longer tries to commit nothing.

- **`planr new` quotes the `title:` it writes** ([#1]). A colon in the title
  (`"Sanitary history: boundary rev, rewriter"`) produced frontmatter that
  planr's own YAML reader could not parse -- silently at creation, then as
  lint errors later. Titles are now emitted as YAML scalars, quoted whenever
  a colon, a leading indicator character, or a trailing space would otherwise
  break the parse.
- **`planr lint` reports a frontmatter parse failure as itself** ([#1]).
  A block that fails to parse reads as every-field-missing, so lint used to
  cascade into `missing id`, `kind '<missing>'`, and `must name a parent`
  findings about fields that were present -- and children of the broken
  ticket picked up wrong-parent-kind warnings. Lint now emits one error
  naming the parse failure, and still resolves the ticket's slug and kind
  from its path so its children stay clean.
- **`planr lint` says `stories directory`**, not `storys directory`.

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

[#1]: https://github.com/unprofessor/planr-rs/issues/1
[#4]: https://github.com/unprofessor/planr-rs/issues/4
