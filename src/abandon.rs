//! `planr abandon` -- close a ticket without review for an explicit reason.
//!
//! Abandonment is deliberately separate from `close`: it is a trunk-local
//! operation for work that is overtaken by events (OBE) or will not be done.
//! It records an `abandoned` status and never merges or removes an active task
//! branch.

use crate::close_cmd::find_ticket_by_slug;
use crate::git;
use crate::lock::PlanrLock;
use crate::ticket::parse_ticket;
use std::path::Path;

const VALID_REASONS: [&str; 2] = ["obe", "wont-do"];

/// Validate the ticket kind and return its plan subdirectory.
fn kind_dir(kind: &str) -> Result<&'static str, String> {
    match kind {
        "task" => Ok("tasks"),
        "story" => Ok("stories"),
        "epic" => Ok("epics"),
        _ => Err(format!("unknown kind: {kind} (want task|story|epic)")),
    }
}

/// Validate the explicit reason recorded on an abandoned ticket.
fn validate_reason(reason: &str) -> Result<(), String> {
    if VALID_REASONS.contains(&reason) {
        Ok(())
    } else {
        Err(format!(
            "invalid abandon reason '{reason}' (want obe|wont-do)"
        ))
    }
}

/// Replace status, updated, and reason in the first frontmatter block.
///
/// Existing fields retain their position. Missing fields are appended so the
/// rest of the ticket remains byte-for-byte unchanged. This intentionally
/// stores the reason in frontmatter: it is easy for board/lint tooling and
/// other consumers to query without having to parse a prose section.
fn abandon_frontmatter(content: &str, reason: &str, date: &str) -> Result<String, String> {
    let sf = split_fm(content).ok_or_else(|| "no frontmatter".to_string())?;
    let mut has_status = false;
    let mut has_updated = false;
    let mut has_reason = false;
    let mut out: Vec<String> = Vec::with_capacity(sf.fm_lines.len() + 3);

    for line in &sf.fm_lines {
        if line.starts_with("status:") {
            out.push("status: abandoned".to_string());
            has_status = true;
        } else if line.starts_with("updated:") {
            out.push(format!("updated: {date}"));
            has_updated = true;
        } else if line.starts_with("reason:") {
            out.push(format!("reason: {reason}"));
            has_reason = true;
        } else {
            out.push(line.to_string());
        }
    }

    if !has_status {
        out.push("status: abandoned".to_string());
    }
    if !has_updated {
        out.push(format!("updated: {date}"));
    }
    if !has_reason {
        out.push(format!("reason: {reason}"));
    }

    Ok(format!("---\n{}\n---\n{}", out.join("\n"), sf.rest))
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

/// Return the local date in YYYY-MM-DD form.
fn local_date_string() -> String {
    let now = jiff::Zoned::now();
    format!("{:04}-{:02}-{:02}", now.year(), now.month(), now.day())
}

/// Abandon a ticket on trunk without a review or merge.
///
/// The exclusive plan lock covers the branch check through the commit. This
/// serializes against `claim`, which holds the shared lock while creating a
/// task branch, so an active branch cannot appear between the check and write.
pub fn abandon_ticket(
    kind: &str,
    slug: &str,
    reason: &str,
    trunk: &str,
    plan_dir: &str,
    cwd: &Path,
) -> Result<String, String> {
    let kind_dir = kind_dir(kind)?;
    validate_reason(reason)?;

    let _lock = PlanrLock::exclusive(cwd).map_err(|e| format!("lock error: {e}"))?;

    let branch = format!("plan/{slug}");
    if git::rev_parse_verify(&branch).is_ok() {
        return Err(format!(
            "refuse abandon: ticket '{slug}' has active branch {branch};\n\
             clean up the branch/worktree first; planr abandon never discards or merges it"
        ));
    }

    let file = find_ticket_by_slug(slug, kind_dir, trunk, plan_dir)?;
    let blob = git::show_ref(trunk, &file)?;
    let ticket = parse_ticket(&blob);
    if ticket.status == "abandoned" {
        return Err(format!(
            "refuse abandon: {kind} '{slug}' is already abandoned (reason is recorded)"
        ));
    }

    // Checkout trunk before writing so the commit lands on the authoritative
    // backlog, even when the caller invoked planr from another branch.
    git::checkout(trunk)?;

    let date = local_date_string();
    let new_content = abandon_frontmatter(&blob, reason, &date)?;
    let fpath = Path::new(&file);
    let parent = fpath.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent)
        .map_err(|e| format!("cannot create dir {}: {e}", parent.display()))?;
    std::fs::write(fpath, &new_content).map_err(|e| format!("cannot write {file}: {e}"))?;

    git::add_file(&file, Path::new("."))?;
    git::commit_in(
        &format!("plan: abandon {kind} {slug} ({reason})"),
        Path::new("."),
    )?;

    Ok(format!("abandoned {kind} {slug}; reason: {reason}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kind_dir_accepts_all_ticket_kinds() {
        assert_eq!(kind_dir("task"), Ok("tasks"));
        assert_eq!(kind_dir("story"), Ok("stories"));
        assert_eq!(kind_dir("epic"), Ok("epics"));
    }

    #[test]
    fn test_kind_dir_rejects_unknown_kind() {
        let err = kind_dir("milestone").unwrap_err();
        assert!(err.contains("unknown kind"));
    }

    #[test]
    fn test_validate_reason() {
        assert!(validate_reason("obe").is_ok());
        assert!(validate_reason("wont-do").is_ok());
        assert!(validate_reason("later").is_err());
    }

    #[test]
    fn test_abandon_frontmatter_replaces_and_preserves_body() {
        let content = "---\nid: x\nstatus: todo\nupdated: 2026-01-01\n---\n\n## Goal\nBody\n";
        let result = abandon_frontmatter(content, "obe", "2026-08-11").unwrap();
        assert!(result.contains("status: abandoned"));
        assert!(result.contains("updated: 2026-08-11"));
        assert!(result.contains("reason: obe"));
        assert!(result.ends_with("\n## Goal\nBody\n"));
    }

    #[test]
    fn test_abandon_frontmatter_inserts_missing_fields() {
        let content = "---\nid: x\n---\nbody\n";
        let result = abandon_frontmatter(content, "wont-do", "2026-08-11").unwrap();
        assert!(result.contains("status: abandoned"));
        assert!(result.contains("updated: 2026-08-11"));
        assert!(result.contains("reason: wont-do"));
        assert!(result.ends_with("body\n"));
    }

    #[test]
    fn test_abandon_frontmatter_replaces_existing_reason() {
        let content = "---\nid: x\nstatus: todo\nreason: old\n---\nbody";
        let result = abandon_frontmatter(content, "obe", "2026-08-11").unwrap();
        assert!(result.contains("reason: obe"));
        assert!(!result.contains("reason: old"));
    }
}
