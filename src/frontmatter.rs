//! The frontmatter write path: split a ticket, rewrite `status:` and
//! `updated:`, put it back together.
//!
//! This is deliberately not [`crate::parse::split_frontmatter`]. That one is
//! the read path: it tolerates `--- ` with trailing whitespace and rejoins
//! with `split('\n')`. This one demands an exact `---\n` fence and preserves
//! the body byte-for-byte. The two disagree on malformed frontmatter, and
//! that disagreement is intended -- a reader may guess, a writer may not.
//! Do not collapse them into one.

/// A ticket split into its frontmatter lines and everything after the closing
/// fence.
pub struct FmSplit<'a> {
    pub fm_lines: Vec<&'a str>,
    pub rest: &'a str,
}

/// Split on `---\n...\n---` -- first block only, no re-entry.
pub fn split_fm(blob: &str) -> Option<FmSplit<'_>> {
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

/// Rewrite `status:` and `updated:` in already-split frontmatter, inserting
/// either if absent, and return the whole ticket.
///
/// TS order: check hasStatus first and unshift, then check hasUpdated -- the
/// second unshift puts `updated` ABOVE `status`. Real tickets carry both
/// lines, so the insert path is nearly dead, but the order is observable and
/// tests pin it.
pub fn flip_lines(fm_lines: &[&str], rest: &str, new_status: &str, date: &str) -> String {
    let mut has_status = false;
    let mut has_updated = false;
    let mut out: Vec<String> = Vec::with_capacity(fm_lines.len() + 2);

    for line in fm_lines {
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
    if !has_status {
        out.insert(0, format!("status: {new_status}"));
    }
    if !has_updated {
        out.insert(0, format!("updated: {date}"));
    }

    format!("---\n{}\n---\n{}", out.join("\n"), rest)
}

/// Split a ticket and flip its status and date in one step.
pub fn flip_frontmatter(content: &str, new_status: &str, date: &str) -> Result<String, String> {
    let sf = split_fm(content).ok_or_else(|| "no frontmatter".to_string())?;
    Ok(flip_lines(&sf.fm_lines, sf.rest, new_status, date))
}

/// The local date in YYYY-MM-DD form.
///
/// Local, not UTC: it matches the TS original's `new Date()`, and a ticket
/// dated a day off from the operator's own calendar reads as wrong.
pub fn local_date_string() -> String {
    let now = jiff::Zoned::now();
    format!("{:04}-{:02}-{:02}", now.year(), now.month(), now.day())
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_flip_frontmatter_no_fm() {
        assert!(flip_frontmatter("no frontmatter", "done", "2026-08-05").is_err());
    }

    #[test]
    fn test_flip_lines_replaces() {
        let fm = ["id: x", "status: todo", "updated: 2026-01-01"];
        let full = flip_lines(&fm, "body\n", "in_progress", "2026-08-05");
        assert!(full.contains("status: in_progress"));
        assert!(full.contains("updated: 2026-08-05"));
        assert!(full.contains("id: x"));
        assert!(full.ends_with("body\n"));
    }

    #[test]
    fn test_flip_lines_inserts_if_absent() {
        let fm = ["id: x"];
        let full = flip_lines(&fm, "body\n", "in_progress", "2026-08-05");
        let sep = full.find("\n---\n").unwrap();
        let fm_part = &full[4..sep];
        let lines: Vec<&str> = fm_part.split('\n').collect();
        let status_pos = lines.iter().position(|l| l.starts_with("status:"));
        let updated_pos = lines.iter().position(|l| l.starts_with("updated:"));
        assert!(status_pos.is_some(), "status missing: {full}");
        assert!(updated_pos.is_some(), "updated missing: {full}");
        // TS order: updated is unshifted second, so it lands ABOVE status.
        assert!(
            updated_pos.unwrap() < status_pos.unwrap(),
            "updated (pos {}) should be above status (pos {}): {full}",
            updated_pos.unwrap(),
            status_pos.unwrap(),
        );
    }

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
