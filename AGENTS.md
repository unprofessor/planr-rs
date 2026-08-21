# AGENTS.md

Agent instructions for the planr-rs repository. CLAUDE.md is a symlink to this file, so any agent (pi, Claude, or otherwise) picks up the same instructions.

## Repository layout

- `src/` -- Rust source. `main.rs` wires up clap; subcommands live in
  `claim.rs`, `abandon.rs`, `close_cmd.rs`, `lint.rs`, `board.rs`, `git.rs`.
- `tests/` -- end-to-end CLI tests using `assert_cmd` + `tempfile` in
  throwaway git repos.
- `docs/user-guide.md` -- narrative walkthroughs (abandoning a ticket).
- `.github/workflows/ci.yml` -- fmt, clippy, tests on every push; publish
  - multi-arch binary release on tag pushes.

## Conventions

- **ASCII-only prose** in docs and source comments: `--` for em dashes,
  `->` for arrows, straight quotes, `...` not the ellipsis character.
  Test string literals are exempt (they verify user-input passthrough).
- **Self-documenting CLI**: the README points at `planr --help` instead of
  duplicating command listings. If a flag or subcommand changes, update
  `src/main.rs` help text, not the README.
- **Changelog**: every user-visible change gets an entry under `## [Unreleased]`
  (or the in-progress version heading) before merge.
- Rust formatting/linting: `cargo fmt --check` and
  `cargo clippy --workspace --all-targets -- -D warnings` must pass.

## Releasing a new version

1. Bump `version` in `Cargo.toml` (keep semver: the crate is pre-1.0, so
   breaking changes bump the minor, additions/fixes bump the patch).
2. Refresh the lockfile: `cargo generate-lockfile` and commit `Cargo.lock`.
   **The CI publish job rejects a dirty working tree** -- a missed
   `Cargo.lock` update aborts `cargo publish`.
3. Add a changelog section for the new version (no date -- git handles
   that). Move any `[Unreleased]` entries into it.
4. Commit. Create an annotated tag with the changelog entry as the message:
   `git tag -a vX.Y.Z -m "planr X.Y.Z ..."`.
5. Push: `git push origin main --tags`.
6. CI (`fmt` -> `clippy` -> `test`) gates everything. On success, two tag-only
   jobs run:
   - `publish` -- OIDC trusted publishing to crates.io. The `publish`
     environment has a 15-minute wait timer rather than a required reviewer:
     it runs unattended once the timer expires. Do not retag to "fix" it
     unless the run actually fails.
   - `release` -- builds 5 targets (x86_64/aarch64 Linux, x86_64/aarch64
     macOS, x86_64 Windows) and uploads `planr-{target}-v{version}.tgz`
     archives to the GitHub release. Cross-compiles use runner-specific
     toolchains already configured in the workflow.
7. After all jobs go green, verify end-to-end:
   `cargo binstall --install-path /tmp -y planr` (expect it to resolve
   `v{version}` and download from GitHub).

### binstall metadata invariants

`[package.metadata.binstall]` in `Cargo.toml` must agree with what CI
uploads:

- `pkg-fmt = "tgz"` -- `"tar.gz"` is **not** a valid value and breaks
  `cargo binstall` with a TOML parse error.
- `pkg-url` must end in `.tgz` (CI names archives `.tgz`, not `.tar.gz`).
- `bin-dir = "{ name }-{ target }-v{ version }/{ bin }{ binary-ext }"` --
  the archive contains a single top-level folder holding the binary.
- If the archive layout or extension changes, update both `Cargo.toml` and
  the `Package archive` step in `.github/workflows/ci.yml` together.

### Tag re-push caveats

- Moving a tag (`git tag -f` + `git push --force`) re-triggers CI. It is
  only safe before the crates.io publish for that version lands; after the
  crate is published the tag must not move (crate metadata is immutable).
- If a tag push failed while `main` is already ahead, retag on the new
  HEAD rather than rebuilding history.
- Releasing to crates.io is one-way: a broken publish cannot be withdrawn,
  so validate `cargo binstall` metadata *before* pushing the tag.
