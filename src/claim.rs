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

    // 2b. An abandoned task is terminal and cannot be claimed again.
    let info = read_task_on_ref(trunk, &task_file)?;
    if info.status == "abandoned" {
        return Err(format!(
            "refuse claim: task '{slug}' is abandoned; update the ticket or create a new one"
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

    // ---- 3. Worktree or no-worktree path ----
    let Some(worktree_path) = worktree_path else {
        // None means skip worktree creation, status flip, and commit.
        // The caller handles its own worktree and will flip the status
        // independently.
        return Ok(format!("claimed: {slug}"));
    };

    let wt_path = if worktree_path.is_empty() {
        // --worktree with no value: use default
        let mut p = cwd.to_path_buf();
        p.push(plan_dir);
        p.push("worktrees");
        p.push(format!("wt-{slug}"));
        p
    } else {
        let p = PathBuf::from(&worktree_path);
        if p.is_absolute() {
            p
        } else {
            cwd.join(&p)
        }
    };

    // 3a. Create worktree branch
    let wt_display = wt_path.display();
    git::worktree_add(&wt_path, &branch, Some(trunk))?;

    // 3b. Read the task file in the worktree, flip status, write back
    let wf_path = wt_path.join(&task_file);
    let content = std::fs::read_to_string(&wf_path)
        .map_err(|e| format!("cannot read {}: {e}", wf_path.display()))?;
    let sf = split_frontmatter(&content).ok_or_else(|| format!("no frontmatter in {task_file}"))?;
    let date = local_date_string();
    let (_new_fm, new_content) = flip_status_in_fm(&sf.fm_lines, sf.rest, "in_progress", &date);

    std::fs::write(&wf_path, &new_content)
        .map_err(|e| format!("cannot write {}: {e}", wf_path.display()))?;

    // 3c. git add + git commit in the worktree
    git::add_file(&task_file, &wt_path)?;
    git::commit_in(&format!("plan: claim {slug} (in_progress)"), &wt_path)?;

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
