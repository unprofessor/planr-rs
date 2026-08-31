//! Git plumbing: build a tree, make a commit, move one ref.
//!
//! Everything here works WITHOUT a working tree or a checkout. That is what
//! lets a verb build its declaration on a ref another worktree currently has
//! checked out, and it is why the one-branch-one-worktree rule never binds.
//!
//! The shape every verb follows:
//!   read base tree -> apply content steps -> write tree -> commit-tree -> move a ref
//! Everything before the ref move is unreferenced object construction, which
//! git garbage-collects, so a failure at any earlier point leaves nothing
//! behind.

use std::path::{Path, PathBuf};
use std::process::Command;

fn run_env(args: &[&str], index: Option<&Path>) -> Result<String, String> {
    let mut cmd = Command::new("git");
    cmd.args(args);
    if let Some(index) = index {
        cmd.env("GIT_INDEX_FILE", index);
    }
    let out = cmd
        .output()
        .map_err(|e| format!("git command failed: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&out.stderr);
        Err(stderr
            .lines()
            .rfind(|l| !l.trim().is_empty())
            .unwrap_or("git failed")
            .to_string())
    }
}

fn run(args: &[&str]) -> Result<String, String> {
    run_env(args, None)
}

fn run_stdin(args: &[&str], stdin_data: &str) -> Result<String, String> {
    use std::io::Write;
    use std::process::Stdio;

    let mut child = Command::new("git")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("git command failed: {e}"))?;
    child
        .stdin
        .as_mut()
        .ok_or("cannot write to git stdin")?
        .write_all(stdin_data.as_bytes())
        .map_err(|e| format!("cannot write to git stdin: {e}"))?;
    let out = child
        .wait_with_output()
        .map_err(|e| format!("git command failed: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&out.stderr);
        Err(stderr
            .lines()
            .rfind(|l| !l.trim().is_empty())
            .unwrap_or("git failed")
            .to_string())
    }
}

pub fn log_raw(args: &[&str]) -> Result<String, String> {
    let mut full = vec!["log"];
    full.extend_from_slice(args);
    run(&full)
}

pub fn rev_parse(ref_: &str) -> Result<String, String> {
    Ok(run(&["rev-parse", "--verify", "--quiet", ref_])?
        .trim()
        .to_string())
}

pub fn ref_exists(ref_: &str) -> bool {
    rev_parse(ref_).map(|s| !s.is_empty()).unwrap_or(false)
}

/// Read a file's content at a ref. Errors when the path is absent there.
pub fn show(ref_: &str, path: &str) -> Result<String, String> {
    run(&["show", &format!("{ref_}:{path}")])
}

/// How many commits `tip` carries that `base` cannot reach -- i.e. what would
/// be lost if `tip`'s ref were deleted.
pub fn count_unreachable(base: &str, tip: &str) -> Result<usize, String> {
    let out = run(&["rev-list", "--count", &format!("{base}..{tip}")])?;
    out.trim()
        .parse()
        .map_err(|e| format!("cannot count commits on {tip}: {e}"))
}

pub fn is_ancestor(a: &str, b: &str) -> bool {
    Command::new("git")
        .args(["merge-base", "--is-ancestor", a, b])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// A scratch index, so tree building never disturbs the real one.
pub struct ScratchIndex {
    path: PathBuf,
}

impl ScratchIndex {
    /// Populate a scratch index from `base`'s tree.
    pub fn from_ref(base: &str) -> Result<ScratchIndex, String> {
        let unique = format!(
            "planr-next-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        );
        let path = std::env::temp_dir().join(unique);
        let index = ScratchIndex { path };
        run_env(&["read-tree", base], Some(&index.path))?;
        Ok(index)
    }

    pub fn put(&self, path: &str, content: &str) -> Result<(), String> {
        let sha = run_stdin(&["hash-object", "-w", "--stdin"], content)?
            .trim()
            .to_string();
        run_env(
            &[
                "update-index",
                "--add",
                "--cacheinfo",
                &format!("100644,{sha},{path}"),
            ],
            Some(&self.path),
        )?;
        Ok(())
    }

    pub fn remove(&self, path: &str) -> Result<(), String> {
        run_env(&["update-index", "--force-remove", path], Some(&self.path))?;
        Ok(())
    }

    pub fn write_tree(&self) -> Result<String, String> {
        Ok(run_env(&["write-tree"], Some(&self.path))?
            .trim()
            .to_string())
    }
}

impl Drop for ScratchIndex {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

pub fn commit_tree(tree: &str, parents: &[&str], message: &str) -> Result<String, String> {
    let mut args: Vec<String> = vec!["commit-tree".to_string(), tree.to_string()];
    for p in parents {
        args.push("-p".to_string());
        args.push(p.to_string());
    }
    args.push("-m".to_string());
    args.push(message.to_string());
    let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    Ok(run(&refs)?.trim().to_string())
}

/// Create a branch atomically: the empty old-value means "must not exist", so
/// two concurrent claims of one ticket resolve by compare-and-swap and the
/// loser gets a clear failure rather than a silent overwrite.
pub fn create_ref(name: &str, new: &str) -> Result<(), String> {
    run(&["update-ref", &format!("refs/heads/{name}"), new, ""])
        .map_err(|e| format!("cannot create branch '{name}': {e}"))?;
    Ok(())
}

pub fn update_ref(name: &str, new: &str) -> Result<(), String> {
    run(&["update-ref", &format!("refs/heads/{name}"), new])?;
    Ok(())
}

pub fn delete_ref(name: &str) -> Result<(), String> {
    if !ref_exists(name) {
        return Ok(()); // idempotent -- a re-run after partial failure completes
    }
    run(&["update-ref", "-d", &format!("refs/heads/{name}")])?;
    Ok(())
}

/// Integrate `source` into `target`. Fast-forwards when it can; otherwise
/// builds a real merge commit without checking anything out. A conflict is
/// reported rather than resolved -- the target is left untouched.
pub fn merge_into(target: &str, source: &str, message: &str) -> Result<String, String> {
    let target_sha = rev_parse(target)?;
    let source_sha = rev_parse(source)?;

    if is_ancestor(&target_sha, &source_sha) {
        update_ref(target, &source_sha)?;
        return Ok(source_sha);
    }

    let tree = run(&["merge-tree", "--write-tree", &target_sha, &source_sha]).map_err(|e| {
        format!("merge conflict integrating '{source}' into '{target}': {e}\nrebase the branch onto {target} and re-run; {target} is untouched")
    })?;
    let tree = tree
        .lines()
        .next()
        .ok_or("merge-tree produced no tree")?
        .trim()
        .to_string();
    let merge = commit_tree(&tree, &[&target_sha, &source_sha], message)?;
    update_ref(target, &merge)?;
    Ok(merge)
}

/// `git worktree add <path> <branch>` -- checking out an EXISTING branch.
///
/// Deliberately not `crate::git::worktree_add`: that helper omits the branch
/// argument when the branch already exists, which makes git invent a new
/// branch named after the path. 0.3 never hits that path because it creates
/// branch and worktree together; here the ref is created first, by design.
pub fn worktree_add(path: &str, branch: &str) -> Result<(), String> {
    run(&["worktree", "add", path, branch]).map(|_| ())
}

pub fn worktree_remove(path: &str) -> Result<(), String> {
    run(&["worktree", "remove", "--force", path]).map(|_| ())
}
