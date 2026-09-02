//! `planr claim` -- claim a task: dependency gate, worktree creation,
//! frontmatter-scoped status flip.
//!
//! Port of `skills/planr/src/claim.ts`.

use crate::git;
use crate::lock::PlanrLock;
use std::path::{Path, PathBuf};

/// The `findTask` predicate from TS: `f.replace(/^\d+-/, "").endsWith(slug + ".md")`.
/// NOTE: on full paths (e.g. `tasks/03-http-proxy.md`), the `^\d+-` strip never
/// fires because the path starts with `<planDir>/`. The effective behavior is
/// "path ends with `<slug>.md`" -- looser than the NN-regex match used by
/// merge-task. Ported as-is; tightening is follow-up, not port scope.
fn find_task_file<'a>(files: &'a [String], slug: &str) -> Option<&'a str> {
    let pat = format!("{}.md", slug);
    files.iter().find(|f| f.ends_with(&pat)).map(|s| s.as_str())
}

/// Statuses that mean work on the branch has already begun. A claim never
/// rewrites one of these -- see the flip in `claim_task`.
const STARTED_STATUSES: [&str; 4] = ["in_progress", "review", "done", "abandoned"];

// ---- frontmatter helpers (simplified; assumes valid YAML frontmatter) ----

/// Helper type for the simplified frontmatter parse used in claim.
struct FmSplit<'a> {
    fm_lines: Vec<&'a str>,
    rest: &'a str,
}

/// Split on `---\n...\n---` -- first block only, no re-entry.
fn split_frontmatter(blob: &str) -> Option<FmSplit<'_>> {
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

/// Read the `status:` line from frontmatter lines.
fn read_status_from_fm<'a>(fm: &'a [&str]) -> &'a str {
    for l in fm {
        if let Some(val) = l.strip_prefix("status:") {
            return val.trim();
        }
    }
    ""
}

/// Parse `depends_on` from frontmatter blob. Supports inline list, bare
/// string, and block-style YAML list (same logic as TS readDeps).
fn read_deps_from_fm(fm: &[&str]) -> Vec<String> {
    let mut deps: Vec<String> = Vec::new();
    for (i, l) in fm.iter().enumerate() {
        let val = if let Some(v) = l.strip_prefix("depends_on:") {
            v.trim()
        } else {
            continue;
        };
        if val.starts_with('[') {
            // inline list: [a, b]
            if let Some(inner) = val.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
                for x in inner.split(',') {
                    let x = x.trim();
                    if !x.is_empty() {
                        deps.push(x.to_string());
                    }
                }
            }
        } else if !val.is_empty() && val != "[]" {
            // bare string
            deps.push(val.to_string());
        }
        // block-style continuation: subsequent lines matching /^\s+-\s+(.+)$/
        for cl in fm.iter().skip(i + 1) {
            if let Some(item) = cl.trim_start().strip_prefix("- ") {
                deps.push(item.trim().to_string());
                continue;
            }
            if cl.trim().is_empty() {
                continue;
            }
            break;
        }
        break;
    }
    deps
}

