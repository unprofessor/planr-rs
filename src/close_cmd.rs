//! `planr close` — complete a ticket: gate-check, flip to done, merge.
//!
//! Three routing paths:
//! - `close task <slug>` — branch-backed: guards + exclusive lock → done flip
//!   on branch → checkout trunk → merge --no-ff → cleanup
//! - `close story <slug>` — trunk-local: child-task gate → done flip → commit
//! - `close epic <slug>` — trunk-local: child-story gate → done flip → commit

use crate::git;
use crate::lock::PlanrLock;
use crate::parse::extract_last_review_verdict;
use crate::ticket::{parse_ticket, ParsedTicket};
use std::path::{Path, PathBuf};
use std::process::Command;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Find child tickets of a given parent on trunk by scanning files in a
/// plan subdirectory and reading the `parent:` field.
fn find_children_on_trunk(
    parent_slug: &str,
    kind_dir: &str, // e.g. "tasks" or "stories"
    trunk: &str,
    plan_dir: &str,
) -> Result<Vec<ParsedTicket>, String> {
    let dir = format!("{plan_dir}/{kind_dir}");
    let files = git::ls_tree_md(trunk, &dir).unwrap_or_default();
    let mut children = Vec::new();
    for f in &files {
        let blob = match git::show_ref(trunk, f) {
            Ok(b) => b,
            Err(_) => continue,
        };
        let ticket = parse_ticket(&blob);
        if ticket.parent.as_deref() == Some(parent_slug) {
            children.push(ticket);
        }
    }
    Ok(children)
}

/// Find the task file on a branch using NN-regex match.
fn find_task_file_on_branch(branch: &str, slug: &str, plan_dir: &str) -> Result<String, String> {
    let files = git::ls_tree_md(branch, &format!("{plan_dir}/tasks")).unwrap_or_default();
    let pattern = format!(r"/[0-9]+-{}\.md$", regex::escape(slug));
    let re = regex::Regex::new(&pattern).unwrap();
    files
        .into_iter()
        .find(|f| re.is_match(f))
        .ok_or_else(|| format!("no task file for '{slug}' on {branch}"))
}

/// Local date string (YYYY-MM-DD).
fn local_date_string() -> String {
    let now = jiff::Zoned::now();
    format!("{:04}-{:02}-{:02}", now.year(), now.month(), now.day())
}

/// Frontmatter-scoped flip: status + updated, with insert-if-absent
/// (same pattern as claim.rs).
fn flip_frontmatter(content: &str, new_status: &str, date: &str) -> Result<String, String> {
    let sf = split_fm(content).ok_or_else(|| "no frontmatter".to_string())?;
    let mut has_status = false;
    let mut has_updated = false;
    let mut out: Vec<String> = Vec::with_capacity(sf.fm_lines.len() + 2);

    for line in &sf.fm_lines {
        if line.starts_with("status:") {
            out.push(format!("status: {new_status}"));
            has_status = true;
        } else if line.starts_with("updated:") {
            out.push(format!("updated: {date}"));
            has_updated = true;
        } else {
            out.push(line.to_string());
        }
    }
    // TS order: check hasStatus first (unshift), then hasUpdated (unshift puts
    // updated ABOVE status).
    if !has_status {
        out.insert(0, format!("status: {new_status}"));
    }
    if !has_updated {
        out.insert(0, format!("updated: {date}"));
    }

    let fm_str = out.join("\n");
    Ok(format!("---\n{fm_str}\n---\n{}", sf.rest))
}

struct FmSplit<'a> {
    fm_lines: Vec<&'a str>,
    rest: &'a str,
}

fn split_fm(blob: &str) -> Option<FmSplit> {
    if !blob.starts_with("---\n") {
        return None;
    }
    let end = blob[4..].find("\n---\n")?;
    let fm_end = 4 + end;
    let fm_str = &blob[4..fm_end];
    let rest = &blob[fm_end + 5..];
    let fm_lines: Vec<&str> = fm_str.lines().collect();
    Some(FmSplit { fm_lines, rest })
}

// ---------------------------------------------------------------------------
// close task <slug>
// ---------------------------------------------------------------------------

