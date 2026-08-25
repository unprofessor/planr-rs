//! Pure frontmatter/body parsers -- no I/O, no git.
//!
//! These functions are the direct port of `skills/planr/src/parse.ts`.

use regex::Regex;
use serde_yaml::Value;

/// Split a ticket blob into frontmatter (YAML between the first pair of `---`
/// lines) and body (everything after). Reads only the FIRST `---` block; a
/// body `---` thematic break does NOT re-enter frontmatter parsing.
pub fn split_frontmatter(blob: &str) -> FrontmatterSplit {
    // Use split('\n') like the TS source to preserve trailing empty elements.
    // Rust's lines() strips the trailing newline, which loses a trailing
    // empty line that the TS port's consumers may rely on.
    let parts: Vec<&str> = blob.split('\n').collect();

    // Must start with --- on its own line (trimmed trailing whitespace)
    if parts.is_empty() || parts[0].trim_end() != "---" {
        return FrontmatterSplit {
            fm: String::new(),
            body: blob.to_string(),
            raw: blob.to_string(),
        };
    }

    // Find the closing --- (first --- after the opening, on its own line)
    let mut close_idx = None;
    for (i, line) in parts[1..].iter().enumerate() {
        if line.trim_end() == "---" {
            close_idx = Some(i + 1); // +1 because we started from index 1
            break;
        }
    }

    match close_idx {
        None => {
            // Malformed: opening --- but no closing ---; treat all as body
            FrontmatterSplit {
                fm: String::new(),
                body: blob.to_string(),
                raw: blob.to_string(),
            }
        }
        Some(idx) => {
            let fm = parts[1..idx].join("\n");
            let body = parts[idx + 1..].join("\n");
            FrontmatterSplit {
                fm,
                body,
                raw: blob.to_string(),
            }
        }
    }
}

/// Result of [`split_frontmatter`].
pub struct FrontmatterSplit {
    pub fm: String,
    pub body: String,
    #[allow(dead_code)]
    pub raw: String,
}

