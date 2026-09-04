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

/// The only status a claim may act on.
///
/// Stated as "what may be claimed" rather than a list of what may not: an
/// enumeration of the excluded statuses has to be kept in step with
/// [`crate::ticket::VALID_STATUSES`], and the first version of it already
/// fell a status short -- `blocked` was missing, so a worker who set it on
/// their branch had it silently rewritten to `in_progress` on the next
/// claim. A claim flips `todo` to `in_progress` and leaves every other
/// status alone; adding a status to the vocabulary cannot break that.
const CLAIMABLE_STATUS: &str = "todo";

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
///
/// Quotes are stripped, because every other command reads frontmatter through
/// serde_yaml and so sees `status: "done"` as `done`. A reader here that kept
/// the quote characters disagreed with `lint` and `board` about what the same
/// file said, and -- since the terminal-status guards compare against bare
/// words -- let a quoted `"done"` past them to be reopened and committed.
fn read_status_from_fm<'a>(fm: &'a [&str]) -> &'a str {
    for l in fm {
        if let Some(val) = l.strip_prefix("status:") {
            let val = val.trim();
            return val
                .strip_prefix('"')
                .and_then(|v| v.strip_suffix('"'))
                .or_else(|| val.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
                .unwrap_or(val);
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

/// What a partially-completed claim left behind, for the rollback to undo.
#[derive(Default)]
struct Placed {
    /// The exclude rule covering the worktree is in place.
    hidden: bool,
    /// *This* call wrote that rule, so this call may remove it.
    rule_added: bool,
}

/// Hide the new worktree and flip the task to `in_progress` on its branch.
///
/// Split out so the caller has a single fallible unit to unwind: every step
/// here runs after the worktree exists, and a failure in any of them must
/// leave nothing behind. It reports what it managed to place rather than
/// unwinding itself, because the rule can only be judged once the worktree
/// is gone -- see `rollback_claim`.
fn hide_and_flip(
    wt_path: &Path,
    ignore_target: &Path,
    task_file: &str,
    slug: &str,
    cwd: &Path,
    placed: &mut Placed,
) -> Result<(), String> {
    placed.rule_added = git::exclude_add(ignore_target, cwd)?;
    placed.hidden = true;
    flip_to_in_progress(wt_path, task_file, slug)
}

/// Undo a claim that failed after its worktree was created, and describe
/// anything that could not be undone.
fn rollback_claim(
    err: String,
    wt_path: &Path,
    ignore_target: &Path,
    placed: &Placed,
    branch: &str,
    branch_existed: bool,
    cwd: &Path,
) -> String {
    // It was just created and holds nothing, so forcing is safe.
    if let Err(cleanup) = git::worktree_remove(wt_path, true) {
        // The worktree is still there, so its rule stays with it: removing the
        // rule now would leave a live worktree visible, which is worse than
        // the claim having failed. Say which state the operator is in --
        // claiming it is unhidden when the rule still covers it would send
        // them deleting a directory on a false premise.
        let exposure = if placed.hidden {
            "it is still hidden from git, so trunk is clean; remove it by hand"
        } else {
            "it is not hidden from git -- remove it before committing on trunk"
        };
        return format!(
            "{err}\nand the worktree at {} could not be removed ({cleanup}); {exposure}",
            wt_path.display()
        );
    }

    // A branch this call created goes too -- otherwise `planr board` lists an
    // in-flight branch for a task nobody successfully claimed.
    if !branch_existed {
        let _ = git::branch_delete(branch, true, cwd);
    }

    // The rule comes out last, and only if this call wrote it. Order matters:
    // `exclude_remove` keeps a rule that any live worktree still needs, and
    // ours was one of those until the line above. Removing it first would ask
    // that question while our own worktree still counted as a dependant.
    if placed.rule_added {
        git::drop_exclude(ignore_target, cwd);
    }
    err
}

/// Move the task on the branch to `in_progress` and commit it.
fn flip_to_in_progress(wt_path: &Path, task_file: &str, slug: &str) -> Result<(), String> {
    let wf_path = wt_path.join(task_file);
    let content = std::fs::read_to_string(&wf_path)
        .map_err(|e| format!("cannot read {}: {e}", wf_path.display()))?;
    let sf = split_frontmatter(&content).ok_or_else(|| format!("no frontmatter in {task_file}"))?;

    // Flip the status, but never over one that is already set. A resumed
    // claim finds the branch at in_progress, or further along -- a worker can
    // reach review, or mark the task blocked, before the worktree is removed.
    // Rewriting any of those to in_progress discards that work and then makes
    // `close` refuse the task for having the wrong status. Leaving them alone
    // also keeps `git commit` from running with nothing staged when the claim
    // is merely being resumed.
    if read_status_from_fm(&sf.fm_lines) != CLAIMABLE_STATUS {
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
    if info.status != CLAIMABLE_STATUS {
        let shown = if info.status.is_empty() {
            "<missing>"
        } else {
            &info.status
        };
        // Name a remedy planr actually offers. No command sets a ticket back
        // to `todo` -- `close` writes done, `abandon` writes abandoned, `new`
        // only scaffolds -- so for a blocked or hand-edited ticket the fix is
        // to edit the frontmatter on trunk, and saying "reopen the ticket"
        // pointed at an operation that does not exist.
        return Err(format!(
            "refuse claim: trunk records task '{slug}' as {shown}; \
             only a {CLAIMABLE_STATUS} task can be claimed -- \
             set `status: {CLAIMABLE_STATUS}` on trunk and commit, \
             or create a new ticket"
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
    // Serialize claims of *this* task, so the check below and the
    // `worktree_add` it guards cannot be interleaved by a second claim of the
    // same slug -- both would see no holder and the loser would get git's raw
    // error, which is the message this check exists to replace. The lock is
    // per slug: `planr.lock` is held shared here because claims of different
    // tasks running at once are the point of the workflow.
    let _claim_lock = PlanrLock::claim_slug(cwd, slug).map_err(|e| format!("lock error: {e}"))?;

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
        // `try_exists`, not `exists`: the latter reports `false` for any I/O
        // error, so a permission denied on an ancestor read as "the worktree
        // is gone" and destroyed a live holder's record. Not knowing counts as
        // held, the same fail-closed choice `another_worktree_needs` makes.
        //
        // It is not a guarantee, and the limit is worth naming: a path under
        // an unmounted mountpoint answers `ENOENT`, which is `Ok(false)`, not
        // an error. A worktree on a removable or network volume that happens
        // to be unmounted is therefore indistinguishable from one deleted by
        // hand -- git's own `prunable` flag makes the same call. That is why
        // dropping a record warns below rather than doing it silently.
        if held.try_exists().unwrap_or(true) {
            return Err(format!(
                "refuse claim: task '{slug}' is already claimed; its worktree is at {}",
                held.display()
            ));
        }
        // The record outlived the directory -- someone deleted the worktree
        // with `rm -rf` rather than `git worktree remove`, and git keeps
        // listing it until the record is dropped. Refusing on that would name
        // a path that is not there and lock the task out for good.
        //
        // Drop that one record, not every unreachable one. `git worktree
        // prune` is repo-global, so claiming this task would also forget any
        // worktree that merely happens to be unreachable right now -- an
        // unmounted volume, a network path, a home directory not yet
        // decrypted -- orphaning it as a side effect of an unrelated claim.
        // Say so. The directory being absent cannot be told apart from a
        // volume that is merely unmounted, so this is the operator's one
        // chance to notice that a worktree they still care about was
        // forgotten -- and it is otherwise invisible.
        eprintln!(
            "warning: {branch} had a registered worktree at {} whose directory is \
             not there; dropping that record and re-claiming. If that path lives \
             on a volume that is currently unmounted, mount it and run \
             `git worktree repair` before working there again.",
            held.display()
        );
        git::worktree_remove(&held, true)?;
        // The rule written for that worktree goes with it. This claim may
        // land somewhere else entirely (the previous one could have used an
        // explicit `--worktree`), and no later `close` would remove it --
        // `close` only ever considers the path the task is holding now. Left
        // behind, it silently hides whatever is created at the old path.
        git::drop_exclude(&held, cwd);
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
    if let Err(e) = git::worktree_add(&wt_path, &branch, Some(trunk)) {
        // `git worktree add -b <branch> <path>` creates the branch *before* it
        // validates the path, so a refused path (already occupied, typo'd)
        // leaves the branch behind. `planr board` would then list an in-flight
        // branch for a task nobody claimed and mark its status from it, and
        // `planr abandon` would refuse the task for having an active branch,
        // until someone deleted it by hand.
        if !branch_existed {
            let _ = git::branch_delete(&branch, true, cwd);
        }
        return Err(e);
    }

    // Everything from here on can fail after the worktree exists, so every
    // one of those failures unwinds it. A claim that reports an error must
    // leave nothing behind: a worktree left by a failed claim is unhidden
    // (so it dirties trunk as a gitlink), and it makes the *next* attempt
    // report "already claimed", masking the real reason for good.
    let mut placed = Placed::default();
    if let Err(e) = hide_and_flip(&wt_path, &ignore_target, &task_file, slug, cwd, &mut placed) {
        return Err(rollback_claim(
            e,
            &wt_path,
            &ignore_target,
            &placed,
            &branch,
            branch_existed,
            cwd,
        ));
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
