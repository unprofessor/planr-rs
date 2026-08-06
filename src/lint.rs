//! Pure lint engine — three-pass backlog checker.
//!
//! Port of `skills/planr/src/lint.ts`. The engine is pure (takes tickets,
//! returns issues); the CLI I/O (working tree scan, ref scan) is separate.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::git;
use crate::ticket::{Kind, ParsedTicket};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Level {
    Error,
    Warning,
}

impl std::fmt::Display for Level {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Level::Error => write!(f, "error"),
            Level::Warning => write!(f, "warning"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LintIssue {
    pub file: String,
    pub level: Level,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct LintInput {
    pub file: String,
    pub ticket: ParsedTicket,
}

#[derive(Debug, Clone)]
pub struct LintReport {
    pub issues: Vec<LintIssue>,
    pub error_count: usize,
    pub warning_count: usize,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Determine the expected kind from the file path (containing `/epics/`,
/// `/stories/`, or `/tasks/`).
fn dir_kind_from_path(file: &str) -> Option<&'static str> {
    if file.contains("/epics/") {
        Some("epic")
    } else if file.contains("/stories/") {
        Some("story")
    } else if file.contains("/tasks/") {
        Some("task")
    } else {
        None
    }
}

/// Extract the slug from a filename: strip the directory, `.md` suffix, and
/// the `NN-` sort-hint prefix.
fn slug_from_filename(file: &str) -> String {
    let base = Path::new(file)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    let no_md = base.strip_suffix(".md").unwrap_or(base);
    // Strip leading NN- prefix (TS equivalent: replace(/^\d+-/, ''))
    // e.g. "01-foo" → "foo", "foo" → "foo"
    if let Some(hyphen_at) = no_md.find('-') {
        let prefix = &no_md[..hyphen_at];
        if !prefix.is_empty() && prefix.chars().all(|c| c.is_ascii_digit()) {
            return no_md[hyphen_at + 1..].to_string();
        }
    }
    no_md.to_string()
}

fn escape_regex(s: &str) -> String {
    regex::escape(s)
}

/// Return the parent kind that a given kind expects.
fn expected_parent_kind(kind: &Kind) -> Option<&'static str> {
    match kind {
        Kind::Epic => None,
        Kind::Story => Some("epic"),
        Kind::Task => Some("story"),
    }
}

// ---------------------------------------------------------------------------
// Three-pass engine
// ---------------------------------------------------------------------------

/// Run the three-pass lint check over a backlog of parsed tickets.
pub fn check_backlog(inputs: &[LintInput]) -> LintReport {
    let mut issues: Vec<LintIssue> = Vec::new();

    // Indexes built in pass 1
    let mut file_of: HashMap<String, String> = HashMap::new(); // id → file
    let mut kind_of: HashMap<String, Kind> = HashMap::new();
    let mut parent_of: HashMap<String, Option<String>> = HashMap::new();
    let mut deps_of: HashMap<String, Vec<String>> = HashMap::new();
    let mut links_of: HashMap<String, Vec<String>> = HashMap::new();

    // ---- pass 1: per-file checks (no cross-refs) ----
    for input in inputs {
        let file = &input.file;
        let ticket = &input.ticket;

        let dir_kind = dir_kind_from_path(file);
        let fslug = slug_from_filename(file);

        // Validate id matches filename slug
        if ticket.id.is_empty() {
            issues.push(LintIssue {
                file: file.clone(),
                level: Level::Error,
                message: "missing id in frontmatter".to_string(),
            });
        } else if ticket.id != fslug {
            issues.push(LintIssue {
                file: file.clone(),
                level: Level::Error,
                message: format!(
                    "id '{}' does not match filename slug '{}'",
                    ticket.id, fslug
                ),
            });
        }

        // Validate kind matches directory (if directory is known)
        if let Some(dk) = dir_kind {
            let ticket_kind_str = match &ticket.kind {
                Some(Kind::Epic) => "epic",
                Some(Kind::Story) => "story",
                Some(Kind::Task) => "task",
                None => "<missing>",
            };
            if ticket_kind_str != dk {
                issues.push(LintIssue {
                    file: file.clone(),
                    level: Level::Error,
                    message: format!(
                        "kind '{}' but the file lives in the {}s directory",
                        ticket_kind_str, dk
                    ),
                });
            }
        }

        // Validate status
        let valid_statuses = [
            "todo", "in_progress", "review", "done", "blocked",
        ];
        if !valid_statuses.contains(&ticket.status.as_str()) {
            let display = if ticket.status.is_empty() {
                "<missing>".to_string()
            } else {
                ticket.status.clone()
            };
            issues.push(LintIssue {
                file: file.clone(),
                level: Level::Error,
                message: format!(
                    "invalid status '{}' (want todo|in_progress|review|done|blocked)",
                    display
                ),
            });
        }

        // Duplicate slug check
        let id = if ticket.id.is_empty() {
            fslug.clone()
        } else {
            ticket.id.clone()
        };
        if file_of.contains_key(&id) {
            issues.push(LintIssue {
                file: file.clone(),
                level: Level::Error,
                message: format!(
                    "duplicate slug '{}' (also {}) — slugs are identity \
                     and must be unique across the backlog",
                    id,
                    file_of.get(&id).unwrap()
                ),
            });
            continue; // skip indexing duplicates
        }

        file_of.insert(id.clone(), file.clone());
        kind_of.insert(id.clone(), ticket.kind.clone().unwrap_or(Kind::Task));
        parent_of.insert(id.clone(), ticket.parent.clone());
        deps_of.insert(id.clone(), ticket.depends_on.clone());
        links_of.insert(id.clone(), ticket.links.clone());
    }

    // ---- pass 2: cross-reference checks ----
    let mut sorted_ids: Vec<&String> = file_of.keys().collect();
    sorted_ids.sort();
    for id in &sorted_ids {
        let file = file_of.get(*id).unwrap();
        let kind = kind_of.get(*id).unwrap();
        let parent = parent_of.get(*id).and_then(|p| p.as_deref());

        // Parent checks
        if *kind == Kind::Epic {
            if let Some(p) = parent {
                if p != "null" {
                    issues.push(LintIssue {
                        file: file.clone(),
                        level: Level::Error,
                        message: format!(
                            "epics must not have a parent (found '{}')",
                            p
                        ),
                    });
                }
            }
        } else {
            // story or task
            match parent {
                None | Some("null") => {
                    let kind_str = match kind {
                        Kind::Story => "story",
                        Kind::Task => "task",
                        _ => unreachable!(),
                    };
                    issues.push(LintIssue {
                        file: file.clone(),
                        level: Level::Error,
                        message: format!("a {kind_str} must name a parent slug"),
                    });
                }
                Some(p) => {
                    if !file_of.contains_key(p) {
                        let kind_str = match kind {
                            Kind::Story => "story",
                            Kind::Task => "task",
                            _ => unreachable!(),
                        };
                        issues.push(LintIssue {
                            file: file.clone(),
                            level: Level::Error,
                            message: format!(
                                "parent '{}' does not exist — roll-up is derived \
                                 by scanning children, so this {} would be orphaned",
                                p, kind_str
                            ),
                        });
                    } else {
                        // Wrong-kind parent check (warning only)
                        let expected = expected_parent_kind(kind);
                        let parent_actual_kind = kind_of.get(p);
                        if let (Some(exp), Some(pk)) = (expected, parent_actual_kind) {
                            let pk_str = match pk {
                                Kind::Epic => "epic",
                                Kind::Story => "story",
                                Kind::Task => "task",
                            };
                            if pk_str != exp {
                                let kind_str = match kind {
                                    Kind::Story => "story",
                                    Kind::Task => "task",
                                    _ => unreachable!(),
                                };
                                issues.push(LintIssue {
                                    file: file.clone(),
                                    level: Level::Warning,
                                    message: format!(
                                        "parent '{}' is a {} (a {}'s parent is \
                                         usually a {})",
                                        p, pk_str, kind_str, exp
                                    ),
                                });
                            }
                        }
                    }
                }
            }
        }

        // Depends_on checks
        if let Some(deps) = deps_of.get(*id) {
            for d in deps {
                if d == *id {
                    issues.push(LintIssue {
                        file: file.clone(),
                        level: Level::Error,
                        message: "depends_on itself".to_string(),
                    });
                } else if !file_of.contains_key(d) {
                    // TS uses the exact: depends_on '<dep>' does not exist — claim.sh could never be satisfied
                    issues.push(LintIssue {
                        file: file.clone(),
                        level: Level::Error,
                        message: format!(
                            "depends_on '{}' does not exist — planr claim \
                             could never be satisfied",
                            d
                        ),
                    });
                }
            }
        }

        // Wiki-link checks
        if let Some(links) = links_of.get(*id) {
            for l in links {
                if !file_of.contains_key(l) {
                    issues.push(LintIssue {
                        file: file.clone(),
                        level: Level::Warning,
                        message: format!(
                            "[[{}]] matches no ticket slug (fine if it points \
                             at a non-ticket note)",
                            l
                        ),
                    });
                }
            }
        }
    }

    // ---- pass 3: cycle detection (DFS) ----
    // Use owned Strings to avoid lifetime complexity with nested borrows.
    let mut color: HashMap<String, &str> = HashMap::new(); // id → "w", "g", "b"
    let mut stack: Vec<String> = Vec::new();

    fn visit(
        n: &str,
        file_of: &HashMap<String, String>,
        deps_of: &HashMap<String, Vec<String>>,
        color: &mut HashMap<String, &str>,
        stack: &mut Vec<String>,
        issues: &mut Vec<LintIssue>,
    ) {
        let c = color.get(n).copied().unwrap_or("w");
        if c == "b" {
            return;
        }
        if c == "g" {
            // Found a cycle — build the cycle path
            let mut started = false;
            let mut parts: Vec<&str> = Vec::new();
            for s in stack.iter() {
                if s == n {
                    started = true;
                }
                if started {
                    parts.push(s);
                }
            }
            let cycle = parts.join(" -> ");
            let full_cycle = format!("{} -> {}", cycle, n);
            issues.push(LintIssue {
                file: file_of.get(n).cloned().unwrap_or_default(),
                level: Level::Error,
                message: format!(
                    "depends_on cycle: {} — nothing in the cycle can ever \
                     be claimed",
                    full_cycle
                ),
            });
            return;
        }

        color.insert(n.to_string(), "g");
        stack.push(n.to_string());
        if let Some(deps) = deps_of.get(n) {
            for d in deps {
                // Self-dependency already reported in pass 2; skip to avoid
                // one-node "cycle" report.
                if d == n {
                    continue;
                }
                if file_of.contains_key(d) {
                    visit(d, file_of, deps_of, color, stack, issues);
                }
            }
        }
        color.insert(n.to_string(), "b");
        stack.pop();
    }

    for id in sorted_ids {
        visit(id, &file_of, &deps_of, &mut color, &mut stack, &mut issues);
    }

    // ---- counts ----
    let error_count = issues.iter().filter(|i| i.level == Level::Error).count();
    let warning_count = issues.iter().filter(|i| i.level == Level::Warning).count();

    LintReport {
        issues,
        error_count,
        warning_count,
    }
}

/// Render lint output: one line per issue, then a summary line if there were
/// any issues. Returns empty string when there are no issues (matching TS
/// silent-on-empty behavior).
pub fn render_report(report: &LintReport) -> String {
    let mut out = String::new();
    for issue in &report.issues {
        out.push_str(&format!(
            "{}: {}: {}\n",
            issue.level, issue.file, issue.message
        ));
    }
    if report.error_count > 0 || report.warning_count > 0 {
        out.push_str(&format!(
            "lint: {} error(s), {} warning(s)\n",
            report.error_count, report.warning_count
        ));
    }
    out
}

/// Run lint in ref mode: read tickets from a git ref.
pub fn lint_ref(ref_: &str, plan_dir: &str) -> LintReport {
    let kinds = ["epics", "stories", "tasks"];
    let mut inputs = Vec::new();

    for kind in &kinds {
        let dir = format!("{plan_dir}/{kind}");
        let files = match git::ls_tree_md(ref_, &dir) {
            Ok(f) => f,
            Err(_) => continue,
        };
        for f in &files {
            if !f.ends_with(".md") {
                continue;
            }
            let blob = match git::show_ref(ref_, f) {
                Ok(b) => b,
                Err(_) => continue,
            };
            let ticket = crate::ticket::parse_ticket(&blob);
            inputs.push(LintInput {
                file: f.clone(),
                ticket,
            });
        }
    }

    check_backlog(&inputs)
}

/// Run lint in working-tree mode: scan the local filesystem.
pub fn lint_working_tree(plan_dir: &str) -> LintReport {
    let kinds = ["epics", "stories", "tasks"];
    let mut inputs = Vec::new();

    for kind in &kinds {
        let dir = format!("{plan_dir}/{kind}");
        let dir_path = std::path::Path::new(&dir);
        if !dir_path.exists() {
            continue;
        }
        let mut entries: Vec<_> = match std::fs::read_dir(dir_path) {
            Ok(rd) => rd.filter_map(|e| e.ok()).map(|e| e.path()).collect(),
            Err(_) => continue,
        };
        entries.sort();
        for entry in &entries {
            if !entry.extension().map_or(false, |e| e == "md") {
                continue;
            }
            if !entry.is_file() {
                continue;
            }
            let blob = match std::fs::read_to_string(entry) {
                Ok(b) => b,
                Err(_) => continue,
            };
            let ticket = crate::ticket::parse_ticket(&blob);
            inputs.push(LintInput {
                file: entry.to_string_lossy().to_string(),
                ticket,
            });
        }
    }

    check_backlog(&inputs)
}


#[cfg(test)]
mod tests {
    use super::*;

    fn make(id: &str, kind: &str) -> ParsedTicket {
        let k = match kind {
            "epic" => Some(Kind::Epic),
            "story" => Some(Kind::Story),
            "task" => Some(Kind::Task),
            _ => None,
        };
        ParsedTicket {
            id: id.to_string(),
            kind: k,
            status: "todo".to_string(),
            parent: None,
            title: "Test".to_string(),
            depends_on: vec![],
            aliases: vec![],
            links: vec![],
            raw: String::new(),
        }
    }

    #[test]
    fn test_empty_backlog() {
        let report = check_backlog(&[]);
        assert!(report.issues.is_empty());
        assert_eq!(report.error_count, 0);
    }

    /// Helper: create a ticket whose id matches its filename slug and has a
    /// valid parent, so it produces no incidental errors.
    /// Create a minimal parent epic that acts as a valid parent slug
    /// for all the test children. Epics don't need a parent themselves.
    fn parent_ticket() -> LintInput {
        LintInput {
            file: ".plan/epics/01-p.md".to_string(),
            ticket: make("p", "epic"),
        }
    }

    #[test]
    fn test_clean_backlog() {
        let mut e = make("v1", "epic");
        let mut s = make("net", "story");
        s.parent = Some("v1".to_string());
        let mut t = make("proxy", "task");
        t.parent = Some("net".to_string());
        let inputs = vec![
            LintInput { file: ".plan/epics/01-v1.md".to_string(), ticket: e },
            LintInput { file: ".plan/stories/01-net.md".to_string(), ticket: s },
            LintInput { file: ".plan/tasks/01-proxy.md".to_string(), ticket: t },
        ];
        let report = check_backlog(&inputs);
        assert!(report.issues.is_empty(), "issues: {:?}", report.issues);
    }

    #[test]
    fn test_missing_id() {
        let mut t = make("", "task");
        t.parent = Some("p".to_string());
        let inputs = vec![
            LintInput { file: "01-x.md".to_string(), ticket: t },
            parent_ticket(),
        ];
        let r = check_backlog(&inputs);
        // Only one error: missing id. (parent exists, status valid, slug matches)
        assert_eq!(r.error_count, 1);
        assert!(r.issues.iter().any(|i| i.message.contains("missing id")));
    }

    #[test]
    fn test_id_slug_mismatch() {
        let mut t = make("bar", "task");
        t.parent = Some("p".to_string());
        let inputs = vec![
            LintInput { file: "01-foo.md".to_string(), ticket: t },
            parent_ticket(),
        ];
        let r = check_backlog(&inputs);
        assert_eq!(r.error_count, 1);
        assert!(r.issues.iter().any(|i| i.message.contains("does not match filename")));
    }

    #[test]
    fn test_duplicate_slug() {
        // Proper hierarchy: epic  ep → story p → tasks (dupe, dupe)
        let epic = make("ep", "epic");
        let mut story = make("p", "story");
        story.parent = Some("ep".to_string());
        let mut a = make("dupe", "task");
        a.parent = Some("p".to_string());
        let mut b = make("dupe", "task");
        b.parent = Some("p".to_string());
        let inputs = vec![
            LintInput { file: ".plan/epics/01-ep.md".to_string(), ticket: epic },
            LintInput { file: ".plan/tasks/01-dupe.md".to_string(), ticket: a },
            LintInput { file: ".plan/tasks/02-dupe.md".to_string(), ticket: b },
            LintInput { file: ".plan/stories/01-p.md".to_string(), ticket: story },
        ];
        let r = check_backlog(&inputs);
        assert_eq!(r.issues.len(), 1, "only the duplicate error: {:?}", r.issues);
        assert!(r.issues[0].message.contains("duplicate slug"));
    }

    #[test]
    fn test_invalid_status() {
        let mut t = make("s", "task");
        t.parent = Some("p".to_string());
        t.status = "finished".to_string();
        let inputs = vec![
            LintInput { file: "01-s.md".to_string(), ticket: t },
            parent_ticket(),
        ];
        let r = check_backlog(&inputs);
        assert_eq!(r.error_count, 1);
        assert!(r.issues.iter().any(|i| i.message.contains("invalid status")));
    }

    #[test]
    fn test_dangling_parent() {
        let mut t = make("orph", "task");
        t.parent = Some("ghost".to_string());
        let inputs = vec![LintInput { file: "01-orph.md".to_string(), ticket: t }];
        let r = check_backlog(&inputs);
        assert!(r.issues.iter().any(|i| i.message.contains("does not exist")));
    }

    #[test]
    fn test_dangling_dep() {
        let mut t = make("d", "task");
        t.parent = Some("p".to_string());
        t.depends_on = vec!["ghost".to_string()];
        let mut p = make("p", "task");
        p.parent = Some("s".to_string());
        let inputs = vec![
            LintInput { file: "01-d.md".to_string(), ticket: t },
            LintInput { file: "01-p.md".to_string(), ticket: p },
        ];
        let r = check_backlog(&inputs);
        assert!(r.issues.iter().any(|i| i.message.contains("depends_on")));
    }

    #[test]
    fn test_cycle() {
        let mut a = make("a", "task");
        a.parent = Some("s".to_string());
        a.depends_on = vec!["b".to_string()];
        let mut b = make("b", "task");
        b.parent = Some("s".to_string());
        b.depends_on = vec!["a".to_string()];
        let inputs = vec![
            LintInput { file: "01-a.md".to_string(), ticket: a },
            LintInput { file: "01-b.md".to_string(), ticket: b },
        ];
        let r = check_backlog(&inputs);
        assert!(r.issues.iter().any(|i| i.message.contains("cycle")));
    }

    #[test]
    fn test_self_dep_is_not_a_cycle() {
        let mut t = make("self", "task");
        t.parent = Some("s".to_string());
        t.depends_on = vec!["self".to_string()];
        // Also need a parent that exists for parent check to pass
        let mut p = make("s", "story");
        let inputs = vec![
            LintInput { file: "01-self.md".to_string(), ticket: t },
            LintInput { file: "01-s.md".to_string(), ticket: p },
        ];
        let r = check_backlog(&inputs);
        let self_count = r.issues.iter().filter(|i| i.message.contains("depends_on itself")).count();
        let cycle_count = r.issues.iter().filter(|i| i.message.contains("cycle")).count();
        assert_eq!(self_count, 1);
        assert_eq!(cycle_count, 0, "self-dep must not be double-reported as a cycle");
    }

    #[test]
    fn test_render_empty() {
        let r = LintReport { issues: vec![], error_count: 0, warning_count: 0 };
        assert_eq!(render_report(&r), "");
    }

    #[test]
    fn test_render_summary() {
        let r = LintReport {
            issues: vec![LintIssue {
                file: "f.md".to_string(), level: Level::Error, message: "err".to_string(),
            }],
            error_count: 1, warning_count: 0,
        };
        let out = render_report(&r);
        assert!(out.contains("error: f.md: err"));
        assert!(out.contains("lint: 1 error(s), 0 warning(s)"));
    }
}
