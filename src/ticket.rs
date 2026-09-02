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
    /// field above then reads as absent, so consumers must report this rather
    /// than the fields it swallowed.
    pub frontmatter_error: Option<String>,
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
    if status.is_empty() {
        // TS defaults to "todo" when missing -- mirror that.
        // Note: the actual default is set in parseTicket TS:
        //    const status = String(front.status ?? "todo") as Status;
        // If the field is absent, YAML key is missing -> as_str -> None -> "";
        // Default to "todo" to match TS.
    }
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
