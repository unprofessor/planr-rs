//! Pure frontmatter/body parsers -- no I/O, no git.
//!
//! These functions are the direct port of `skills/planr/src/parse.ts`.

use std::sync::LazyLock;

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
            }
        }
        Some(idx) => {
            let fm = parts[1..idx].join("\n");
            let body = parts[idx + 1..].join("\n");
            FrontmatterSplit { fm, body }
        }
    }
}

/// Result of [`split_frontmatter`].
pub struct FrontmatterSplit {
    pub fm: String,
    pub body: String,
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

/// Fenced code blocks (both backtick and tilde).
static FENCE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"```[\s\S]*?```|~~~[\s\S]*?~~~").unwrap());

/// Byte ranges of every code span in `body` -- fenced blocks and inline spans.
///
/// A `[[slug]]` inside code is a piece of writing about the link syntax, not a
/// link, and the backlog is full of tickets that say so: the parser's own
/// ticket documents `` `[[a|label]]` `` as a case to handle. Reading those as
/// references makes `lint` report dangling links that no author wrote, and
/// makes a renderer rewrite the sample it was quoting.
///
/// Inline spans follow the CommonMark rule -- a run of N backticks closes on
/// the next run of exactly N -- which is why this is a scan and not another
/// regex: the crate has no backreferences to match a run against itself.
fn code_spans(body: &str) -> Vec<std::ops::Range<usize>> {
    let mut spans: Vec<std::ops::Range<usize>> =
        FENCE_RE.find_iter(body).map(|m| m.range()).collect();

    let bytes = body.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // A fence is already accounted for; step over it whole so its
        // contents cannot open an inline span.
        if let Some(f) = spans.iter().find(|f| f.start == i) {
            i = f.end;
            continue;
        }
        if bytes[i] != b'`' {
            i += 1;
            continue;
        }
        let open = run_len(bytes, i);
        match close_at(bytes, i + open, open) {
            Some(close) => {
                spans.push(i..close + open);
                i = close + open;
            }
            // An unmatched run is literal backticks. Step past the run rather
            // than one byte, or the next iteration re-opens on its tail.
            None => i += open,
        }
    }
    spans
}

/// Length of the backtick run starting at `at`.
fn run_len(bytes: &[u8], at: usize) -> usize {
    let mut n = 0;
    while at + n < bytes.len() && bytes[at + n] == b'`' {
        n += 1;
    }
    n
}

/// Where a run of exactly `want` backticks starts, at or after `from`.
fn close_at(bytes: &[u8], from: usize, want: usize) -> Option<usize> {
    let mut i = from;
    while i < bytes.len() {
        if bytes[i] != b'`' {
            i += 1;
            continue;
        }
        let n = run_len(bytes, i);
        if n == want {
            return Some(i);
        }
        i += n;
    }
    None
}

