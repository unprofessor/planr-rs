//! In-process flock helper -- replaces the TS pattern of spawning
//! `flock -s|-x <lock> node -e <script>` children.
//!
//! The lock is held on `<git-common-dir>/planr.lock`, which is the SAME
//! file path and locking mechanism the TS/bash tooling uses, so Rust and TS
//! scripts serialize against each other during transition.
//!
//! Lock modes:
//!   - `Claim` tasks: **shared** (`lock_shared`) -- multiple claims can
//!     read concurrently.
//!   - `NewTicket`, `MergeTask`: **exclusive** (`lock_exclusive`) --
//!     full serialization of prefix allocation / trunk mutation.

use fs2::FileExt;
use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

use crate::git;

/// An acquired flock on `<git-common-dir>/planr.lock`. Holding this guard
/// means the kernel holds the lock for our process. When the guard is
/// dropped, the wrapped `File` is closed and the lock is released.
pub struct PlanrLock {
    _file: File,
}

impl PlanrLock {
    /// Acquire a **shared** lock on planr.lock for the repo containing `cwd`.
    /// Multiple processes can hold a shared lock simultaneously.
    pub fn shared(cwd: &Path) -> io::Result<Self> {
        let path = lock_path(cwd)?;
        let file = open_lock_file(&path)?;
        file.lock_shared()?;
        Ok(PlanrLock { _file: file })
    }

    /// Acquire an **exclusive** lock on planr.lock for the repo containing
    /// `cwd`. Only one process can hold the exclusive lock at a time.
    pub fn exclusive(cwd: &Path) -> io::Result<Self> {
        let path = lock_path(cwd)?;
        let file = open_lock_file(&path)?;
        file.lock_exclusive()?;
        Ok(PlanrLock { _file: file })
    }
}

/// Resolve the lock file path: `<git-common-dir>/planr.lock`.
fn lock_path(cwd: &Path) -> io::Result<PathBuf> {
    let gd = git::git_common_dir(cwd).map_err(|e| {
        io::Error::new(io::ErrorKind::Other, format!("git-common-dir: {e}"))
    })?;
    Ok(Path::new(&gd).join("planr.lock"))
}

/// Open (creating if absent) the lock file. The file is opened read-write so
/// that the kernel flock is effective on Linux (a write FD is needed for
/// exclusive locks).
fn open_lock_file(path: &Path) -> io::Result<File> {
    // Ensure the parent directory exists (TS `mkdirSync(dirname(lp), { recursive: true })`)
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::{Arc, Barrier};
    use std::thread;
    use tempfile::TempDir;

    /// Create a minimal git repo in a tmpdir and return (tmpdir, repo_path).
    fn init_repo() -> (TempDir, PathBuf) {
        let tmp = TempDir::new().unwrap();
        let repo = tmp.path().join("repo");
        fs::create_dir_all(&repo).unwrap();
        let out = std::process::Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(&repo)
            .output()
            .unwrap();
        assert!(out.status.success());
        // initial empty commit (without which git-rev-parse --git-common-dir
        // still works, but some git operations need it)
        std::process::Command::new("git")
            .args(["config", "user.email", "test@test"])
            .current_dir(&repo)
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(&repo)
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["commit", "--allow-empty", "-m", "init"])
            .current_dir(&repo)
            .output()
            .unwrap();
        (tmp, repo)
    }

    #[test]
    fn test_lock_path_matches_planr_lock() {
        let (_tmp, repo) = init_repo();
        let path = lock_path(&repo).unwrap();
        assert_eq!(path.file_name().unwrap(), "planr.lock");
        assert!(path.to_string_lossy().contains(".git"));
    }

    #[test]
    fn test_shared_lock_does_not_block_shared() {
        let (_tmp, repo) = init_repo();
        // Two shared locks from the same process -- should not conflict
        let lock1 = PlanrLock::shared(&repo).unwrap();
        let lock2 = PlanrLock::shared(&repo).unwrap();
        drop(lock1);
        drop(lock2);
    }

    #[test]
    fn test_exclusive_lock_serializes() {
        let (_tmp, repo) = init_repo();
        // Use two threads with independent lock files to test exclusivity
        let repo1 = repo.clone();
        let repo2 = repo.clone();
        let barrier = Arc::new(Barrier::new(2));

        let b1 = barrier.clone();
        let t1 = thread::spawn(move || {
            let l = PlanrLock::exclusive(&repo1).unwrap();
            b1.wait(); // signal: thread 1 has the lock
            // Hold for a bit
            thread::sleep(std::time::Duration::from_millis(50));
            drop(l);
        });

        let b2 = barrier;
        let t2 = thread::spawn(move || {
            b2.wait(); // wait for thread 1 to grab the lock
            // Now try to acquire exclusive -- this should block until t1
            // releases, proving that exclusive locks serialize.
            let started = std::time::Instant::now();
            let l = PlanrLock::exclusive(&repo2).unwrap();
            let elapsed = started.elapsed();
            assert!(elapsed >= std::time::Duration::from_millis(40),
                "exclusive lock should block: elapsed={elapsed:?}");
            drop(l);
        });

        t1.join().unwrap();
        t2.join().unwrap();
    }
}
