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
    let out = run_git_raw(cwd, args)?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    } else {
        Err(last_stderr_line(&out))
    }
}

/// A `git` child process with its messages pinned to English.
///
/// git translates its own diagnostics, and planr reads them two ways that
/// both break under a translated locale: `toplevel_or_none` tells "there is
/// no repository here" from a real failure by matching git's wording, and
/// every other wrapper puts git's last stderr line inside a planr sentence
/// that is English either way. Left to the environment, an ordinary run
/// outside a repository under `LC_ALL=fr_FR.UTF-8` was reported as a git
/// failure -- exactly the noise the match exists to avoid -- and warnings
/// came out half in one language and half in the other.
///
/// `LC_ALL=C` is the pin: it outranks `LC_MESSAGES`, and GNU gettext ignores
/// `LANGUAGE` entirely once the locale is `C`, so nothing else in the
/// environment can put the translation back. It is set on every git planr
/// runs, not just the one that matches, because a message planr is about to
/// quote is a message planr has to be able to read.
pub(crate) fn git_command() -> Command {
    let mut cmd = Command::new("git");
    cmd.env("LC_ALL", "C");
    cmd
}

/// The raw process result, for the one caller that has to read all of git's
/// stderr rather than the last line of it.
fn run_git_raw(cwd: Option<&Path>, args: &[&str]) -> Result<std::process::Output, String> {
    let mut cmd = git_command();
    cmd.args(args);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    cmd.output().map_err(|e| format!("git command failed: {e}"))
}

/// The last non-empty line of git's stderr -- the part that usually names the
/// problem, and short enough to put in one warning.
fn last_stderr_line(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stderr)
        .lines()
        .rfind(|l| !l.trim().is_empty())
        .unwrap_or("git failed")
        .to_string()
}

/// List every file under `dir` at `ref`, whatever its extension.
///
/// The unfiltered listing answers a question the `.md` one cannot: whether
/// anything is there at all. A backlog scaffolded with `.gitkeep` files and
/// no tickets yet reads as zero tickets, and that is not the same fact as a
/// plan directory that is not in the commit.
pub fn ls_tree(ref_: &str, dir: &str) -> Result<Vec<String>, String> {
    let out = git(&["ls-tree", "-r", "--name-only", ref_, "--", dir])?;
    Ok(out
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect())
}

/// List all `.md` files under `dir` at `ref` (e.g. `HEAD:.plan`).
pub fn ls_tree_md(ref_: &str, dir: &str) -> Result<Vec<String>, String> {
    Ok(ls_tree(ref_, dir)?
        .into_iter()
        .filter(|l| l.ends_with(".md"))
        .collect())
}

/// Show a single blob at `ref:path`.
pub fn show_ref(ref_: &str, path: &str) -> Result<String, String> {
    git(&["show", &format!("{ref_}:{path}")])
}

/// Whether `refs/heads/<branch>` already exists.
pub fn branch_exists(branch: &str) -> bool {
    git(&["rev-parse", "--verify", &format!("refs/heads/{branch}")]).is_ok()
}

/// `git worktree add <path> [-b] <branch> <commit-ish>`.
///
/// `ref_` is the commit-ish to branch *from*, and applies only when the
/// branch is being created. Once the branch exists it is its own starting
/// point: passing `ref_` there would ask git to check out trunk in the new
/// worktree, which fails with "'<trunk>' is already used by worktree at
/// ..." because trunk is checked out already. Naming the branch explicitly
/// also stops git from inferring one from the path basename.
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

/// `git worktree remove <path> [--force]`.
pub fn worktree_remove(path: &Path, force: bool) -> Result<(), String> {
    let mut args = vec!["worktree", "remove"];
    if force {
        args.push("--force");
    }
    args.push(path.to_str().unwrap_or_default());
    git(&args).map(|_| ())
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
/// common one, not the exotic one. Asking git for the ref name sidesteps
/// the decoration entirely.
///
/// `lstrip=2`, not `:short`: the short form is the shortest *unambiguous*
/// name, so a tag sharing a branch's name makes it report `heads/plan/x`.
/// Stripping the two leading components yields the branch name whatever
/// else exists.
pub fn branch_list(pattern: Option<&str>) -> Result<Vec<String>, String> {
    let mut args = vec!["branch", "--list", "--format=%(refname:lstrip=2)"];
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

/// The repository root, with "there is no repository here" as its own answer.
///
/// Git reports both outcomes as exit 128 and only the message tells them
/// apart, so the message has to be read in full. Its discovery failure says
/// "not a git repository" on the first line and, when the search stops at a
/// mount point, adds "Stopping at filesystem boundary" after it -- which is
/// the line that survives when only the last one is kept. Matching that alone
/// reported an ordinary run outside a repository as a git failure, which is
/// exactly the noise the caller is trying not to make.
///
/// Matching English text is only sound because `git_command` pins the child
/// to `LC_ALL=C`. Reading whatever the environment's locale produced made
/// the same spurious warning on every ordinary non-repository run under a
/// translated locale -- the very case this function exists to keep quiet.
pub fn toplevel_or_none() -> Result<Option<String>, String> {
    let out = run_git_raw(None, &["rev-parse", "--show-toplevel"])?;
    if out.status.success() {
        return Ok(Some(
            String::from_utf8_lossy(&out.stdout).trim().to_string(),
        ));
    }
    if String::from_utf8_lossy(&out.stderr).contains("not a git repository") {
        return Ok(None);
    }
    Err(last_stderr_line(&out))
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
pub(crate) mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    pub(crate) fn with_temp_repo<F: FnOnce(&TempDir, &Path)>(f: F) {
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