/// A wiki-link, in the shape the body writes it.
///
/// Regex breakdown:
///   \[\[            literal [[
///   ([^\]|#]+)     capture group 1: slug (no ], |, or #)
///   (?:#([^\]|]*))? optional #heading, captured
///   (?:\|([^\]]*))? optional |alias, captured
///   \]\]            literal ]]
///
/// The heading capture stops at `|` where the slug-only version ran to `]`.
/// Both accept the same strings and read the same slug out of them; only the
/// split between heading and alias differs, which nothing looked at before.
static LINK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[\[([^\]|#]+)(?:#([^\]|]*))?(?:\|([^\]]*))?\]\]").unwrap());

/// One wiki-link found in a body.
///
/// Only `slug` has a reader without the `serve` feature -- `extract_wiki_links`
/// throws the rest away -- so the position and label are dead code in a
/// `--no-default-features` build and live code in every other.
#[cfg_attr(not(feature = "serve"), allow(dead_code))]
pub struct WikiLink {
    /// Byte range of the whole `[[...]]` in the body it was found in.
    pub range: std::ops::Range<usize>,
    /// The slug it points at, with any heading and alias stripped.
    pub slug: String,
    /// What to show for it: the alias when it carries one, otherwise the
    /// slug with its heading still attached.
    pub label: String,
}

/// Every wiki-link in `body`, in the order they appear and including repeats.
///
/// The single scan behind both [`extract_wiki_links`] and anything that
/// rewrites links in place. A second scanner would eventually disagree with
/// this one about which links are real -- lint would report a dangling link
/// that the rendered page shows as live, or the reverse -- and the reader has
/// no way to tell which of the two is wrong. Do not add one.
///
/// Code is skipped by range rather than by deleting it, because a caller
/// replacing a link needs the offset it has in the original body.
pub fn wiki_links(body: &str) -> Vec<WikiLink> {
    let code = code_spans(body);
    let in_code = |at: usize| code.iter().any(|f| f.contains(&at));

    let mut out = Vec::new();
    for cap in LINK_RE.captures_iter(body) {
        // The whole match and group 1 are always present when the pattern
        // matched, so neither `get` can be None here.
        let whole = cap.get(0).unwrap();
        if in_code(whole.start()) {
            continue;
        }
        let slug = cap.get(1).unwrap().as_str().to_string();
        let label = match cap.get(3) {
            Some(alias) => alias.as_str().to_string(),
            None => match cap.get(2) {
                Some(heading) => format!("{slug}#{}", heading.as_str()),
                None => slug.clone(),
            },
        };
        out.push(WikiLink {
            range: whole.range(),
            slug,
            label,
        });
    }
    out
}

/// The distinct slugs [`wiki_links`] found, in first-seen order.
pub fn extract_wiki_links(body: &str) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    wiki_links(body)
        .into_iter()
        .map(|l| l.slug)
        .filter(|slug| seen.insert(slug.clone()))
        .collect()
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
    // wiki_links -- ranges and labels
    // -----------------------------------------------------------------------

    #[test]
    fn test_wiki_links_range_indexes_the_original_body() {
        // The range has to address the body as given, fences and all -- a
        // caller replacing a link seeks with it.
        let body = "```\n[[inside]]\n```\nsee [[outside]] here";
        let links = wiki_links(body);
        assert_eq!(links.len(), 1);
        assert_eq!(&body[links[0].range.clone()], "[[outside]]");
    }

    #[test]
    fn test_wiki_links_labels() {
        let body = "[[plain]] [[slug|Alias]] [[slug#heading]] [[slug#heading|Alias]]";
        let labels: Vec<String> = wiki_links(body).into_iter().map(|l| l.label).collect();
        assert_eq!(labels, vec!["plain", "Alias", "slug#heading", "Alias"]);
    }

    #[test]
    fn test_wiki_links_skip_inline_code() {
        // The parser's own ticket writes `[[a|label]]` in backticks as a case
        // to handle. Reading that as a reference invents a dangling link.
        let body = "handle `[[a|label]]` and `[[b#heading]]`, but [[real]] counts";
        assert_eq!(extract_wiki_links(body), vec!["real"]);
    }

    #[test]
    fn test_wiki_links_inline_code_run_must_match() {
        // A run of two closes on a run of two, not on the single backtick
        // inside it.
        let body = "``a ` [[hidden]] b`` then [[shown]]";
        assert_eq!(extract_wiki_links(body), vec!["shown"]);
    }

    #[test]
    fn test_wiki_links_unmatched_backtick_is_literal() {
        // One stray backtick opens nothing, so the link after it is real.
        let body = "a ` stray tick and [[still-a-link]]";
        assert_eq!(extract_wiki_links(body), vec!["still-a-link"]);
    }

    #[test]
    fn test_wiki_links_keeps_repeats() {
        // extract_wiki_links dedupes; the scan under it must not, or a body
        // with the same link twice loses the second one's position.
        let body = "[[a]] then [[a]]";
        assert_eq!(wiki_links(body).len(), 2);
        assert_eq!(extract_wiki_links(body), vec!["a"]);
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