pub fn close_task(slug: &str, trunk: &str, plan_dir: &str, cwd: &Path) -> Result<String, String> {
    let branch = format!("plan/{slug}");

    // ---- 1. Guards (read from branch, no lock) ----

    // Branch exists
    let _verify = git::rev_parse_verify(&branch).map_err(|_| format!("no such branch: {branch}"))?;

    let task_file = find_task_file_on_branch(&branch, slug, plan_dir)?;

    let blob = git::show_ref(&branch, &task_file)?;
    let ticket = parse_ticket(&blob);

    if ticket.status != "review" {
        return Err(format!(
            "refuse merge: task '{slug}' status is '{}', must be 'review'.\n\
             the worker must self-validate against ## Acceptance (record ## Validation) \
             and set status: review.",
            ticket.status
        ));
    }

    let verdict = extract_last_review_verdict(&ticket.raw);
    if verdict.as_deref() != Some("approved") {
        return Err(format!(
            "refuse merge: no approved review verdict on '{slug}' (found: '{}').\n\
             assign a reviewer: planr review {slug}",
            verdict.as_deref().unwrap_or("none")
        ));
    }

    // ---- 2. Under exclusive lock ----
    let _lock = PlanrLock::exclusive(cwd)
        .map_err(|e| format!("lock error: {e}"))?;

    let wt_path = find_worktree_path(&branch);

    // Flip status on the branch
    let branch_content = git::show_ref(&branch, &task_file)?;
    let date = local_date_string();
    let new_content = flip_frontmatter(&branch_content, "done", &date)?;

    // Write to the worktree (if found) or to the branch (via checkout)
    if let Some(ref wt) = wt_path {
        let fpath = Path::new(wt).join(&task_file);
        std::fs::write(&fpath, &new_content)
            .map_err(|e| format!("cannot write {}: {e}", fpath.display()))?;
        git::add_file(&task_file, Path::new(wt))?;
        git::commit_in(&format!("plan: mark {slug} done"), Path::new(wt))?;
    } else {
        // No worktree — need to do this differently.
        // Checkout branch, write, commit, then merge.
        git::checkout(&branch)?;
        let fpath = Path::new(&task_file);
        std::fs::write(fpath, &new_content)
            .map_err(|e| format!("cannot write {task_file}: {e}"))?;
        git::add_file(&task_file, Path::new("."))?;
        git::commit_in(&format!("plan: mark {slug} done"), Path::new("."))?;
    }

    // Checkout trunk and merge
    git::checkout(trunk)?;

    // Merge --no-ff with custom message
    let merge_result = try_merge(&branch, &format!("plan: merge {slug}"), trunk, slug);
    match merge_result {
        Ok(_) => {
            // Flip is already on the merged branch commit; the merge brings it to trunk.
            // Cleanup: tolerant worktree remove + branch delete
            if let Some(ref wt) = wt_path {
                let _ = git::worktree_remove(Path::new(wt), false);
            }
            let _ = git::branch_delete(&branch, false);

            Ok(format!("merged {branch} into {trunk}; {slug} done"))
        }
        Err(conflict_msg) => {
            // On conflict the merge was aborted; worktree+branch intact
            // for the worker to rebase.
            Err(conflict_msg)
        }
    }
}

/// Try a merge, capturing the full output for conflict reporting.
fn try_merge(branch: &str, message: &str, trunk: &str, slug: &str) -> Result<String, String> {
    let mut cmd = Command::new("git");
    cmd.args(["merge", "--no-ff", branch, "-m", message]);

    let out = cmd.output().map_err(|e| format!("git merge failed: {e}"))?;

    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    } else {
        // Capture merge log (stdout + stderr)
        let stdout = String::from_utf8_lossy(&out.stdout);
        let stderr = String::from_utf8_lossy(&out.stderr);
        let full_log = format!("{stdout}{stderr}").trim().to_string();

        // List conflicted files
        let conflicted = Command::new("git")
            .args(["diff", "--name-only", "--diff-filter=U"])
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "<unknown>".to_string());

        // Abort the merge
        let _ = Command::new("git")
            .args(["merge", "--abort"])
            .output();

        // Build error message with guidance
        let mut err = String::new();
        if !full_log.is_empty() {
            err.push_str(&full_log);
            err.push('\n');
        }
        err.push('\n');
        err.push_str(&format!("merge conflict in: {conflicted}\n\n"));
        err.push_str("The worker must rebase onto fresh trunk and resolve:\n");
        err.push_str(&format!("  git rebase {trunk}   # resolve conflicts, git rebase --continue\n"));
        err.push_str(&format!("  # then re-run: planr close task {slug}\n"));

        Err(err)
    }
}

/// Detect the worktree path for a branch from `worktree list --porcelain`.
fn find_worktree_path(branch: &str) -> Option<PathBuf> {
    let lines = git::worktree_list().ok()?;
    let branch_ref = format!("refs/heads/{branch}");
    let mut current_wt: Option<PathBuf> = None;

    for line in &lines {
        if let Some(path) = line.strip_prefix("worktree ") {
            current_wt = Some(PathBuf::from(path));
        } else if line.strip_prefix("branch ") == Some(&branch_ref) {
            return current_wt;
        }
    }
    None
}

// ---------------------------------------------------------------------------
// close story <slug>
// ---------------------------------------------------------------------------

