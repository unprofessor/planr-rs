//! `planr abandon` -- close a ticket without review, recording a free-text
//! reason as a prose section rather than a constrained frontmatter field.
//!
//! Abandonment is deliberately separate from `close`: it is a trunk-local
//! operation for work that is overtaken by events (OBE) or will not be done.
//! It records an `abandoned` status and never merges or removes an active task
//! branch. The user-supplied message is appended as `## Reason Abandoned` below
//! the existing body.

use crate::close_cmd::{find_ticket_by_slug, write_and_commit_on_trunk};
use crate::exclude;
use crate::frontmatter::{local_date_string, split_fm};
use crate::git;
use crate::lock::PlanrLock;
use crate::ticket::parse_ticket;
use std::path::Path;

/// Validate the ticket kind and return its plan subdirectory.
fn kind_dir(kind: &str) -> Result<&'static str, String> {
    match kind {
        "task" => Ok("tasks"),
        "story" => Ok("stories"),
        "epic" => Ok("epics"),
        _ => Err(format!("unknown kind: {kind} (want task|story|epic)")),
    }
}

/// Replace status and updated in the first frontmatter block, then append a
/// `## Reason Abandoned` section with the user-supplied message.
///
/// Existing fields retain their position. Missing fields are appended so the
/// rest of the ticket remains byte-for-byte unchanged. The message lives in
/// the body (not frontmatter) so the user can write arbitrary prose -- it is
/// not constrained to a fixed vocabulary.
fn abandon_frontmatter(content: &str, message: &str, date: &str) -> Result<String, String> {
    let sf = split_fm(content).ok_or_else(|| "no frontmatter".to_string())?;
    let mut has_status = false;
    let mut has_updated = false;
    let mut out: Vec<String> = Vec::with_capacity(sf.fm_lines.len() + 2);

    for line in &sf.fm_lines {
        if line.starts_with("status:") {
            out.push("status: abandoned".to_string());
            has_status = true;
        } else if line.starts_with("updated:") {
            out.push(format!("updated: {date}"));
            has_updated = true;
        } else if line.starts_with("reason:") {
            // Drop legacy reason field -- we now use a prose section.
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

    let mut body = sf.rest.to_string();

    let msg = message.trim();
    if !msg.is_empty() {
        // Strip any trailing blank lines so the new section sits cleanly.
        let trimmed_body = body.trim_end();
        body = format!("{trimmed_body}\n\n## Reason Abandoned\n\n{msg}\n");
    }

    Ok(format!("---\n{}\n---\n{}", out.join("\n"), body))
}

/// Abandon a ticket on trunk without a review or merge.
///
/// The exclusive plan lock covers the branch check through the commit. This
/// serializes against `claim`, which holds the shared lock while creating a
/// task branch, so an active branch cannot appear between the check and write.
pub fn abandon_ticket(
    kind: &str,
    slug: &str,
    message: &str,
    trunk: &str,
    plan_dir: &str,
    cwd: &Path,
) -> Result<String, String> {
    let kind_dir = kind_dir(kind)?;

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
            "refuse abandon: {kind} '{slug}' is already abandoned"
        ));
    }

    // Resolve the working directory that has trunk checked out (possibly a
    // different worktree) so the commit lands on the authoritative backlog,
    // even when planr was invoked from a task worktree on another branch.
    let trunk_dir = git::trunk_worktree(trunk, cwd)?;

    let date = local_date_string();
    let new_content = abandon_frontmatter(&blob, message, &date)?;
    write_and_commit_on_trunk(
        &trunk_dir,
        &file,
        &new_content,
        &format!("plan: abandon {kind} {slug}"),
    )?;

    // Abandon refuses until the branch and worktree are cleaned up by hand, so
    // it never learns which path the worktree had and cannot remove that one
    // rule by name. Nothing else would either -- `close` never runs for an
    // abandoned task -- so the rule would outlive everything that referred to
    // it and go on hiding whatever is created at that path. Prune whatever no
    // live worktree still justifies.
    exclude::exclude_prune(cwd);

    Ok(format!("abandoned {kind} {slug}"))
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
    fn test_abandon_frontmatter_appends_reason_section() {
        let content = "---\nid: x\nstatus: todo\nupdated: 2026-01-01\n---\n\n## Goal\nBody\n";
        let result =
            abandon_frontmatter(content, "OBE — no longer relevant", "2026-08-11").unwrap();
        assert!(result.contains("status: abandoned"));
        assert!(result.contains("updated: 2026-08-11"));
        assert!(result.contains("## Reason Abandoned"));
        assert!(result.contains("OBE — no longer relevant"));
        // Frontmatter should NOT contain a reason field
        assert!(!result.contains("\nreason:"));
        // The original body is preserved before the new section
        assert!(result.contains("## Goal\nBody"));
    }

    #[test]
    fn test_abandon_frontmatter_inserts_missing_fields() {
        let content = "---\nid: x\n---\nbody\n";
        let result = abandon_frontmatter(content, "wont-do", "2026-08-11").unwrap();
        assert!(result.contains("status: abandoned"));
        assert!(result.contains("updated: 2026-08-11"));
        assert!(result.contains("## Reason Abandoned\n\nwont-do\n"));
        assert!(!result.contains("\nreason:"));
    }

    #[test]
    fn test_abandon_frontmatter_skips_section_when_empty_message() {
        let content = "---\nid: x\n---\nbody\n";
        let result = abandon_frontmatter(content, "", "2026-08-11").unwrap();
        assert!(!result.contains("## Reason Abandoned"));
        assert!(!result.contains("\nreason:"));
    }

    #[test]
    fn test_abandon_frontmatter_removes_old_reason_field() {
        // Old tickets may have a reason: field in frontmatter; it should be
        // dropped (not carried forward) since we no longer use it.
        let content = "---\nid: x\nstatus: todo\nreason: old\n---\nbody";
        let result = abandon_frontmatter(content, "new message", "2026-08-11").unwrap();
        assert!(!result.contains("reason:"));
        assert!(result.contains("## Reason Abandoned\n\nnew message\n"));
    }

    #[test]
    fn test_abandon_frontmatter_multi_line_message() {
        let content = "---\nid: x\nstatus: todo\n---\nbody";
        let msg = "First paragraph.\n\nSecond paragraph with details.";
        let result = abandon_frontmatter(content, msg, "2026-08-11").unwrap();
        assert!(result.contains("## Reason Abandoned"));
        assert!(result.contains("First paragraph."));
        assert!(result.contains("Second paragraph with details."));
    }
}
