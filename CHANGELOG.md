# Changelog

## [Unreleased]

### Added

- **`planr next state` is bounded, and says what it cost.** Reading a ticket's
  state walked its whole reachable history and then discarded all but the last
  state-changing event. The backwards scan now stops at that event: an event
  denotes `const s` when its verb declares `to: s` and the identity otherwise,
  so `const s . f = const s` annihilates everything earlier -- the absorption
  lemma of `docs/semantics.md` section 4, not a heuristic. A state read costs
  commits since the ticket last moved rather than commits since it existed, and
  a ticket that has never moved stops at its own `new` commit, which is already
  a record in the same stream. `new` is therefore a reserved verb name. The
  output reports commits scanned alongside the state, because the whole point
  of the change is a number that should be observable rather than asserted: on
  a 2000-commit history whose ticket last moved at the tip, it reads 1 commit
  instead of 2003.

- **`planr next new` refuses a slug that has ever been used**, not merely one
  that exists now. A ticket's file path is its primary key and the slug maps to
  it one-to-one and permanently: archival deletes the file but does not release
  the name. Reusing one made a single identifier name two tickets, so the new
  ticket folded the archived one's events -- `next board` reported a ticket
  created seconds earlier as `abandoned` while `next state` reported it as
  `todo`, and every verb's `from` gate reads the second answer, so a terminal
  ticket could be re-entered. The refusal names the commit that created the
  slug and the verb that last declared anything about it.

  The check asks the question the fold asks -- has any reachable commit written
  `Planr-Verb: new` for this `Planr-Ticket` -- rather than the path-shaped
  question of whether the ticket's file was ever added. Those look identical
  and are not: renaming the plan directory (or exporting `PLANR_DIR`), purging
  a path with `filter-branch` while every commit survives, or creating from a
  shallow clone each moved the path without touching the trailers, and each
  made a used slug look free. Reading commit messages costs about 20ms flat and
  is indifferent to a commit-graph, against 97ms for the path-limited walk on a
  2000-commit history, so the fresh-slug case got faster rather than slower.
  It costs `new`, once per ticket, and never a read. `new` refuses outright in
  a shallow clone rather than issue a reservation it cannot back.

  A slug whose events are reachable while its creation is not -- a `git replace
  --graft` over the creation commit, or a rewrite that drops it -- is refused
  too, with a different message: the fold still answers for those events, so
  reporting the slug free would hand them to the new ticket. That refusal says
  the repository's history is broken rather than that the name is taken.

  One case no check at creation time can cover: two lineages that each create
  the same slug, whose merge is clean because archival deleted the file on one
  side. That needs a check at integration and is not yet implemented.

- **`planr next new` validates the slug**, which it did not do at all. A slug
  is both the ticket's filename and its identity in the event log, and
  `Planr-Ticket` is read back trimmed -- so `new task "foo "` wrote
  `.plan/tickets/foo .md` while declaring `Planr-Ticket: foo`, and two files
  shared one identity. Acting on one then moved the other: `abandon "foo "`
  reported `todo -> todo` because it could not see its own effect, the ticket
  that actually went terminal was the one nobody had touched, and `board`
  printed two rows with the same name. A trailing space from a shell paste is
  enough, so this needed no adversary. Slugs must now match the published
  schema's `^[a-z0-9][a-z0-9_-]*$`, and the refusal names the rule; the
  pattern is pinned against the published document by a test, since it is now
  written down in two places.

- **Two concurrent `planr next new` calls can no longer both succeed.** The
  slug reservation read trunk, then the tip was read again separately, and the
  final compare-and-swap asserted the *second* read -- so a racer whose
  reservation ran before the winner's ref move but whose tip read ran after it
  landed on top and its swap succeeded. Both processes reported success, one
  ticket file survived, and two genesis records existed for one slug. A
  synchronised race never showed it, because four racers starting together all
  read the same tip; staggering them across the window produced duplicates in
  three of 276 pairs. The reservation is now evaluated against the same commit
  the swap asserts, making creation a single atomic step, and the loser is told
  the slug was taken concurrently instead of receiving a raw ref-lock error.

- **A schema may not declare a verb named `new`.** Creation is fixed tooling
  rather than a verb, and it writes `Planr-Verb: new` -- the record a bounded
  state read stops at, and the only floor a ticket that has never transitioned
  has. A verb of that name ended every walk at itself, silently: the runner
  reads a verb's before and after states through that same walk, so even a
  stateless verb reported itself as a transition to the initial state. Rejected
  at load by `planr` and by the published JSON Schema alike.

- **Published schema for the typed-graph model**, identified by its canonical URL
  `https://schemas.columnzero.com/planr/v1/1.0.0/planr.schema.json` and kept
  in-tree at the matching path so validation never needs the network.
  Projects cite the alias `.../planr/v1/planr.schema.json`, which moves
  forward with each compatible release; the canonical URL never moves, which
  is why it is the document's `$id`. One JSON Schema 2020-12 document covers
  all three artifacts: the root schema validates `.plan/schema.yml`, the
  `#ticket` anchor validates ticket frontmatter, and the `#commit` anchor
  validates a commit's `Planr-*` trailer block. The `v1` in the URL is the
  schema language's own version and is independent of planr's release
  number -- planr is at 0.3.x and the language is at its first version.
- **Schema validation in CI.** `cargo test` now meta-validates the schema
  document against draft 2020-12, checks that its `$id` still agrees with the
  path it is published at, confirms both anchors resolve, and runs a corpus of
  29 accept/reject fixtures under `tests/fixtures/schema/`. No new CI job --
  the existing `test` job covers it.

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

- **`PLANR_NEXT_ORACLE` parses its value instead of its presence.** The
  unbounded state read is a diagnostic reached by tests; setting the variable
  to `0`, `false`, or `off` used to turn it *on*, so anyone exporting it to
  disable the oracle enabled it -- and being an environment variable it is
  inherited by every child process and every CI shell, where it would silently
  degrade `planr next state` to a full history walk. An unrecognized value is
  now an error rather than a guess, and enabling it says so on stderr.

- **A panicking `log_streaming` callback no longer leaks a `git` child.**
  `std::process::Child` has no reaping `Drop`, so every early return reaped by
  hand and a panic reaped not at all; forty panicking callbacks left forty
  zombies. The child is now owned by a guard that kills and waits on unwind.
  Harmless in a short-lived CLI, but the primitive's whole justification is
  that the next caller will not check.

- **A verb declaring `worktree: create` alongside `effect: merge` is now
  rejected.** `base: own` proves the ticket's ref exists when the verb is
  checked, but `merge` releases that ref as part of the effect -- so the
  worktree attached to a branch that no longer existed, and the run reported
  success. The rule is now stated over the post-state ("a ref that outlives the
  effect") rather than as a prohibition on `base`, in both `src/next/schema.rs`
  and the published JSON Schema. Found by enumerating the
  `base × effect × worktree` space while writing `docs/semantics.md`, not by
  testing.
- **`planr next do archive` could never run.** The verb is from-less and
  declares `require: { self: { status: terminal } }`, but the absorbing rule
  that stops a from-less verb firing on a finished ticket refused it on
  exactly the tickets it exists for. A verb that states its own status
  precondition now overrides the implicit rule.
- **`planr next state` reads an archived ticket.** Folding needs the ticket's
  kind to pick its sub-machine, and the kind lives in the file `archive`
  deletes -- so enumeration survived archival but interpretation did not.
  State now falls back to the last commit that still carried the file, and
  only when the file is genuinely absent: a ticket that is present but
  invalid still reports its own parse error.

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
