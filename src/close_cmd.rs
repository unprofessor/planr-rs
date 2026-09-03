//! `planr close` -- complete a ticket: gate-check, flip to done, merge.
//!
//! Three routing paths:
//! - `close task <slug>` -- branch-backed: guards + exclusive lock -> done flip
//!   on branch -> checkout trunk -> merge --no-ff -> cleanup
//! - `close story <slug>` -- trunk-local: child-task gate -> done flip -> commit
//! - `close epic <slug>` -- trunk-local: child-story gate -> done flip -> commit

use crate::git;
use crate::lock::PlanrLock;
use crate::parse::extract_last_review_verdict;
use crate::ticket::{parse_ticket, ParsedTicket};
use std::path::Path;
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

fn split_fm(blob: &str) -> Option<FmSplit<'_>> {
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
    let _verify =
        git::rev_parse_verify(&branch).map_err(|_| format!("no such branch: {branch}"))?;

    let task_file = find_task_file_on_branch(&branch, slug, plan_dir)?;

    let blob = git::show_ref(&branch, &task_file)?;
    let ticket = parse_ticket(&blob);
    let parent_story = ticket.parent.clone();

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
    let _lock = PlanrLock::exclusive(cwd).map_err(|e| format!("lock error: {e}"))?;

    let wt_path = git::find_worktree_for_branch(&branch);

    // Resolve the worktree that holds trunk; the merge (and, when the task has
    // no worktree of its own, the status flip) happen there so `close` works
    // even when planr is invoked from another worktree on a different branch.
    let trunk_dir = git::trunk_worktree(trunk, cwd)?;

    // Flip status on the branch
    let branch_content = git::show_ref(&branch, &task_file)?;
    let date = local_date_string();
    let new_content = flip_frontmatter(&branch_content, "done", &date)?;

    // Write to the task's own worktree if it has one; otherwise check the
    // branch out in the trunk worktree, commit the flip, then return trunk
    // there before merging.
    if let Some(ref wt) = wt_path {
        let fpath = wt.join(&task_file);
        std::fs::write(&fpath, &new_content)
            .map_err(|e| format!("cannot write {}: {e}", fpath.display()))?;
        git::add_file(&task_file, wt)?;
        git::commit_in(&format!("plan: mark {slug} done"), wt)?;
    } else {
        git::checkout_in(&trunk_dir, &branch)?;
        let fpath = trunk_dir.join(&task_file);
        std::fs::write(&fpath, &new_content)
            .map_err(|e| format!("cannot write {}: {e}", fpath.display()))?;
        git::add_file(&task_file, &trunk_dir)?;
        git::commit_in(&format!("plan: mark {slug} done"), &trunk_dir)?;
        git::checkout_in(&trunk_dir, trunk)?;
    }

    // Merge --no-ff with custom message, in the trunk worktree.
    let merge_result = try_merge(
        &branch,
        &format!("plan: merge {slug}"),
        trunk,
        slug,
        &trunk_dir,
    );
    match merge_result {
        Ok(_) => {
            // Flip is already on the merged branch commit; the merge brings it to trunk.
            // Cleanup: tolerant worktree remove + branch delete
            if let Some(ref wt) = wt_path {
                // The ignore rule may only go once the worktree it hides is
                // actually gone. `worktree remove` without --force refuses
                // whenever the worktree holds untracked or modified files --
                // a stray build artifact or log is enough -- and dropping the
                // rule anyway would leave the worktree in place and unhidden,
                // which is the gitlink corruption the rule exists to prevent.
                // A stale rule is the lesser harm, and the next close of that
                // path clears it. (The exact conditions under which the rule
                // may go are on the arms below, which is where they are
                // enforced.)
                //
                // First, though: never remove a worktree that holds another
                // one. `git worktree remove` decides it is safe by asking
                // `git status --porcelain`, which does not list ignored paths
                // -- and planr's own rule hides `<plan-dir>/worktrees/` inside
                // every working tree. A worker that claims from inside its own
                // worktree nests one there by default, so git's safety check
                // cannot see it and deletes it recursively, uncommitted work
                // and all. Without the rule git refuses; with it, closing the
                // parent destroys the child in silence.
                match git::worktrees_under(wt, cwd) {
                    Ok(nested) if !nested.is_empty() => {
                        let paths: Vec<String> =
                            nested.iter().map(|p| p.display().to_string()).collect();
                        eprintln!(
                            "warning: merged, but the worktree at {} was left in place: \
                             it holds {} live worktree(s) ({}). Removing it would delete \
                             them and any uncommitted work in them. Close or remove those \
                             first, then `git worktree remove {}` and \
                             `git branch -d {branch}` -- until the worktree goes the \
                             branch cannot be deleted either, so `planr board` keeps \
                             listing {slug} in flight",
                            wt.display(),
                            nested.len(),
                            paths.join(", "),
                            wt.display()
                        );
                    }
                    Err(e) => {
                        eprintln!(
                            "warning: merged, but the worktree at {} was left in place: \
                             could not check whether it holds other worktrees ({e}); \
                             remove it by hand once you have",
                            wt.display()
                        );
                    }
                    Ok(_) => match git::worktree_remove(wt, false) {
                        Ok(()) => {
                            // The shared default rule covers a parent directory,
                            // so it does not match here and survives.
                            git::drop_exclude(wt, cwd);
                        }
                        // A stale record: the directory is gone *and* git has
                        // forgotten it, so there is nothing left to hide and
                        // keeping the rule would hide whatever is created at that
                        // path next.
                        //
                        // Both halves are load-bearing. `try_exists().unwrap_or
                        // (true)` keeps an I/O error from reading as "the
                        // directory is gone". And the record has to be checked
                        // too: a *locked* worktree whose directory was deleted
                        // fails removal while staying registered, so judging by
                        // the directory alone dropped the rule, let the branch
                        // delete below fail silently, and reported unqualified
                        // success while `board` still listed the task in flight.
                        Err(_)
                            if !wt.try_exists().unwrap_or(true)
                                && git::find_worktree_for_branch(&branch).is_none() =>
                        {
                            git::drop_exclude(wt, cwd);
                        }
                        Err(e) => {
                            // The merge succeeded, so this is not a failure of the
                            // close -- but it is not nothing either. The worktree
                            // survives, `git branch -d` below will refuse to
                            // delete a branch checked out in it, and `planr board`
                            // will keep listing the task as in flight. Silence
                            // here made `close` report unqualified success while
                            // the board contradicted it.
                            eprintln!(
                            "warning: merged, but the worktree at {} could not be removed ({e}); \
                             it and branch {branch} remain -- `git worktree remove --force {}` \
                             to finish cleaning up",
                            wt.display(),
                            wt.display()
                        );
                        }
                    },
                }
            }
            let _ = git::branch_delete(&branch, false, &trunk_dir);

            // Check if parent story can also be closed
            if let Some(ref pslug) = parent_story {
                let siblings =
                    find_children_on_trunk(pslug, "tasks", trunk, plan_dir).unwrap_or_default();
                let all_done = siblings.iter().all(|t| t.status == "done");
                if all_done && !siblings.is_empty() {
                    Ok(format!(
                        "merged {branch} into {trunk}; {slug} done\n\
                         info: all tasks under story '{pslug}' are done. \
                         you may also close parent story: planr close story {pslug}"
                    ))
                } else {
                    Ok(format!("merged {branch} into {trunk}; {slug} done"))
                }
            } else {
                Ok(format!("merged {branch} into {trunk}; {slug} done"))
            }
        }
        Err(conflict_msg) => {
            // On conflict the merge was aborted; worktree+branch intact
            // for the worker to rebase.
            Err(conflict_msg)
        }
    }
}

