//! Derive the git version at build time via `semvertag-shell`.
//!
//! Calls `git describe --tags --long --always --dirty=.dirty` through the
//! `semvertag-shell` adapter, which rewrites the output into a correctly-ordered
//! SemVer string (e.g. `0.2.1-dev.3+g87af40b` past a release, `0.2.1` at a
//! tagged commit, or `0.2.0+dirty` for a dirty worktree at the tag).
//!
//! The manifest version (`CARGO_PKG_VERSION`) is passed as a derivation hint
//! so the declared next release (e.g. `0.3.0` after tag `v0.2.0`) targets that
//! version instead of a blind patch bump.
//!
//! The result is exposed as `PLANR_VERSION` so the CLI can print it via
//! `planr --version`. Falls back to the Cargo.toml `package.version` when git
//! is unreachable (no `.git` directory, shallow clone, missing binary, etc.)
//! so the build never fails.

fn main() {
    // Best-effort rerun triggers so `cargo build` re-runs this script when
    // the git state changes. Misses packed refs (rare in practice).
    for p in [".git/HEAD", ".git/index", ".git/refs/tags"] {
        println!("cargo:rerun-if-changed={p}");
    }

    // Parse the manifest version to use as a derivation hint: when it's a
    // legal successor of the latest git tag (e.g. 0.3.0 after v0.2.0),
    // the derived version targets it instead of a blind patch bump.
    let hint = std::env::var("CARGO_PKG_VERSION")
        .ok()
        .and_then(|v| semvertag_shell::Version::parse(&v).ok());

    let version = semvertag_shell::describe_with_hint(hint.as_ref())
        .map(|v| v.to_string())
        .unwrap_or_else(|_| {
            // Fall back to Cargo.toml's version so the build never breaks.
            hint.map(|v| v.to_string())
                .or_else(|| std::env::var("CARGO_PKG_VERSION").ok())
                .unwrap_or_else(|| "0.0.0-unknown".to_string())
        });

    println!("cargo:rustc-env=PLANR_VERSION={version}");
}