/// Flip `status:` and `updated:` in frontmatter lines, inserting either if
/// absent. Returns the new frontmatter content and the new full blob.
///
/// TS order: unshift updated then unshift status, so updated lands above
/// status. Real tickets always have both lines, so this is nearly dead code.
fn flip_status_in_fm(fm: &[&str], rest: &str, new_status: &str, date: &str) -> (String, String) {
    let mut has_status = false;
    let mut has_updated = false;
    let mut out: Vec<String> = Vec::with_capacity(fm.len() + 2);

    for line in fm {
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
    // Insert in TS order: check hasStatus first and unshift;
    // then check hasUpdated -- the second unshift puts updated ABOVE status.
    if !has_status {
        out.insert(0, format!("status: {new_status}"));
    }
    if !has_updated {
        out.insert(0, format!("updated: {date}"));
    }

    let fm_str = out.join("\n");
    let full = format!("---\n{fm_str}\n---\n{rest}");
    (fm_str, full)
}

/// Hide the new worktree and flip the task to `in_progress` on its branch.
///
/// Split out so the caller has a single fallible unit to unwind: every step
/// here runs after the worktree exists, and a failure in any of them must
/// leave nothing behind.
fn hide_and_flip(
    wt_path: &Path,
    ignore_target: &Path,
    task_file: &str,
    slug: &str,
    cwd: &Path,
) -> Result<(), String> {
    let added = git::exclude_add(ignore_target, cwd)?;
    if let Err(e) = flip_to_in_progress(wt_path, task_file, slug) {
        // The rule outlives the failed claim otherwise, hiding whatever the
        // user later creates at that path -- invisible in `git status`, which
        // is the hazard the write-after-create ordering was meant to close.
        // Only a rule this call wrote comes back out: the shared default rule
        // is reused by every claim under it.
        if added {
            let _ = git::exclude_remove(ignore_target, cwd);
        }
        return Err(e);
    }
    Ok(())
}

/// Move the task on the branch to `in_progress` and commit it.
fn flip_to_in_progress(wt_path: &Path, task_file: &str, slug: &str) -> Result<(), String> {
    let wf_path = wt_path.join(task_file);
    let content = std::fs::read_to_string(&wf_path)
        .map_err(|e| format!("cannot read {}: {e}", wf_path.display()))?;
    let sf = split_frontmatter(&content).ok_or_else(|| format!("no frontmatter in {task_file}"))?;

    // Flip the status, but never backwards. A resumed claim finds the branch
    // already at in_progress, or further along -- a worker can reach review
    // before the worktree is removed. Rewriting that to in_progress would
    // discard a finished review and then make `close` refuse the task for
    // having the wrong status. Only a branch that has not started work gets
    // flipped, which also keeps `git commit` from running with nothing staged
    // when the claim is merely being resumed.
    let current = read_status_from_fm(&sf.fm_lines);
    if STARTED_STATUSES.contains(&current) {
        return Ok(());
    }

    let date = local_date_string();
    let (_new_fm, new_content) = flip_status_in_fm(&sf.fm_lines, sf.rest, "in_progress", &date);
    std::fs::write(&wf_path, &new_content)
        .map_err(|e| format!("cannot write {}: {e}", wf_path.display()))?;
    git::add_file(task_file, wt_path)?;
    git::commit_in(&format!("plan: claim {slug} (in_progress)"), wt_path)
}

/// The status a branch records for a task, read from the branch's own blob.
///
/// `None` when the branch has no such file or it has no frontmatter -- both
/// mean "the branch says nothing", which is not the same as a status and must
/// not be treated as one.
fn status_on_branch(branch: &str, task_file: &str) -> Option<String> {
    let blob = git::show_ref(branch, task_file).ok()?;
    let sf = split_frontmatter(&blob)?;
    Some(read_status_from_fm(&sf.fm_lines).to_string())
}

/// Read the `depends_on` and status from a trunk blob.
struct DepCheck {
    deps: Vec<String>,
    status: String,
}

fn read_task_on_ref(ref_: &str, task_file: &str) -> Result<DepCheck, String> {
    let blob = git::show_ref(ref_, task_file)?;
    let sf = split_frontmatter(&blob).ok_or_else(|| format!("no frontmatter in {task_file}"))?;
    Ok(DepCheck {
        deps: read_deps_from_fm(&sf.fm_lines),
        status: read_status_from_fm(&sf.fm_lines).to_string(),
    })
}

/// Find a dependency across {epics, stories, tasks} on a git ref.
fn find_dep_on_ref(dep_slug: &str, ref_: &str, plan_dir: &str) -> Result<Option<String>, String> {
    for kd in &["epics", "stories", "tasks"] {
        let dir = format!("{plan_dir}/{kd}");
        let files = git::ls_tree_md(ref_, &dir).unwrap_or_default();
        if let Some(f) = find_task_file(&files, dep_slug) {
            return Ok(Some(f.to_string()));
        }
    }
    Ok(None)
}

/// Compute the local date (YYYY-MM-DD) -- uses local timezone,
/// matching TS `new Date()` which returns local time.
fn local_date_string() -> String {
    // We compute from system time using UTC + timezone offset heuristic.
    // For simplicity and correctness, use jiff's ZonedDateTime.
    let now = jiff::Zoned::now();
    format!("{:04}-{:02}-{:02}", now.year(), now.month(), now.day(),)
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Claim a task: validate deps, create worktree, flip status, commit.
///
/// `worktree_path` encodes both whether to create a worktree and where:
///   - `None`        -> skip worktree entirely (only `--no-worktree`)
///   - `Some("")`    -> create worktree at default path
///   - `Some(path)`  -> create worktree at the given path
///
/// `None` is reserved for the explicit `--no-worktree` opt-out: callers that
/// manage their own workspace also take on the branch, status flip, and
/// commit. Do not map an omitted `--worktree` onto it -- that turns a bare
/// `claim` into a no-op that still reports success.
///
/// Returns the worktree path (one line), or `claimed: <slug>` when no
/// worktree is created.
pub fn claim_task(
    slug: &str,
    trunk: &str,
    plan_dir: &str,
    worktree_path: Option<String>,
    cwd: &Path,
) -> Result<String, String> {
    let branch = format!("plan/{slug}");

    // ---- 1. Informational lint on trunk ----
    // Run lint on trunk using ref mode, echo to stderr (never fails).
    // This matches the TS behavior of spawning lint.cjs before the lock.
    let trunk_report = crate::lint::lint_ref(trunk, plan_dir);
    let trunk_findings = crate::lint::render_report(&trunk_report);
    if !trunk_findings.is_empty() {
        eprint!("{trunk_findings}");
    }

    // ---- 2. Under shared lock ----
    let _lock = PlanrLock::shared(cwd).map_err(|e| format!("lock error: {e}"))?;

    // 2a. Find task file on trunk
    let task_files = git::ls_tree_md(trunk, &format!("{plan_dir}/tasks"))
        .map_err(|_| format!("no task file for slug '{slug}' on {trunk}"))?;
    let task_file = find_task_file(&task_files, slug)
        .ok_or_else(|| format!("no task file for slug '{slug}' on {trunk}"))?
        .to_string();

    // 2b. Trunk must record work that has not started. Anything else means
    // the claim would do nothing: the flip below refuses to move a status
    // that is already at or past in_progress, so the call would create a
    // worktree, skip the flip, and exit 0 -- the silent success this PR
    // exists to remove. The branch-side guard cannot cover this, because
    // `close` deletes the branch, so a task closed and then claimed again has
    // no branch left to check.
    let info = read_task_on_ref(trunk, &task_file)?;
    if info.status == "abandoned" {
        return Err(format!(
            "refuse claim: task '{slug}' is abandoned; update the ticket or create a new one"
        ));
    }
    if STARTED_STATUSES.contains(&info.status.as_str()) {
        return Err(format!(
            "refuse claim: trunk records task '{slug}' as {}; \
             reopen the ticket or create a new one",
            info.status
        ));
    }

    // 2c. Dependency gate. Only `done` satisfies a dependency; an abandoned
    // dependency intentionally remains a blocker until the relationship is
    // changed or the dependent task is abandoned too.
    let mut blockers: Vec<String> = Vec::new();
    for dep in &info.deps {
        let dep_file = find_dep_on_ref(dep, trunk, plan_dir)?;
        let dep_status = match dep_file {
            Some(ref f) => {
                let blob = git::show_ref(trunk, f)?;
                let sf = split_frontmatter(&blob)
                    .ok_or_else(|| format!("no frontmatter in dep '{dep}'"))?;
                read_status_from_fm(&sf.fm_lines).to_string()
            }
            None => String::new(),
        };
        if dep_status != "done" {
            blockers.push(format!("{dep}({dep_status})"));
        }
    }
    if !blockers.is_empty() {
        let err = format!(
            "refuse claim: '{slug}' has unfinished depends_on: {}\n\
             resolve or complete these first, or have the leader update depends_on.",
            blockers.join(" ")
        );
        return Err(err);
    }

    // ---- 3. Holder check ----
    // A branch already checked out somewhere means the task is held: say so
    // in planr's terms rather than letting git's "'<path>' already exists"
    // reach an agent that has no idea what it means.
    //
    // This runs above the `--no-worktree` opt-out, not below it. Whether the
    // caller wants planr to build a worktree says nothing about whether
    // someone else already holds the task, and `claim` is the concurrency
    // primitive of the whole workflow -- with the check below the opt-out, a
    // second agent could `claim --no-worktree` a task another agent was
    // actively working and be told it succeeded.
    if let Some(held) = git::find_worktree_for_branch(&branch) {
        if held.exists() {
            return Err(format!(
                "refuse claim: task '{slug}' is already claimed; its worktree is at {}",
                held.display()
            ));
        }
        // The record outlived the directory -- someone deleted the worktree
        // with `rm -rf` rather than `git worktree remove`, and git keeps
        // listing it until it is pruned. Refusing on that would name a path
        // that is not there and lock the task out for good, so drop the
        // stale record and let the claim resume.
        git::worktree_prune(cwd)?;
    }

    // A terminal branch is not something to resume. Step 2b refuses an
    // abandoned task by reading trunk, but the branch can be ahead of trunk:
    // a worker may have set done or abandoned there and had the worktree
    // removed before close ran. Resuming would hand an agent a finished or
    // dead ticket to work on.
    //
    // Read it out of the branch rather than a checkout, so the refusal lands
    // before anything is created. Checking after `worktree_add` would leave a
    // worktree behind for a claim that failed, and the next attempt would
    // then report "already claimed" -- masking this diagnostic with a false
    // one and putting the branch back on the board as in-flight.
    if git::branch_exists(&branch) {
        if let Some(status) = status_on_branch(&branch, &task_file) {
            if status == "done" || status == "abandoned" {
                return Err(format!(
                    "refuse claim: branch {branch} already reports task '{slug}' as {status}; \
                     close it or create a new ticket"
                ));
            }
        }
    }

    // ---- 4. Worktree or no-worktree path ----
    let Some(worktree_path) = worktree_path else {
        // None means skip worktree creation, status flip, and commit.
        // The caller handles its own worktree and will flip the status
        // independently. The holder check above still applied.
        return Ok(format!("claimed: {slug}"));
    };

    // The path to create, and the path to hide from git. For the default
    // location they differ: planr owns `<plan-dir>/worktrees/` and reuses it
    // for every claim, so one rule on the parent covers every task ever
    // claimed there. An explicit path is the caller's choice of location, but
    // landing inside the repo corrupts trunk the same way, so it is hidden
    // itself.
    let (wt_path, ignore_target) = if worktree_path.is_empty() {
        let mut p = cwd.to_path_buf();
        p.push(plan_dir);
        p.push("worktrees");
        let parent = p.clone();
        p.push(format!("wt-{slug}"));
        (p, parent)
    } else {
        let p = PathBuf::from(&worktree_path);
        let p = if p.is_absolute() { p } else { cwd.join(&p) };
        (p.clone(), p)
    };

    // 3a. Create the worktree, then hide it. Order matters: a rule written
    // for a worktree that was never created is never cleaned up, and an
    // exclude rule is invisible in `git status`. Writing it first means a
    // failed claim -- a typo'd path, a directory that already exists -- can
    // permanently hide a real directory from git.
    let wt_display = wt_path.display().to_string();
    let branch_existed = git::branch_exists(&branch);
    git::worktree_add(&wt_path, &branch, Some(trunk))?;

    // Everything from here on can fail after the worktree exists, so every
    // one of those failures unwinds it. A claim that reports an error must
    // leave nothing behind: a worktree left by a failed claim is unhidden
    // (so it dirties trunk as a gitlink), and it makes the *next* attempt
    // report "already claimed", masking the real reason for good.
    if let Err(e) = hide_and_flip(&wt_path, &ignore_target, &task_file, slug, cwd) {
        let removed = git::worktree_remove(&wt_path, true);
        // A branch this call created goes too -- otherwise `planr board`
        // lists an in-flight branch for a task nobody successfully claimed.
        if removed.is_ok() && !branch_existed {
            let _ = git::branch_delete(&branch, true, cwd);
        }
        // Never report only the original error when the rollback also failed:
        // the worktree is then still there and still unhidden, which is the
        // state the rollback exists to prevent.
        return Err(match removed {
            Ok(()) => e,
            Err(cleanup) => format!(
                "{e}\nand the worktree at {wt_display} could not be removed ({cleanup}); \
                 it is not hidden from git -- remove it before committing on trunk"
            ),
        });
    }

    // 3d. Output worktree path
    Ok(wt_display.to_string())
}

// Lock is dropped here (RAII) -- the shared lock covers the entire
// read-verify-create-flip-commit critical section.

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ---- frontmatter helpers ----

    #[test]
    fn test_split_frontmatter_simple() {
        let blob = "---\nid: x\nstatus: todo\n---\nbody";
        let sf = split_frontmatter(blob).unwrap();
        assert_eq!(sf.fm_lines[0], "id: x");
        assert_eq!(sf.fm_lines[1], "status: todo");
        assert_eq!(sf.rest, "body");
    }

    #[test]
    fn test_split_frontmatter_no_fm() {
        assert!(split_frontmatter("no frontmatter").is_none());
    }

    #[test]
    fn test_read_status_from_fm() {
        let fm = ["id: x", "status: todo", "parent: p"];
        assert_eq!(read_status_from_fm(&fm), "todo");
    }

    #[test]
    fn test_read_status_from_fm_missing() {
        let fm = ["id: x", "parent: p"];
        assert_eq!(read_status_from_fm(&fm), "");
    }

    #[test]
    fn test_read_deps_inline_list() {
        let fm = ["id: x", "depends_on: [dep1, dep2]", "status: todo"];
        let deps = read_deps_from_fm(&fm);
        assert_eq!(deps, vec!["dep1", "dep2"]);
    }

    #[test]
    fn test_read_deps_bare_string() {
        let fm = ["id: x", "depends_on: dep1", "status: todo"];
        let deps = read_deps_from_fm(&fm);
        assert_eq!(deps, vec!["dep1"]);
    }

    #[test]
    fn test_read_deps_block_list() {
        let fm = [
            "id: x",
            "depends_on:",
            "  - dep1",
            "  - dep2",
            "status: todo",
        ];
        let deps = read_deps_from_fm(&fm);
        assert_eq!(deps, vec!["dep1", "dep2"]);
    }

    #[test]
    fn test_read_deps_empty() {
        let fm = ["id: x", "depends_on: []", "status: todo"];
        let deps = read_deps_from_fm(&fm);
        assert!(deps.is_empty());
    }

    #[test]
    fn test_flip_status_in_fm_replaces() {
        let fm = ["id: x", "status: todo", "updated: 2026-01-01"];
        let rest = "body\n";
        let (_new_fm, full) = flip_status_in_fm(&fm, rest, "in_progress", "2026-08-05");
        assert!(full.contains("status: in_progress"));
        assert!(full.contains("updated: 2026-08-05"));
        assert!(full.contains("id: x"));
        assert!(full.ends_with("body\n"));
    }

    #[test]
    fn test_flip_status_in_fm_inserts_if_absent() {
        let fm = ["id: x"];
        let rest = "body\n";
        let (_new_fm, full) = flip_status_in_fm(&fm, rest, "in_progress", "2026-08-05");
        // Should have both; extract frontmatter block
        let sep = full.find("\n---\n").unwrap();
        let fm_part = &full[4..sep];
        let lines: Vec<&str> = fm_part.split('\n').collect();
        let status_pos = lines.iter().position(|l| l.starts_with("status:"));
        let updated_pos = lines.iter().position(|l| l.starts_with("updated:"));
        assert!(status_pos.is_some(), "status missing: {full}");
        assert!(updated_pos.is_some(), "updated missing: {full}");
        // TS order: updated is unshifted first (before status insertion),
        // so updated ends up ABOVE status (lower array index).
        assert!(
            updated_pos.unwrap() < status_pos.unwrap(),
            "updated (pos {}) should be above status (pos {}): {full}",
            updated_pos.unwrap(),
            status_pos.unwrap(),
        );
    }

    // ---- findTask ----

    #[test]
    fn test_find_task_file_exact() {
        let files = vec![
            "tasks/01-scaffold.md".to_string(),
            "tasks/02-parse.md".to_string(),
            "tasks/03-http-proxy.md".to_string(),
        ];
        assert_eq!(
            find_task_file(&files, "http-proxy"),
            Some("tasks/03-http-proxy.md")
        );
    }

    #[test]
    fn test_find_task_file_not_found() {
        let files = vec!["tasks/01-scaffold.md".to_string()];
        assert!(find_task_file(&files, "nonexistent").is_none());
    }

    #[test]
    fn test_find_task_file_ends_with_semantics() {
        // TS behavior: f.endsWith(slug + ".md") -- loose because it doesn't
        // enforce NN- prefix before the slug. "01-http-proxy.md" matches
        // slug "http-proxy" (last 14 chars = "http-proxy.md").
        let files = vec!["tasks/01-http-proxy.md".to_string()];
        assert_eq!(
            find_task_file(&files, "http-proxy"),
            Some("tasks/01-http-proxy.md")
        );
        // Also matches "02-http-proxy.md" if it existed (first match wins)
    }

    // ---- local_date_string ----

    #[test]
    fn test_local_date_string_format() {
        let s = local_date_string();
        assert_eq!(s.len(), 10, "bad format: {s}");
        assert_eq!(&s[4..5], "-");
        assert_eq!(&s[7..8], "-");
        let y: u32 = s[..4].parse().unwrap();
        let m: u32 = s[5..7].parse().unwrap();
        let d: u32 = s[8..].parse().unwrap();
        assert!((2025..=2030).contains(&y));
        assert!((1..=12).contains(&m));
        assert!((1..=31).contains(&d));
    }
}