/// Try a merge, capturing the full output for conflict reporting. Runs in
/// `cwd` (the worktree that has trunk checked out).
fn try_merge(
    branch: &str,
    message: &str,
    trunk: &str,
    slug: &str,
    cwd: &Path,
) -> Result<String, String> {
    let mut cmd = Command::new("git");
    cmd.current_dir(cwd);
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
            .current_dir(cwd)
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
            .current_dir(cwd)
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
        err.push_str(&format!(
            "  git rebase {trunk}   # resolve conflicts, git rebase --continue\n"
        ));
        err.push_str(&format!("  # then re-run: planr close task {slug}\n"));

        Err(err)
    }
}

// ---------------------------------------------------------------------------
// close story <slug>
// ---------------------------------------------------------------------------

pub fn close_story(slug: &str, trunk: &str, plan_dir: &str, cwd: &Path) -> Result<String, String> {
    // Read story ticket to capture parent before lock/gate
    let story_file = find_ticket_by_slug(slug, "stories", trunk, plan_dir)?;
    let blob = git::show_ref(trunk, &story_file)?;
    let ticket = parse_ticket(&blob);
    let parent_epic = ticket.parent.clone();

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
    let _lock = PlanrLock::exclusive(cwd).map_err(|e| format!("lock error: {e}"))?;
    flip_and_commit_kind(slug, "stories", trunk, plan_dir, cwd)?;

    // Check if parent epic can also be closed
    if let Some(ref pslug) = parent_epic {
        let siblings =
            find_children_on_trunk(pslug, "stories", trunk, plan_dir).unwrap_or_default();
        let all_done = siblings.iter().all(|t| t.status == "done");
        if all_done && !siblings.is_empty() {
            Ok(format!(
                "closed story {slug}; all tasks done\n\
                 info: all stories under epic '{pslug}' are done. \
                 you may also close parent epic: planr close epic {pslug}"
            ))
        } else {
            Ok(format!("closed story {slug}; all tasks done"))
        }
    } else {
        Ok(format!("closed story {slug}; all tasks done"))
    }
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
    let _lock = PlanrLock::exclusive(cwd).map_err(|e| format!("lock error: {e}"))?;

    flip_and_commit_kind(slug, "epics", trunk, plan_dir, cwd)?;

    Ok(format!("closed epic {slug}; all stories done"))
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Find a ticket file on trunk under a given kind directory by slug.
pub(crate) fn find_ticket_by_slug(
    slug: &str,
    kind_dir: &str,
    trunk: &str,
    plan_dir: &str,
) -> Result<String, String> {
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
fn flip_and_commit_kind(
    slug: &str,
    kind_dir: &str,
    trunk: &str,
    plan_dir: &str,
    cwd: &Path,
) -> Result<(), String> {
    let file = find_ticket_by_slug(slug, kind_dir, trunk, plan_dir)?;

    // Resolve the working directory that has trunk checked out (possibly a
    // different worktree) so the commit lands on the authoritative backlog,
    // even when planr was invoked from a task worktree on another branch.
    let trunk_dir = git::trunk_worktree(trunk, cwd)?;

    let blob = git::show_ref(trunk, &file)?;
    let date = local_date_string();
    let new_content = flip_frontmatter(&blob, "done", &date)?;

    // The file is available in the trunk working tree.
    let fpath = trunk_dir.join(&file);
    let parent = fpath.parent().unwrap_or(&trunk_dir);
    std::fs::create_dir_all(parent)
        .map_err(|e| format!("cannot create dir {}: {e}", parent.display()))?;
    std::fs::write(&fpath, &new_content)
        .map_err(|e| format!("cannot write {}: {e}", fpath.display()))?;

    git::add_file(&file, &trunk_dir)?;
    git::commit_in(&format!("plan: close {kind_dir} {slug}"), &trunk_dir)?;

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