/// Parse frontmatter YAML into a value map using serde_yaml.
///
/// `Ok(None)` means there was nothing to parse -- empty input, an explicit
/// null, or a document that is not a mapping (matching the TS
/// `parseFrontmatter` which returns `{}` for those cases).
///
/// `Err` carries the serde_yaml message. Callers must distinguish it from
/// `Ok(None)`: a block that fails to parse reads as "every field missing",
/// so reporting it as a parse failure is the difference between one accurate
/// error and a cascade of bogus ones about fields that are actually present.
pub fn parse_frontmatter(fm: &str) -> Result<Option<Value>, String> {
    if fm.trim().is_empty() {
        return Ok(None);
    }
    match serde_yaml::from_str::<Value>(fm) {
        Ok(v @ Value::Mapping(_)) => Ok(Some(v)),
        Ok(_) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

// ---------------------------------------------------------------------------
// Wiki-link extraction
// ---------------------------------------------------------------------------

/// Extract wiki-links from body text. Matches `[[slug]]`, `[[slug|alias]]`,
/// and `[[slug#heading]]` -- stripping alias/heading, skipping fenced code
/// blocks, and deduplicating results.
///
/// Regex breakdown:
///   \[\[           literal [[
///   ([^\]|#]+)    capture group 1: slug (no ], |, or #)
///   (?:#[^\]]*)?  optional #heading (non-capturing)
///   (?:\|[^\]]*)? optional |alias  (non-capturing)
///   \]\]           literal ]]
pub fn extract_wiki_links(body: &str) -> Vec<String> {
    // Remove fenced code blocks before scanning (both backtick and tilde)
    let re_fence = Regex::new(r"```[\s\S]*?```|~~~[\s\S]*?~~~").unwrap();
    let without_fences = re_fence.replace_all(body, "").to_string();

    let link_re = Regex::new(r"\[\[([^\]|#]+)(?:#[^\]]*)?(?:\|[^\]]*)?\]\]").unwrap();

    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();

    for cap in link_re.captures_iter(&without_fences) {
        if let Some(slug_match) = cap.get(1) {
            let slug = slug_match.as_str().to_string();
            if seen.insert(slug.clone()) {
                result.push(slug);
            }
        }
    }

    result
}

// ---------------------------------------------------------------------------
// Section extraction
// ---------------------------------------------------------------------------

/// Extract the content of a named `## Section` from body text. Uses a state
/// machine on lines starting with `## `; returns lines from the heading until
/// the next `## ` heading, excluding the heading line itself.
pub fn extract_section(body: &str, name: &str) -> String {
    let heading = format!("## {name}");
    let lines: Vec<&str> = body.lines().collect();

    let mut in_section = false;
    let mut section_lines: Vec<&str> = Vec::new();

    for line in &lines {
        if line.starts_with("## ") {
            if in_section {
                // Next heading -- stop collecting
                break;
            }
            if *line == heading {
                in_section = true;
            }
        } else if in_section {
            section_lines.push(line);
        }
    }

    // Trim trailing blank lines
    while section_lines.last().is_some_and(|l| l.trim().is_empty()) {
        section_lines.pop();
    }
    // Trim leading blank lines
    while section_lines.first().is_some_and(|l| l.trim().is_empty()) {
        section_lines.remove(0);
    }

    section_lines.join("\n")
}

// ---------------------------------------------------------------------------
// Last review verdict
// ---------------------------------------------------------------------------

/// Extract the verdict from the **last** `## Review` block in the body.
/// Returns the trimmed value after `verdict:` or `None` if no review block.
fn is_heading(line: &str) -> bool {
    line.starts_with("## ")
}

/// Scan lines for `## Review` sections and return the verdict from the last one.
pub fn extract_last_review_verdict(body: &str) -> Option<String> {
    let lines: Vec<&str> = body.lines().collect();
    let mut in_review = false;
    let mut last_verdict: Option<String> = None;

    for line in &lines {
        if is_heading(line) {
            in_review = line.trim() == "## Review";
            continue;
        }
        if in_review {
            // Match ^verdict:\s*\S...
            if let Some(stripped) = line.strip_prefix("verdict:") {
                let val = stripped.trim();
                if !val.is_empty() {
                    last_verdict = Some(val.to_string());
                }
            }
        }
    }

    last_verdict
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // split_frontmatter
    // -----------------------------------------------------------------------

    #[test]
    fn test_split_canonical() {
        let blob = "---\nid: test\nkind: task\n---\n\n## Goal\n\nBody.\n";
        let s = split_frontmatter(blob);
        assert_eq!(s.fm, "id: test\nkind: task");
        assert_eq!(s.body, "\n## Goal\n\nBody.\n");
        assert_eq!(s.raw, blob);
    }

    #[test]
    fn test_split_no_opening() {
        let blob = "# Just markdown\n\nNo frontmatter.\n";
        let s = split_frontmatter(blob);
        assert_eq!(s.fm, "");
        assert_eq!(s.body, blob);
    }

    #[test]
    fn test_split_malformed_no_closing() {
        let blob = "---\nid: test\n";
        let s = split_frontmatter(blob);
        assert_eq!(s.fm, "");
        assert_eq!(s.body, blob);
    }

    #[test]
    fn test_split_no_body() {
        let blob = "---\nid: test\n---\n";
        let s = split_frontmatter(blob);
        assert_eq!(s.fm, "id: test");
        assert_eq!(s.body, "");
    }

    #[test]
    fn test_split_thematic_break_no_reentry() {
        let blob = "---\nid: thematic-break-test\nkind: task\n---\n\nSome text.\n\n---\n\nid: fake-id\nstatus: done\n\nMore body.\n";
        let s = split_frontmatter(blob);
        assert!(s.fm.contains("id: thematic-break-test"));
        assert!(!s.fm.contains("fake-id"));
        assert!(s.body.contains("---"));
        assert!(s.body.contains("id: fake-id"));
    }

    // -----------------------------------------------------------------------
    // parse_frontmatter
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_inline_deps() {
        let raw = "id: proxy\nkind: task\ndepends_on: [a, b]\n";
        let val = parse_frontmatter(raw).unwrap().unwrap();
        assert_eq!(val["id"].as_str(), Some("proxy"));
        assert_eq!(val["depends_on"][0].as_str(), Some("a"));
    }

    #[test]
    fn test_parse_block_deps() {
        let raw = "id: proxy\ndepends_on:\n  - a\n  - b\n";
        let val = parse_frontmatter(raw).unwrap().unwrap();
        assert_eq!(val["id"].as_str(), Some("proxy"));
        assert_eq!(val["depends_on"][0].as_str(), Some("a"));
        assert_eq!(val["depends_on"][1].as_str(), Some("b"));
    }

    #[test]
    fn test_parse_empty() {
        assert_eq!(parse_frontmatter(""), Ok(None));
        assert_eq!(parse_frontmatter("   "), Ok(None));
    }

    #[test]
    fn test_parse_quoted_status() {
        let raw = "status: \"done\"\n";
        let val = parse_frontmatter(raw).unwrap().unwrap();
        assert_eq!(val["status"].as_str(), Some("done"));
    }

    #[test]
    fn test_parse_null() {
        let raw = "parent:\n";
        let val = parse_frontmatter(raw).unwrap().unwrap();
        assert!(val["parent"].is_null());
    }

    #[test]
    fn test_parse_unquoted_colon_in_title_is_an_error() {
        // The exact shape `planr new` used to scaffold: a colon-bearing title
        // left unquoted turns the whole block into a parse error, not a
        // mapping with missing fields.
        let raw = "id: shadow-remote
kind: epic
title: The shadow remote: git-native sync
";
        let err = parse_frontmatter(raw).unwrap_err();
        assert!(!err.is_empty());
    }

    #[test]
    fn test_parse_non_mapping_is_not_an_error() {
        // A scalar or sequence document parses fine but carries no fields.
        assert_eq!(parse_frontmatter("just a string").unwrap(), None);
        assert_eq!(
            parse_frontmatter(
                "- a
- b"
            )
            .unwrap(),
            None
        );
    }

    // -----------------------------------------------------------------------
    // extract_wiki_links
    // -----------------------------------------------------------------------

    #[test]
    fn test_wiki_links_basic() {
        let body = "See [[slug]] and [[other|label]] and [[page#heading]].";
        let links = extract_wiki_links(body);
        assert_eq!(links, vec!["slug", "other", "page"]);
    }

    #[test]
    fn test_wiki_links_dedupe() {
        let body = "[[a]] and [[a]] and [[a|label]]";
        let links = extract_wiki_links(body);
        assert_eq!(links, vec!["a"]);
    }

    #[test]
    fn test_wiki_links_skip_backtick_fence() {
        let body = "```\n[[inside]]\n```\n[[outside]]";
        let links = extract_wiki_links(body);
        assert_eq!(links, vec!["outside"]);
    }

    #[test]
    fn test_wiki_links_skip_tilde_fence() {
        let body = "~~~\n[[inside]]\n~~~\n[[outside]]";
        let links = extract_wiki_links(body);
        assert_eq!(links, vec!["outside"]);
    }

    // -----------------------------------------------------------------------
    // extract_section
    // -----------------------------------------------------------------------

    #[test]
    fn test_extract_section_basic() {
        let body = "## Goal\nDo stuff.\n\n## Context\nMore.\n";
        let section = extract_section(body, "Goal");
        assert_eq!(section, "Do stuff.");
    }

    #[test]
    fn test_extract_section_multiline() {
        let body = "## Acceptance\n- [ ] one\n- [ ] two\n\n## Notes\n";
        let section = extract_section(body, "Acceptance");
        assert_eq!(section, "- [ ] one\n- [ ] two");
    }

    #[test]
    fn test_extract_section_missing() {
        let body = "## Goal\nStuff.\n";
        let section = extract_section(body, "Acceptance");
        assert_eq!(section, "");
    }

    #[test]
    fn test_extract_section_trim_blanks() {
        let body = "## Goal\n\n\nContent\n\n\n## Next\n";
        let section = extract_section(body, "Goal");
        assert_eq!(section, "Content");
    }

    // -----------------------------------------------------------------------
    // extract_last_review_verdict
    // -----------------------------------------------------------------------

    #[test]
    fn test_verdict_last_wins() {
        let body = "## Review\nverdict: changes-requested\n\n## Review\nverdict: approved\n";
        let v = extract_last_review_verdict(body);
        assert_eq!(v.as_deref(), Some("approved"));
    }

    #[test]
    fn test_verdict_none() {
        let body = "## Goal\nJust stuff.\n";
        let v = extract_last_review_verdict(body);
        assert_eq!(v, None);
    }

    #[test]
    fn test_verdict_trimmed() {
        let body = "## Review\nverdict:   approved   \n";
        let v = extract_last_review_verdict(body);
        assert_eq!(v.as_deref(), Some("approved"));
    }

    #[test]
    fn test_verdict_no_verdict_line() {
        let body = "## Review\nJust some notes.\n";
        let v = extract_last_review_verdict(body);
        assert_eq!(v, None);
    }
}
