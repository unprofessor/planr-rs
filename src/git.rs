//! Git porcelain wrappers -- every function shells out to `git`.
//!
//! Port of `skills/planr/src/git.ts`. All functions discover the repo root
//! from the OS-level current working directory -- same as the TS, where git
//! inherits the process cwd and finds the repo automatically.
//!
//! Error convention: on a non-zero git exit, the captured stderr (trimmed,
//! last non-empty line) is returned as the error string. Callers should
//! surface git's last-line message to the user.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Run a git command from the OS cwd.
fn git(args: &[&str]) -> Result<String, String> {
    run_git(None, args)
}

/// Run a git command from a specific directory.
pub(crate) fn git_in(cwd: &Path, args: &[&str]) -> Result<String, String> {
    run_git(Some(cwd), args)
}

fn run_git(cwd: Option<&Path>, args: &[&str]) -> Result<String, String> {
    let mut cmd = Command::new("git");
    cmd.args(args);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    let out = cmd
        .output()
        .map_err(|e| format!("git command failed: {e}"))?;

    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let last_line = stderr
            .lines()
            .rfind(|l| !l.trim().is_empty())
            .unwrap_or("git failed")
            .to_string();
        Err(last_line)
    }
}

/// List all `.md` files under `dir` at `ref` (e.g. `HEAD:.plan`).
pub fn ls_tree_md(ref_: &str, dir: &str) -> Result<Vec<String>, String> {
    let out = git(&["ls-tree", "-r", "--name-only", ref_, "--", dir])?;
    Ok(out
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| l.ends_with(".md") && !l.is_empty())
        .collect())
}

/// Show a single blob at `ref:path`.
pub fn show_ref(ref_: &str, path: &str) -> Result<String, String> {
    git(&["show", &format!("{ref_}:{path}")])
}

/// `git worktree add <path> [-b] <branch> <commit-ish>`.
///
/// `ref_` is the commit-ish to branch *from*, and applies only when the
/// branch is being created. Once the branch exists it is its own starting
/// point: passing `ref_` there would ask git to check out trunk in the new
/// worktree, which fails with "'<trunk>' is already used by worktree at
/// ..." because trunk is checked out already. Naming the branch explicitly
/// also stops git from inferring one from the path basename.
/// Whether `refs/heads/<branch>` already exists.
pub fn branch_exists(branch: &str) -> bool {
    git(&["rev-parse", "--verify", &format!("refs/heads/{branch}")]).is_ok()
}

pub fn worktree_add(path: &Path, branch: &str, ref_: Option<&str>) -> Result<(), String> {
    let branch_exists = branch_exists(branch);
    let mut args: Vec<&str> = vec!["worktree", "add"];
    if !branch_exists {
        args.push("-b");
        args.push(branch);
    }
    args.push(path.to_str().unwrap_or_default());
    if branch_exists {
        args.push(branch);
    } else if let Some(r) = ref_ {
        args.push(r);
    }
    git(&args).map(|_| ())
}

// ---------------------------------------------------------------------------
// Local ignore rules (.git/info/exclude)
// ---------------------------------------------------------------------------

/// Strip `.` and resolve `..` lexically. The target may not exist yet, so
/// `canonicalize` is not available to do it.
fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Canonicalize as much of `p` as already exists, keeping the rest verbatim.
///
/// A worktree path does not exist when its rule is written, so plain
/// `canonicalize` fails on it. Resolving only lexically is not enough either:
/// a path that reaches the repository through a symlink would not share a
/// prefix with the canonical root and would look like it lay outside the
/// repository. That is not exotic -- on macOS `/tmp` and `$TMPDIR` are
/// symlinks into `/private`, so every tempdir hits it.
fn canonicalize_existing(p: &Path) -> PathBuf {
    let mut tail: Vec<std::ffi::OsString> = Vec::new();
    let mut cur = p.to_path_buf();
    loop {
        if let Ok(mut resolved) = cur.canonicalize() {
            for part in tail.iter().rev() {
                resolved.push(part);
            }
            return resolved;
        }
        let Some(name) = cur.file_name().map(|n| n.to_os_string()) else {
            return p.to_path_buf();
        };
        tail.push(name);
        if !cur.pop() {
            return p.to_path_buf();
        }
    }
}

