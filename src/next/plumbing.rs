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

/// Like [`run_stdin`] but keeps the output as bytes, because `cat-file
/// --batch` frames its records by byte length and a lossy UTF-8 conversion
/// would move the frame boundaries.
fn run_stdin_bytes(args: &[&str], stdin_data: &str) -> Result<Vec<u8>, String> {
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
        Ok(out.stdout)
    } else {
        let stderr = String::from_utf8_lossy(&out.stderr);
        Err(stderr
            .lines()
            .rfind(|l| !l.trim().is_empty())
            .unwrap_or("git failed")
            .to_string())
    }
}

/// Read many blobs in ONE `git cat-file --batch` process.
///
/// Reading them one at a time costs a subprocess each, which is what dominates
/// a board once the per-ticket history walk is gone: the walk is O(commits)
/// but the spawns are O(tickets), and a spawn is far more expensive than the
/// read it performs.
///
/// Each spec is an object name such as `main:.plan/tickets/foo.md`. The result
/// is positional, with `None` where git reported the object missing, so a
/// caller can pair results back to specs without a second lookup.
pub fn cat_file_batch(specs: &[String]) -> Result<Vec<Option<String>>, String> {
    if specs.is_empty() {
        return Ok(Vec::new());
    }
    let input = format!("{}\n", specs.join("\n"));
    let out = run_stdin_bytes(&["cat-file", "--batch"], &input)?;

    // Records are `<oid> SP <type> SP <size> LF <contents> LF`, or
    // `<name> SP missing LF`. Framing is by the declared byte length, never by
    // scanning for a delimiter -- ticket bodies contain newlines.
    let mut results = Vec::with_capacity(specs.len());
    let mut pos = 0usize;
    while pos < out.len() && results.len() < specs.len() {
        let Some(nl) = out[pos..].iter().position(|b| *b == b'\n') else {
            break;
        };
        let header = String::from_utf8_lossy(&out[pos..pos + nl]).to_string();
        pos += nl + 1;

        if header.ends_with(" missing") {
            results.push(None);
            continue;
        }
        let Some(size) = header
            .rsplit(' ')
            .next()
            .and_then(|s| s.parse::<usize>().ok())
        else {
            return Err(format!("cannot parse cat-file header: {header}"));
        };
        if pos + size > out.len() {
            return Err("cat-file output ended mid-record".to_string());
        }
        results.push(Some(
            String::from_utf8_lossy(&out[pos..pos + size]).to_string(),
        ));
        pos += size + 1; // skip the trailing LF git appends
    }

    while results.len() < specs.len() {
        results.push(None);
    }
    Ok(results)
}

/// Ref names under a prefix, in plumbing form -- no decoration, no current
/// branch marker, no "checked out elsewhere" marker.
pub fn for_each_ref(prefix: &str) -> Result<Vec<String>, String> {
    let out = run(&["for-each-ref", "--format=%(refname:short)", prefix])?;
    Ok(out
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect())
}

/// Bring the invoking worktree in line for the one path a verb authored.
///
/// Verbs build commits with plumbing and move a ref, which never touches a
/// working tree. That is what lets them commit to a branch another worktree
/// holds -- but when the verb writes to the ref THIS worktree has checked out,
/// the result is a tree that disagrees with its own HEAD: `git status` shows
/// the new ticket as staged for deletion.
///
/// Only the authored path is touched, so a user's unrelated edits are safe.
pub fn sync_path(ref_: &str, commit: &str, path: &str) -> Result<(), String> {
    let head = run(&["symbolic-ref", "--quiet", "--short", "HEAD"]).unwrap_or_default();
    if head.trim() != ref_ {
        return Ok(()); // this worktree is elsewhere; nothing to reconcile
    }
    if show(commit, path).is_ok() {
        run(&[
            "restore",
            &format!("--source={commit}"),
            "--staged",
            "--worktree",
            "--",
            path,
        ])?;
    } else {
        // The verb removed it -- archival. Drop it from index and disk alike.
        let _ = run(&["rm", "-q", "--cached", "--ignore-unmatch", "--", path]);
        let _ = std::fs::remove_file(path);
    }
    Ok(())
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

/// Move a ref, asserting what it currently points at.
///
/// `git update-ref` takes an expected-old-value for ANY ref move, not just
/// creation, so every effect can be a compare-and-swap for the price of one
/// argument. Without it `claim` was atomic while `submit` and `approve` were
/// not, which made the safety look like a property of claiming rather than of
/// moving a ref.
pub fn update_ref(name: &str, new: &str, old: &str) -> Result<(), String> {
    run(&["update-ref", &format!("refs/heads/{name}"), new, old]).map_err(|e| {
        format!("cannot move '{name}': {e}\nit moved since this verb read it -- re-run to work from the new tip")
    })?;
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
        update_ref(target, &source_sha, &target_sha)?;
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
    update_ref(target, &merge, &target_sha)?;
    Ok(merge)
}

/// Record `source` as merged into `target` WITHOUT applying its changes,
/// using a tree supplied by the caller. This is `merge -s ours` with one path
/// taken from the other side: the source's commits become reachable from the
/// target, so nothing is garbage-collected, while the target's tree is
/// unchanged apart from that path.
///
/// Never fast-forwards -- a fast-forward would apply the work, which is the
/// one thing this exists to avoid.
pub fn merge_ticket_only(
    target: &str,
    source: &str,
    tree: &str,
    message: &str,
) -> Result<String, String> {
    let target_sha = rev_parse(target)?;
    let source_sha = rev_parse(source)?;
    let commit = commit_tree(tree, &[&target_sha, &source_sha], message)?;
    update_ref(target, &commit, &target_sha)?;
    Ok(commit)
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
