//! Typed ticket representation -- the shape every command consumes.
//!
//! Port of `skills/planr/src/ticket.ts`.

use serde_yaml::Value;

use crate::parse::{extract_wiki_links, parse_frontmatter, split_frontmatter};

/// Every status a ticket's frontmatter may carry.
///
/// One list, because the commands disagree destructively otherwise: `lint`
/// rejects what is not here, and `board` decides from it whether a branch
/// reported something it can display and count. A status added to only one
/// copy would be flagged as invalid by `lint`, or silently miscounted by
/// `board`, depending on which copy was missed.
pub const VALID_STATUSES: [&str; 6] = [
    "todo",
    "in_progress",
    "review",
    "done",
    "blocked",
    "abandoned",
];

/// Ticket kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    Epic,
    Story,
    Task,
}

impl Kind {
    fn from_str(s: &str) -> Option<Kind> {
        match s {
            "epic" => Some(Kind::Epic),
            "story" => Some(Kind::Story),
            "task" => Some(Kind::Task),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// ParsedTicket
// ---------------------------------------------------------------------------

/// Parsed ticket data -- the typed shape every script consumes.
#[derive(Debug, Clone)]
pub struct ParsedTicket {
    pub id: String,
    pub kind: Option<Kind>,
    pub status: String,
    pub parent: Option<String>,
    pub title: String,
    pub depends_on: Vec<String>,
    #[allow(dead_code)]
    pub aliases: Vec<String>,
    pub links: Vec<String>,
    /// Raw body text (everything after the frontmatter block).
    pub raw: String,
    /// serde_yaml's message when the frontmatter block failed to parse. Every
    /// field above is then absent, so consumers must report this rather than
    /// the fields it swallowed.
    pub frontmatter_error: Option<String>,
    /// True when `id` above was not read from the file, but synthesized from
    /// the filename because the frontmatter carried none.
    ///
    /// The slug is then a guess, however good a one: it is the name of a file
    /// that may sit next to another file claiming the same slug for real. A
    /// synthesised id is fine to *name* a ticket with, which is why the
    /// readers fill it in, but it must never be treated as the ticket's own
    /// identity -- notably, nothing keyed on it may shadow a ticket that
    /// declared that slug itself.
    pub id_from_filename: bool,
    /// The file the ticket was read from, when it was read from one. Warnings
    /// use it to tell two broken files of the same slug apart.
    pub source_file: Option<String>,
}

/// Extract the slug from a ticket filename: strip the directory, the `.md`
/// suffix, and the `NN-` sort-hint prefix.
///
/// The filename is the one piece of a ticket that is still readable when the
/// frontmatter is not, so both `lint` and `board` fall back to it to name a
/// file they could not parse.
pub fn slug_from_filename(file: &str) -> String {
    let base = std::path::Path::new(file)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    let no_md = base.strip_suffix(".md").unwrap_or(base);
    // Strip a leading NN- prefix: "01-foo" -> "foo", "foo" -> "foo".
    if let Some(hyphen_at) = no_md.find('-') {
        let prefix = &no_md[..hyphen_at];
        if !prefix.is_empty() && prefix.chars().all(|c| c.is_ascii_digit()) {
            return no_md[hyphen_at + 1..].to_string();
        }
    }
    no_md.to_string()
}

/// The `NN-<slug>.md` file among `files`, if it is there.
///
/// A ticket filename carries a sort-hint prefix, so a slug does not name its
/// file on its own. The match is anchored to a whole final path segment --
/// `/NN-<slug>.md` at the end -- which is what keeps `t1` from matching
/// `.plan/tasks/03-not-t1.md`, and `regex::escape` is what keeps a slug
/// holding a metacharacter literal.
///
/// `claim::find_task_file` matches the same files more loosely on purpose;
/// that is a port decision documented where it lives, not a fourth copy of
/// this waiting to be folded in.
pub fn find_by_slug(files: &[String], slug: &str) -> Option<String> {
    let pattern = format!(r"/[0-9]+-{}\.md$", regex::escape(slug));
    // The pattern is built from an escaped slug, so it always compiles.
    let re = regex::Regex::new(&pattern).unwrap();
    files.iter().find(|f| re.is_match(f)).cloned()
}

/// Parse a complete ticket blob (frontmatter + body) into a `ParsedTicket`.
pub fn parse_ticket(blob: &str) -> ParsedTicket {
    let split = split_frontmatter(blob);
    let (front, frontmatter_error) = match parse_frontmatter(&split.fm) {
        Ok(front) => (front, None),
        Err(e) => (None, Some(e)),
    };

    let get_str = |key: &str| -> String {
        front
            .as_ref()
            .and_then(|v| v.get(key))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_default()
    };

    let id = get_str("id");
    let kind = {
        let ks = get_str("kind");
        Kind::from_str(&ks)
    };
    let status = get_str("status");
    let status = if status.is_empty() {
        "todo".to_string()
    } else {
        status
    };

    let parent = front.as_ref().and_then(|v| v.get("parent")).and_then(|v| {
        if v.is_null() {
            None
        } else {
            v.as_str().map(|s| s.to_string())
        }
    });

    let title = get_str("title");

    // depends_on: inline list [a,b], block list, single bare string, or null
    let depends_on = extract_list(&front, "depends_on");

    // aliases: same coercion
    let aliases = extract_list(&front, "aliases");

    let links = extract_wiki_links(&split.body);

    ParsedTicket {
        id,
        kind,
        status,
        parent,
        title,
        depends_on,
        aliases,
        links,
        raw: split.body,
        frontmatter_error,
        id_from_filename: false,
        source_file: None,
    }
}

/// Extract a field as a list of strings, handling inline list `[a, b]`,
/// block list (serde_yaml sequences), single bare string, or null/absent.
fn extract_list(front: &Option<Value>, key: &str) -> Vec<String> {
    let val = match front.as_ref().and_then(|v| v.get(key)) {
        Some(v) => v,
        None => return Vec::new(),
    };

    match val {
        Value::Sequence(seq) => seq
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect(),
        Value::String(s) if !s.trim().is_empty() => vec![s.trim().to_string()],
        Value::String(_) => Vec::new(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_canonical() {
        let blob = "---\nid: http-connect-proxy\naliases: [http-connect-proxy]\nkind: task\nparent: network-firewall\ntitle: HTTP CONNECT allowlist proxy\nstatus: review\nassignee: null\ncreated: 2026-08-01\nupdated: 2026-08-01\ntags: []\ndepends_on: [parse-foundation, cli-scaffolding]\n---\n\n## Goal\n\nImplement the proxy.\n";
        let t = parse_ticket(blob);
        assert_eq!(t.id, "http-connect-proxy");
        assert_eq!(t.kind, Some(Kind::Task));
        assert_eq!(t.status, "review");
        assert_eq!(t.parent.as_deref(), Some("network-firewall"));
        assert_eq!(t.title, "HTTP CONNECT allowlist proxy");
        assert_eq!(t.depends_on, vec!["parse-foundation", "cli-scaffolding"]);
        assert_eq!(t.aliases, vec!["http-connect-proxy"]);
        assert!(t.raw.contains("## Goal"));
    }

    #[test]
    fn test_parse_block_depends_on() {
        let blob = "---\nid: test\nkind: task\nparent: story\nstatus: todo\ntitle: Test\ndepends_on:\n  - a\n  - b\n---\n";
        let t = parse_ticket(blob);
        assert_eq!(t.depends_on, vec!["a", "b"]);
    }

    #[test]
    fn test_parse_single_dep_as_string() {
        let blob = "---\nid: test\nkind: task\nparent: story\nstatus: todo\ntitle: Test\ndepends_on: some-task\n---\n";
        let t = parse_ticket(blob);
        assert_eq!(t.depends_on, vec!["some-task"]);
    }

    #[test]
    fn test_parse_no_deps() {
        let blob = "---\nid: test\nkind: task\nparent: story\nstatus: todo\ntitle: Test\ndepends_on: []\n---\n";
        let t = parse_ticket(blob);
        let empty: Vec<String> = Vec::new();
        assert_eq!(t.depends_on, empty);
    }

    #[test]
    fn test_parse_absent_deps() {
        let blob = "---\nid: test\nkind: task\nparent: story\nstatus: todo\ntitle: Test\n---\n";
        let t = parse_ticket(blob);
        let empty: Vec<String> = Vec::new();
        assert_eq!(t.depends_on, empty);
    }

    #[test]
    fn test_parse_absent_parent() {
        let blob = "---\nid: test\nkind: epic\nstatus: todo\ntitle: Test\n---\n";
        let t = parse_ticket(blob);
        assert_eq!(t.parent, None);
    }

    #[test]
    fn test_parse_null_parent() {
        let blob = "---\nid: test\nkind: task\nparent:\nstatus: todo\ntitle: Test\n---\n";
        let t = parse_ticket(blob);
        assert_eq!(t.parent, None);
    }

    #[test]
    fn test_parse_missing_status_defaults_todo() {
        let blob = "---\nid: test\nkind: task\nparent: story\ntitle: Test\n---\n";
        let t = parse_ticket(blob);
        assert_eq!(t.status, "todo");
    }

    #[test]
    fn test_parse_quoted_status() {
        let blob = "---\nid: test\nkind: task\nparent: story\nstatus: \"done\"\ntitle: Test\n---\n";
        let t = parse_ticket(blob);
        assert_eq!(t.status, "done");
    }

    #[test]
    fn test_parse_unquoted_colon_records_frontmatter_error() {
        let blob = "---\nid: shadow-remote\nkind: epic\ntitle: The shadow remote: git-native sync\nstatus: todo\n---\n\n## Goal\n";
        let t = parse_ticket(blob);
        assert!(t.frontmatter_error.is_some());
        // Every field is swallowed by the failed parse -- that is exactly why
        // callers need the error instead of the empty fields.
        assert_eq!(t.id, "");
        assert_eq!(t.kind, None);
    }

    #[test]
    fn test_parse_clean_frontmatter_has_no_error() {
        let blob = "---\nid: test\nkind: task\nparent: story\nstatus: todo\ntitle: \"Quoted: colon\"\n---\n";
        let t = parse_ticket(blob);
        assert_eq!(t.frontmatter_error, None);
        assert_eq!(t.title, "Quoted: colon");
    }

    #[test]
    fn test_parse_wiki_links_from_body() {
        let blob = "---\nid: test\nkind: task\nparent: story\nstatus: todo\ntitle: Test\n---\n\nSee [[other-task]].\n";
        let t = parse_ticket(blob);
        assert_eq!(t.links, vec!["other-task"]);
    }

    #[test]
    fn test_parse_obsidian_reformatted() {
        let blob = "---\nid: http-connect-proxy\naliases:\n  - http-connect-proxy\nkind: task\nparent: network-firewall\ntitle: HTTP CONNECT allowlist proxy\nstatus: \"done\"\nassignee: null\ncreated: 2026-08-01\nupdated: 2026-08-01\ntags: []\ndepends_on:\n  - parse-foundation\n  - cli-scaffolding\n---\n\n## Goal\n\nObsidian-reformatted.\n";
        let t = parse_ticket(blob);
        assert_eq!(t.id, "http-connect-proxy");
        assert_eq!(t.status, "done");
        assert_eq!(t.depends_on, vec!["parse-foundation", "cli-scaffolding"]);
        assert_eq!(t.aliases, vec!["http-connect-proxy"]);
    }
}