/// The root of the working tree that contains `target`.
///
/// `.git/info/exclude` is shared by every worktree of the clone, but git
/// anchors a leading-slash pattern to *whichever working tree it is currently
/// evaluating* -- one shared `/target/` rule hides `target` at the top of the
/// main tree and at the top of every linked worktree alike. So the anchor that
/// makes a rule fire is the tree the directory actually sits in, found by
/// longest-prefix match over `git worktree list`. Anchoring to the invoking
/// worktree is wrong when a caller names a path in another tree; anchoring to
/// the main worktree is worse, because a target inside a linked worktree
/// shares no prefix with it and would get no rule at all.
///
/// One consequence is unavoidable: a same-named path at the same depth in a
/// sibling worktree is hidden too. A shared exclude file cannot express
/// "this worktree only", and over-hiding a planr worktree path is the safe
/// direction -- the alternative is a gitlink committed onto trunk.
fn containing_worktree_root(target: &Path, cwd: &Path) -> Option<PathBuf> {
    let out = git_in(cwd, &["worktree", "list", "--porcelain"]).ok()?;
    out.lines()
        .filter_map(|l| l.strip_prefix("worktree "))
        .filter_map(|p| PathBuf::from(p.trim()).canonicalize().ok())
        // Strictly above the target. The rule is written after the worktree
        // exists, so the target is itself a registered worktree by then --
        // matching it against itself would yield an empty relative path and
        // no rule at all.
        .filter(|root| target.starts_with(root) && target != root.as_path())
        // Worktrees nest (planr's default location puts one inside the tree
        // that claimed it), so the deepest match is the containing one.
        .max_by_key(|root| root.components().count())
}

/// The anchored, directory-shaped exclude pattern for `target`, or `None`
/// when it lies outside every working tree -- git never looks there, so no
/// rule is needed and none should be written.
fn exclude_pattern(target: &Path, cwd: &Path) -> Option<String> {
    let base = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
    let abs = canonicalize_existing(&normalize(&if target.is_absolute() {
        target.to_path_buf()
    } else {
        base.join(target)
    }));
    let root = containing_worktree_root(&abs, cwd)?;
    let rel = abs.strip_prefix(&root).ok()?.to_string_lossy().to_string();
    (!rel.is_empty()).then(|| format!("/{}/", glob_escape(&rel.replace('\\', "/"))))
}