pub fn close_story(slug: &str, trunk: &str, plan_dir: &str, cwd: &Path) -> Result<String, String> {
    // Child task gate
    let children = find_children_on_trunk(slug, "tasks", trunk, plan_dir)?;
    let unfinished: Vec<String> = children
        .iter()
        .filter(|t| t.status != "done")
        .map(|t| format!("{}({})", t.id, t.status))
        .collect();

    if !unfinished.is_empty() {
        return Err(format!(
            "refuse close: story '{slug}' has unfinished tasks: {}\n\
             finish or reassign these before closing the story.",
            unfinished.join(" ")
        ));
    }

    // Under exclusive lock: flip story status on trunk
    let _lock = PlanrLock::exclusive(cwd)
        .map_err(|e| format!("lock error: {e}"))?;

    flip_and_commit_kind(slug, "stories", trunk, plan_dir)?;

    Ok(format!("closed story {slug}; all tasks done"))
}

// ---------------------------------------------------------------------------
// close epic <slug>
// ---------------------------------------------------------------------------

pub fn close_epic(slug: &str, trunk: &str, plan_dir: &str, cwd: &Path) -> Result<String, String> {
    // Child story gate
    let children = find_children_on_trunk(slug, "stories", trunk, plan_dir)?;
    let unfinished: Vec<String> = children
        .iter()
        .filter(|t| t.status != "done")
        .map(|t| format!("{}({})", t.id, t.status))
        .collect();

    if !unfinished.is_empty() {
        return Err(format!(
            "refuse close: epic '{slug}' has unfinished stories: {}\n\
             complete all stories before closing the epic.",
            unfinished.join(" ")
        ));
    }

    // Under exclusive lock: flip epic status on trunk
    let _lock = PlanrLock::exclusive(cwd)
        .map_err(|e| format!("lock error: {e}"))?;

    flip_and_commit_kind(slug, "epics", trunk, plan_dir)?;

    Ok(format!("closed epic {slug}; all stories done"))
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Find a ticket file on trunk under a given kind directory by slug.
fn find_ticket_by_slug(slug: &str, kind_dir: &str, trunk: &str, plan_dir: &str) -> Result<String, String> {
    let dir = format!("{plan_dir}/{kind_dir}");
    let files = git::ls_tree_md(trunk, &dir).unwrap_or_default();
    let pattern = format!(r"/[0-9]+-{}\.md$", regex::escape(slug));
    let re = regex::Regex::new(&pattern).unwrap();
    files
        .into_iter()
        .find(|f| re.is_match(f))
        .ok_or_else(|| format!("no {kind_dir} file for slug '{slug}' on {trunk}"))
}

/// Flip a trunk-local ticket to done and commit.
fn flip_and_commit_kind(slug: &str, kind_dir: &str, trunk: &str, plan_dir: &str) -> Result<(), String> {
    let file = find_ticket_by_slug(slug, kind_dir, trunk, plan_dir)?;

    // Checkout trunk first (in case we're on a different branch)
    git::checkout(trunk)?;

    let blob = git::show_ref(trunk, &file)?;
    let date = local_date_string();
    let new_content = flip_frontmatter(&blob, "done", &date)?;

    // After checkout, the file is available in the working tree
    let fpath = Path::new(&file);
    let parent = fpath.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent)
        .map_err(|e| format!("cannot create dir {}: {e}", parent.display()))?;
    std::fs::write(fpath, &new_content)
        .map_err(|e| format!("cannot write {file}: {e}"))?;

    git::add_file(&file, Path::new("."))?;
    git::commit_in(&format!("plan: close {kind_dir} {slug}"), Path::new("."))?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ---- frontmatter helpers ----

    #[test]
    fn test_split_fm_simple() {
        let blob = "---\nid: x\nstatus: todo\n---\nbody";
        let sf = split_fm(blob).unwrap();
        assert_eq!(sf.fm_lines[0], "id: x");
        assert_eq!(sf.fm_lines[1], "status: todo");
        assert_eq!(sf.rest, "body");
    }

    #[test]
    fn test_split_fm_no_fm() {
        assert!(split_fm("no frontmatter").is_none());
    }

    #[test]
    fn test_flip_frontmatter_replaces() {
        let content = "---\nid: x\nstatus: review\nupdated: 2026-01-01\n---\nbody\n";
        let result = flip_frontmatter(content, "done", "2026-08-05").unwrap();
        assert!(result.contains("status: done"));
        assert!(result.contains("updated: 2026-08-05"));
        assert!(result.contains("id: x"));
        assert!(result.ends_with("body\n"));
    }

    #[test]
    fn test_flip_frontmatter_inserts_if_absent() {
        let content = "---\nid: x\n---\nbody\n";
        let result = flip_frontmatter(content, "done", "2026-08-05").unwrap();
        let sep = result.find("\n---\n").unwrap();
        let fm_part = &result[4..sep];
        assert!(fm_part.contains("status: done"));
        assert!(fm_part.contains("updated: 2026-08-05"));
    }

    // ---- local_date_string ----

    #[test]
    fn test_local_date_string_format() {
        let s = local_date_string();
        assert_eq!(s.len(), 10);
        assert_eq!(&s[4..5], "-");
        assert_eq!(&s[7..8], "-");
    }
}