/// Escape the glob metacharacters gitignore gives meaning to.
///
/// A pattern is a glob, not a literal path: a worktree at `wt[1]` written
/// verbatim becomes a character class that matches `wt1` and leaves the real
/// directory visible -- and therefore staged as a gitlink. `/` is left alone;
/// it is the path separator the anchoring depends on.
fn glob_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(c, '*' | '?' | '[' | ']' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Path to this repository's `.git/info/exclude`, creating `info/` if needed.
fn exclude_file(cwd: &Path) -> Result<PathBuf, String> {
    let info = PathBuf::from(git_common_dir(cwd)?).join("info");
    std::fs::create_dir_all(&info).map_err(|e| format!("cannot create {}: {e}", info.display()))?;
    Ok(info.join("exclude"))
}

/// Ignore `target` in this clone only, via `.git/info/exclude`.
///
/// Used for worktrees that land inside the working tree. Such a worktree is
/// an embedded repo, so without a rule `git add` stages it as a `160000`
/// gitlink -- a bogus submodule that rides through every merge and that a
/// fresh clone cannot resolve -- and the tree reads dirty until someone runs
/// `git rm --cached`. Hiding an embedded repo takes a pattern in an
/// ancestor's ignore rules; a `.gitignore` inside it cannot work, since git
/// detects the gitlink from the `.git` file, not the directory contents.
///
/// The rule is local rather than a tracked `.gitignore` because a worktree
/// is local: it exists in this clone alone. That also leaves nothing new in
/// the working tree for anyone to notice or commit.
pub fn exclude_add(target: &Path, cwd: &Path) -> Result<(), String> {
    let Some(pattern) = exclude_pattern(target, cwd) else {
        return Ok(());
    };
    let path = exclude_file(cwd)?;
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    if existing.lines().any(|l| l.trim() == pattern) {
        return Ok(());
    }

    let mut out = existing;
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    if !out.contains(EXCLUDE_HEADER) {
        out.push_str(&format!("\n{EXCLUDE_HEADER}\n"));
    }
    out.push_str(&pattern);
    out.push('\n');
    std::fs::write(&path, out).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

/// Drop the local ignore rule for `target`, if one is present.
///
/// A rule outlives the worktree it was written for, and a stale rule is not
/// harmless: it silently hides anything later created at that path. Only an
/// exact match is removed, so a broader rule covering a parent directory
/// (the default `<plan-dir>/worktrees/`, which planr owns and reuses for
/// every claim) is left in place.
pub fn exclude_remove(target: &Path, cwd: &Path) -> Result<(), String> {
    let Some(pattern) = exclude_pattern(target, cwd) else {
        return Ok(());
    };
    let path = exclude_file(cwd)?;
    let Ok(existing) = std::fs::read_to_string(&path) else {
        return Ok(());
    };
    if !existing.lines().any(|l| l.trim() == pattern) {
        return Ok(());
    }

    let mut kept: Vec<&str> = existing
        .lines()
        .filter(|l| l.trim() != pattern)
        .collect::<Vec<_>>();
    // Drop the header once it no longer introduces anything.
    if !kept.iter().any(|l| l.starts_with('/')) {
        kept.retain(|l| l.trim() != EXCLUDE_HEADER);
        while kept.last().map(|l| l.trim().is_empty()).unwrap_or(false) {
            kept.pop();
        }
    }
    let mut out = kept.join("\n");
    if !out.is_empty() {
        out.push('\n');
    }
    std::fs::write(&path, out).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

const EXCLUDE_HEADER: &str = "# planr worktrees -- checkouts, not backlog content";

/// `git worktree remove <path> [--force]`.
pub fn worktree_remove(path: &Path, force: bool) -> Result<(), String> {
    let mut args = vec!["worktree", "remove"];
    if force {
        args.push("--force");
    }
    args.push(path.to_str().unwrap_or_default());
    git(&args).map(|_| ())
}

/// `git worktree prune` run in `cwd`.
///
/// git keeps listing a worktree whose directory was deleted by hand until
/// something prunes the record, and a stale record is indistinguishable from
/// a live one when asking which worktree holds a branch.
pub fn worktree_prune(cwd: &Path) -> Result<(), String> {
    git_in(cwd, &["worktree", "prune"]).map(|_| ())
}

/// `git branch -d|-D <branch>` run in `cwd`.
///
/// The cwd matters: `-d` (safe delete) refuses unless the branch is merged
/// into the HEAD of the worktree it runs in, so this must run in the worktree
/// where the merge landed (trunk), not wherever planr was invoked from.
pub fn branch_delete(branch: &str, force: bool, cwd: &Path) -> Result<(), String> {
    let flag = if force { "-D" } else { "-d" };
    git_in(cwd, &["branch", flag, branch]).map(|_| ())
}

/// `git merge --no-ff <branch>`.
#[allow(dead_code)]
pub fn merge_no_ff(branch: &str) -> Result<String, String> {
    git(&["merge", "--no-ff", branch])
}

/// `git checkout <branch>`.
#[allow(dead_code)]
pub fn checkout(branch: &str) -> Result<(), String> {
    git(&["checkout", branch]).map(|_| ())
}

/// `git checkout <branch>` run in `cwd`.
pub fn checkout_in(cwd: &Path, branch: &str) -> Result<(), String> {
    git_in(cwd, &["checkout", branch]).map(|_| ())
}

/// `git commit [-m <message>] [-- files...]`.
/// `git add <file>` (run in `cwd`).
pub fn add_file(file: &str, cwd: &Path) -> Result<(), String> {
    git_in(cwd, &["add", "--", file]).map(|_| ())
}

/// `git commit -m <message>` (run in `cwd`, no extra file args).
pub fn commit_in(message: &str, cwd: &Path) -> Result<(), String> {
    git_in(cwd, &["commit", "-m", message]).map(|_| ())
}

#[allow(dead_code)]
pub fn commit(message: &str, files: &[&str]) -> Result<(), String> {
    let mut args = vec!["commit", "-m", message];
    if !files.is_empty() {
        args.push("--");
        args.extend_from_slice(files);
    }
    git(&args).map(|_| ())
}

/// `git diff <ref1>..<ref2>`.
pub fn diff_refs(ref1: &str, ref2: &str) -> Result<String, String> {
    git(&["diff", &format!("{ref1}..{ref2}")])
}

/// `git branch --list [pattern]`, one plain branch name per line.
///
/// Uses `--format` rather than parsing the decorated output. `git branch`
/// prefixes each line with a marker -- `* ` for HEAD, `+ ` for a branch
/// checked out in a linked worktree, two spaces otherwise -- and every
/// branch planr creates is a worktree branch, so the `+ ` case is the
/// common one, not the exotic one. Asking git for `%(refname:short)`
/// sidesteps the decoration entirely.
pub fn branch_list(pattern: Option<&str>) -> Result<Vec<String>, String> {
    let mut args = vec!["branch", "--list", "--format=%(refname:short)"];
    if let Some(p) = pattern {
        args.push(p);
    }
    let out = git(&args)?;
    Ok(out
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect())
}

/// `git worktree list --porcelain`. Returns raw porcelain lines.
pub fn worktree_list() -> Result<Vec<String>, String> {
    let out = git(&["worktree", "list", "--porcelain"])?;
    Ok(out
        .lines()
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect())
}

/// `git rev-parse --verify <ref>`. Returns the full SHA on success.
pub fn rev_parse_verify(ref_: &str) -> Result<String, String> {
    git(&["rev-parse", "--verify", ref_])
}

/// `git rev-parse --show-toplevel`: absolute path to the working-tree root
/// containing the process cwd.
pub fn show_toplevel() -> Result<String, String> {
    Ok(git(&["rev-parse", "--show-toplevel"])?.trim().to_string())
}

/// `git rev-parse --short <ref>`: abbreviated commit id for a commit-ish.
pub fn rev_parse_short(ref_: &str) -> Result<String, String> {
    Ok(git(&["rev-parse", "--short", ref_])?.trim().to_string())
}

/// Current branch name, or `None` when HEAD is detached (abbrev-ref == "HEAD").
pub fn current_branch() -> Option<String> {
    let name = git(&["rev-parse", "--abbrev-ref", "HEAD"])
        .ok()?
        .trim()
        .to_string();
    (!name.is_empty() && name != "HEAD").then_some(name)
}

/// Whether the working tree has uncommitted changes (tracked or untracked),
/// via `git status --porcelain`.
pub fn is_dirty() -> Result<bool, String> {
    Ok(!git(&["status", "--porcelain"])?.trim().is_empty())
}

/// Find the worktree path where `branch` is currently checked out, if any.
/// Parses `git worktree list --porcelain`, pairing each `worktree <path>`
/// stanza with its `branch refs/heads/<branch>` line.
pub fn find_worktree_for_branch(branch: &str) -> Option<PathBuf> {
    let lines = worktree_list().ok()?;
    let branch_ref = format!("refs/heads/{branch}");
    let mut current_wt: Option<PathBuf> = None;

    for line in &lines {
        if let Some(path) = line.strip_prefix("worktree ") {
            current_wt = Some(PathBuf::from(path));
        } else if line.strip_prefix("branch ") == Some(branch_ref.as_str()) {
            return current_wt;
        }
    }
    None
}

/// Resolve a working directory that has `trunk` checked out, for trunk-local
/// writes and commits.
///
/// If `trunk` is already checked out in some worktree -- the common case, the
/// leader's main worktree -- return that path with no checkout, so the caller
/// can write and commit there even when planr was invoked from another
/// worktree on a different branch. `git checkout <trunk>` cannot be used from
/// such a worktree because trunk is already used elsewhere. If trunk is not
/// checked out anywhere, check it out in `cwd` and return `cwd`.
pub fn trunk_worktree(trunk: &str, cwd: &Path) -> Result<PathBuf, String> {
    if let Some(path) = find_worktree_for_branch(trunk) {
        Ok(path)
    } else {
        git_in(cwd, &["checkout", trunk])?;
        Ok(cwd.to_path_buf())
    }
}

/// Discover the git common directory, trimming trailing `/` and resolving
/// relative paths against `cwd` (matching TS `gitCommonDir`).
pub fn git_common_dir(cwd: &Path) -> Result<String, String> {
    let out = git_in(cwd, &["rev-parse", "--git-common-dir"])?;
    let gd = out.trim().trim_end_matches('/');
    let path = std::path::Path::new(gd);
    if path.is_relative() {
        let abs = cwd.join(path);
        Ok(abs.to_string_lossy().to_string())
    } else {
        Ok(gd.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn with_temp_repo<F: FnOnce(&TempDir, &Path)>(f: F) {
        let tmp = TempDir::new().unwrap();
        let repo = tmp.path().join("repo");
        fs::create_dir_all(&repo).unwrap();
        git_in(&repo, &["init", "-b", "main"]).unwrap();
        git_in(&repo, &["config", "user.email", "test@test"]).unwrap();
        git_in(&repo, &["config", "user.name", "Test"]).unwrap();
        fs::write(repo.join("README.md"), "# test").unwrap();
        git_in(&repo, &["add", "."]).unwrap();
        git_in(&repo, &["commit", "-m", "init"]).unwrap();
        f(&tmp, &repo);
    }

    #[test]
    fn test_git_common_dir() {
        with_temp_repo(|_tmp, repo| {
            let gd = git_common_dir(repo).unwrap();
            assert!(gd.ends_with(".git"), "gd = {gd}");
            let p = std::path::Path::new(&gd);
            assert!(p.is_absolute(), "git-common-dir should be absolute: {gd}");
        });
    }
}
