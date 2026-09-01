//! Board renderer -- pure function that turns structured ticket data into
//! the formatted board view.
//!
//! Port of `skills/planr/src/board.ts`.

use crate::ticket::{Kind, ParsedTicket};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct BranchStatus {
    pub branch: String,
    pub status: String,
    pub slug: String,
}

#[derive(Debug, Clone)]
pub struct BoardInput {
    /// All trunk tickets (epics, stories, tasks) from .plan/.
    pub trunk_tickets: Vec<ParsedTicket>,
    /// In-flight branch statuses from plan/* branches.
    pub branch_statuses: Vec<BranchStatus>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn pad_right(s: &str, width: usize) -> String {
    if s.len() >= width {
        s.to_string()
    } else {
        let mut out = String::with_capacity(width);
        out.push_str(s);
        for _ in 0..(width - s.len()) {
            out.push(' ');
        }
        out
    }
}

/// Build a lookup map: slug -> status (from trunk tickets, all kinds).
fn trunk_status_map(tickets: &[ParsedTicket]) -> std::collections::HashMap<String, String> {
    let mut m = std::collections::HashMap::new();
    for t in tickets {
        m.insert(t.id.clone(), t.status.clone());
    }
    m
}

/// Compute BLOCKED-BY for a task: slugs of unmet depends_on.
fn blocked_by(
    task: &ParsedTicket,
    status_map: &std::collections::HashMap<String, String>,
) -> String {
    if task.depends_on.is_empty() {
        return String::new();
    }
    let unmet: Vec<&str> = task
        .depends_on
        .iter()
        .filter(|dep| status_map.get(dep.as_str()).is_none_or(|s| s != "done"))
        .map(|s| s.as_str())
        .collect();
    unmet.join(" ")
}

// ---------------------------------------------------------------------------
// Section rendering
// ---------------------------------------------------------------------------

/// The statuses a ticket file can legitimately carry. A branch scan can also
/// yield a placeholder like `(no task file)`, which describes the branch
/// rather than the task and so must not stand in for a ticket status.
const KNOWN_STATUSES: [&str; 6] = [
    "todo",
    "in_progress",
    "review",
    "done",
    "blocked",
    "abandoned",
];

/// Marker appended to a status that was read from an in-flight branch rather
/// than from the trunk file the rest of the row describes.
const IN_FLIGHT_MARKER: &str = " *";

/// Status to show for a task, and whether it came from an in-flight branch.
///
/// `claim` flips the status on the worktree branch and leaves trunk alone, so
/// a claimed task reads `todo` on trunk for its whole life. Showing that bare
/// would misreport active work as unstarted; substituting the branch value
/// silently would misreport a branch-local edit as committed. Show the branch
/// value and mark it.
fn task_status_display(
    task: &ParsedTicket,
    in_flight: &std::collections::HashMap<&str, &str>,
) -> (String, bool) {
    match in_flight.get(task.id.as_str()) {
        Some(branch_status) if KNOWN_STATUSES.contains(branch_status) => {
            (format!("{branch_status}{IN_FLIGHT_MARKER}"), true)
        }
        _ => (task.status.clone(), false),
    }
}

fn render_section(
    label: &str,
    tickets: &[&ParsedTicket],
    status_map: &std::collections::HashMap<String, String>,
    in_flight: &std::collections::HashMap<&str, &str>,
    is_tasks: bool,
) -> String {
    if tickets.is_empty() {
        return String::new();
    }

    let mut out = format!("## {label}\n");
    out.push_str(&format!(
        "{} {} {} {} {}\n",
        pad_right("ID", 30),
        pad_right("STATUS", 14),
        pad_right("PARENT", 22),
        pad_right("BLOCKED-BY", 22),
        "TITLE",
    ));

    let mut any_in_flight = false;
    for t in tickets {
        let blocked = if is_tasks {
            blocked_by(t, status_map)
        } else {
            String::new()
        };
        let (status_display, from_branch) = if is_tasks {
            task_status_display(t, in_flight)
        } else {
            (t.status.clone(), false)
        };
        any_in_flight |= from_branch;
        let parent_display = t.parent.as_deref().unwrap_or("-");
        let blocked_display = if blocked.is_empty() {
            " -".to_string()
        } else {
            blocked
        };
        out.push_str(&format!(
            "{} {} {} {} {}\n",
            pad_right(&t.id, 30),
            pad_right(&status_display, 14),
            pad_right(parent_display, 22),
            pad_right(&blocked_display, 22),
            t.title,
        ));
    }
    if any_in_flight {
        out.push_str(
            "* status from an in-flight branch; trunk still records the pre-claim value\n",
        );
    }
    out.push('\n');
    out
}

fn render_in_flight(branches: &[BranchStatus]) -> String {
    if branches.is_empty() {
        return String::new();
    }

    let mut out = "## in flight (worktree branches)\n".to_string();
    out.push_str(&format!(
        "{} {} {}\n",
        pad_right("BRANCH", 30),
        pad_right("STATUS", 14),
        "TASK",
    ));

    for b in branches {
        out.push_str(&format!(
            "{} {} {}\n",
            pad_right(&b.branch, 30),
            pad_right(&b.status, 14),
            b.slug,
        ));
    }
    out.push('\n');
    out
}

fn render_summary(
    trunk_tickets: &[ParsedTicket],
    branches: &[BranchStatus],
    status_map: &std::collections::HashMap<String, String>,
) -> String {
    let in_flight_slugs: std::collections::HashSet<&str> =
        branches.iter().map(|b| b.slug.as_str()).collect();

    let mut t_todo = 0usize;
    let mut t_ip = 0;
    let mut t_review = 0;
    let mut t_done = 0;
    let mut t_blocked = 0;
    let mut t_abandoned = 0;

    for t in trunk_tickets {
        // Skip trunk entry if there's an in-flight branch for this slug (only tasks)
        if t.kind == Some(Kind::Task) && in_flight_slugs.contains(t.id.as_str()) {
            continue;
        }

        // Check if a non-done, non-abandoned task is blocked by unmet deps.
        // Abandoned remains visible as its own terminal outcome even when it
        // has an abandoned dependency.
        if t.kind == Some(Kind::Task) && t.status != "done" && t.status != "abandoned" {
            let unmet = blocked_by(t, status_map);
            if !unmet.is_empty() {
                t_blocked += 1;
                continue;
            }
        }

        match t.status.as_str() {
            "todo" => t_todo += 1,
            "in_progress" => t_ip += 1,
            "review" => t_review += 1,
            "done" => t_done += 1,
            "blocked" => t_blocked += 1,
            "abandoned" => t_abandoned += 1,
            _ => {}
        }
    }

    // Count in-flight branch statuses
    for b in branches {
        match b.status.as_str() {
            "todo" => t_todo += 1,
            "in_progress" => t_ip += 1,
            "review" => t_review += 1,
            "done" => t_done += 1,
            "blocked" => t_blocked += 1,
            "abandoned" => t_abandoned += 1,
            _ => {}
        }
    }

    let total = t_todo + t_ip + t_review + t_done + t_blocked + t_abandoned;

    let mut out = "## summary\n".to_string();
    out.push_str(&format!("{} {}\n", pad_right("STATUS", 12), "COUNT"));
    out.push_str(&format!("{} {}\n", pad_right("total", 12), total));
    out.push_str(&format!("{} {}\n", pad_right("todo", 12), t_todo));
    out.push_str(&format!("{} {}\n", pad_right("in_progress", 12), t_ip));
    out.push_str(&format!("{} {}\n", pad_right("review", 12), t_review));
    out.push_str(&format!("{} {}\n", pad_right("done", 12), t_done));
    out.push_str(&format!("{} {}\n", pad_right("blocked", 12), t_blocked));
    out.push_str(&format!("{} {}\n", pad_right("abandoned", 12), t_abandoned));

    out
}

/// Render the full board view: epics, stories, tasks, in-flight, summary.
/// Pure function -- no I/O.
pub fn render_board(input: &BoardInput) -> String {
    let status_map = trunk_status_map(&input.trunk_tickets);

    let epics: Vec<&ParsedTicket> = input
        .trunk_tickets
        .iter()
        .filter(|t| t.kind == Some(Kind::Epic))
        .collect();
    let stories: Vec<&ParsedTicket> = input
        .trunk_tickets
        .iter()
        .filter(|t| t.kind == Some(Kind::Story))
        .collect();
    let tasks: Vec<&ParsedTicket> = input
        .trunk_tickets
        .iter()
        .filter(|t| t.kind == Some(Kind::Task))
        .collect();

    // slug -> status, for the tasks that have a live worktree branch.
    let in_flight: std::collections::HashMap<&str, &str> = input
        .branch_statuses
        .iter()
        .map(|b| (b.slug.as_str(), b.status.as_str()))
        .collect();

    let mut out = String::new();
    out.push_str(&render_section(
        "epics",
        &epics,
        &status_map,
        &in_flight,
        false,
    ));
    out.push_str(&render_section(
        "stories",
        &stories,
        &status_map,
        &in_flight,
        false,
    ));
    out.push_str(&render_section(
        "tasks",
        &tasks,
        &status_map,
        &in_flight,
        true,
    ));
    out.push_str(&render_in_flight(&input.branch_statuses));
    out.push_str(&render_summary(
        &input.trunk_tickets,
        &input.branch_statuses,
        &status_map,
    ));

    out
}

// ---------------------------------------------------------------------------
// CLI I/O helpers (used by main.rs)
// ---------------------------------------------------------------------------

/// Build the one-line source header shown above the board: where the tickets
/// were read from. `ref_arg` is the positional the user passed; `None` or an
/// empty string means the on-disk working tree.
///
/// Working-tree mode reports HEAD, the current branch (when not detached) in
/// parentheses, and a trailing `dirty` marker when the tree has uncommitted
/// changes. Ref mode reports the requested commit-ish and its resolved id; a
/// dirty working tree is irrelevant there since the board reads committed data.
pub fn source_status_line(ref_arg: Option<&str>) -> String {
    let path = crate::git::show_toplevel().unwrap_or_else(|_| ".".to_string());
    let ref_mode = ref_arg.is_some_and(|r| !r.is_empty());

    if ref_mode {
        let refname = ref_arg.unwrap();
        let short = crate::git::rev_parse_short(refname).unwrap_or_else(|_| refname.to_string());
        // Avoid the redundant "(1337d4d) 1337d4d" when the ref is itself a SHA.
        if refname == short {
            format!("# {path} @ {short}")
        } else {
            format!("# {path} @ {refname} {short}")
        }
    } else {
        let short = crate::git::rev_parse_short("HEAD").unwrap_or_else(|_| "unknown".to_string());
        let dirty = if crate::git::is_dirty().unwrap_or(false) {
            " dirty"
        } else {
            ""
        };
        match crate::git::current_branch() {
            Some(branch) => format!("# {path} @ {short} ({branch}){dirty}"),
            None => format!("# {path} @ {short}{dirty}"),
        }
    }
}

/// Gather trunk tickets from a git ref using the git wrappers.
pub fn read_ref_tickets(ref_: &str, plan_dir: &str) -> Vec<ParsedTicket> {
    let kinds = ["epics", "stories", "tasks"];
    let mut results = Vec::new();

    for kind in &kinds {
        let dir = format!("{plan_dir}/{kind}");
        let files = match crate::git::ls_tree_md(ref_, &dir) {
            Ok(f) => f,
            Err(_) => continue,
        };
        for f in &files {
            if !f.ends_with(".md") {
                continue;
            }
            let blob = match crate::git::show_ref(ref_, f) {
                Ok(b) => b,
                Err(_) => continue,
            };
            let ticket = crate::ticket::parse_ticket(&blob);
            results.push(ticket);
        }
    }
    results
}

/// Gather trunk tickets from the local working tree.
pub fn read_working_tree_tickets(plan_dir: &str) -> Vec<ParsedTicket> {
    let kinds = ["epics", "stories", "tasks"];
    let mut results = Vec::new();

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
            if entry.extension().is_none_or(|e| e != "md") {
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
            results.push(ticket);
        }
    }
    results
}

/// Scan in-flight branches and return their statuses.
pub fn read_in_flight_branches(plan_dir: &str) -> Vec<BranchStatus> {
    let branches = match crate::git::branch_list(Some("plan/*")) {
        Ok(b) => b,
        Err(_) => return vec![],
    };

    let mut results = Vec::new();
    for b in &branches {
        let slug = b.strip_prefix("plan/").unwrap_or(b);
        let files = match crate::git::ls_tree_md(b, &format!("{plan_dir}/tasks")) {
            Ok(f) => f,
            Err(_) => continue,
        };
        // Match /[0-9]+-<slug>.md$
        let re_str = format!(r"/[0-9]+-{}\.md$", regex::escape(slug));
        let re = match regex::Regex::new(&re_str) {
            Ok(r) => r,
            Err(_) => {
                results.push(BranchStatus {
                    branch: b.clone(),
                    status: "(no task file)".to_string(),
                    slug: slug.to_string(),
                });
                continue;
            }
        };
        let task_file = files.iter().find(|f| re.is_match(f));
        match task_file {
            Some(f) => {
                let blob = match crate::git::show_ref(b, f) {
                    Ok(bl) => bl,
                    Err(_) => {
                        results.push(BranchStatus {
                            branch: b.clone(),
                            status: "(unreadable)".to_string(),
                            slug: slug.to_string(),
                        });
                        continue;
                    }
                };
                let ticket = crate::ticket::parse_ticket(&blob);
                results.push(BranchStatus {
                    branch: b.clone(),
                    status: ticket.status,
                    slug: slug.to_string(),
                });
            }
            None => {
                results.push(BranchStatus {
                    branch: b.clone(),
                    status: "(no task file)".to_string(),
                    slug: slug.to_string(),
                });
            }
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(
        id: &str,
        kind: &str,
        parent: Option<&str>,
        status: &str,
        deps: Vec<&str>,
    ) -> ParsedTicket {
        let k = match kind {
            "epic" => Some(Kind::Epic),
            "story" => Some(Kind::Story),
            "task" => Some(Kind::Task),
            _ => None,
        };
        ParsedTicket {
            id: id.to_string(),
            kind: k,
            status: status.to_string(),
            parent: parent.map(|s| s.to_string()),
            title: format!("Ticket {id}"),
            depends_on: deps.into_iter().map(|s| s.to_string()).collect(),
            aliases: vec![],
            links: vec![],
            raw: String::new(),
            frontmatter_error: None,
        }
    }

    #[test]
    fn test_empty_board() {
        let input = BoardInput {
            trunk_tickets: vec![],
            branch_statuses: vec![],
        };
        let out = render_board(&input);
        assert!(out.contains("## summary"));
        assert!(out.contains("total"));
    }

    #[test]
    fn test_sections() {
        let tickets = vec![
            t("v1", "epic", None, "todo", vec![]),
            t("net", "story", Some("v1"), "todo", vec![]),
            t("proxy", "task", Some("net"), "todo", vec![]),
        ];
        let input = BoardInput {
            trunk_tickets: tickets,
            branch_statuses: vec![],
        };
        let out = render_board(&input);
        assert!(out.contains("## epics"));
        assert!(out.contains("## stories"));
        assert!(out.contains("## tasks"));
        assert!(out.contains("v1"));
        assert!(out.contains("net"));
        assert!(out.contains("proxy"));
    }

    #[test]
    fn test_blocked_by_shown() {
        let tickets = vec![
            t("v1", "epic", None, "done", vec![]),
            t("net", "story", Some("v1"), "todo", vec![]),
            t("proxy", "task", Some("net"), "todo", vec!["v1"]),
            t("other", "task", Some("net"), "todo", vec!["v1"]),
        ];
        let input = BoardInput {
            trunk_tickets: tickets,
            branch_statuses: vec![],
        };
        let out = render_board(&input);
        // proxy has no unmet deps (v1 is done), so should show " -" for BLOCKED-BY
        assert!(out.contains(" -"), "done dep should not block: {out}");
    }

    #[test]
    fn test_summary_counts() {
        let tickets = vec![
            t("e", "epic", None, "done", vec![]),
            t("s", "story", Some("e"), "done", vec![]),
            t("a", "task", Some("s"), "done", vec![]),
            t("b", "task", Some("s"), "todo", vec![]),
        ];
        let input = BoardInput {
            trunk_tickets: tickets,
            branch_statuses: vec![],
        };
        let out = render_board(&input);
        // total = 4, done = 3 (epic e + story s + task a), todo = 1 (task b)
        assert!(
            out.lines()
                .any(|l| l.starts_with("total") && l.contains("4")),
            "total=4: {out}"
        );
        assert!(
            out.lines()
                .any(|l| l.starts_with("done") && l.contains("3")),
            "done=3: {out}"
        );
        assert!(
            out.lines()
                .any(|l| l.starts_with("todo") && l.contains("1")),
            "todo=1: {out}"
        );
    }

    #[test]
    fn test_task_status_shows_marked_branch_status() {
        // claim flips status on the branch and leaves trunk at todo. The
        // tasks table must show the live value, marked as branch-sourced.
        let tickets = vec![
            t("proxy", "task", Some("net"), "todo", vec![]),
            t("cache", "task", Some("net"), "todo", vec![]),
        ];
        let branches = vec![BranchStatus {
            branch: "plan/proxy".to_string(),
            status: "in_progress".to_string(),
            slug: "proxy".to_string(),
        }];
        let input = BoardInput {
            trunk_tickets: tickets,
            branch_statuses: branches,
        };
        let out = render_board(&input);

        let proxy_row = out
            .lines()
            .find(|l| l.starts_with("proxy"))
            .expect("proxy row");
        assert!(
            proxy_row.contains("in_progress *"),
            "claimed task should show the marked branch status: {proxy_row}"
        );
        let cache_row = out
            .lines()
            .find(|l| l.starts_with("cache"))
            .expect("cache row");
        assert!(
            cache_row.contains("todo") && !cache_row.contains('*'),
            "unclaimed task must stay unmarked: {cache_row}"
        );
        assert!(
            out.contains("* status from an in-flight branch"),
            "legend missing: {out}"
        );
    }

    #[test]
    fn test_no_marker_legend_without_in_flight_tasks() {
        let tickets = vec![t("proxy", "task", Some("net"), "todo", vec![])];
        let input = BoardInput {
            trunk_tickets: tickets,
            branch_statuses: vec![],
        };
        let out = render_board(&input);
        assert!(
            !out.contains("* status from an in-flight branch"),
            "legend should only appear when a row is marked: {out}"
        );
    }

    #[test]
    fn test_branch_placeholder_does_not_replace_task_status() {
        // A branch with no readable task file yields a placeholder describing
        // the branch, not the task. It belongs in the in-flight section only;
        // leaking it into the STATUS column would invent a status.
        let tickets = vec![t("proxy", "task", Some("net"), "todo", vec![])];
        let branches = vec![BranchStatus {
            branch: "plan/proxy".to_string(),
            status: "(no task file)".to_string(),
            slug: "proxy".to_string(),
        }];
        let input = BoardInput {
            trunk_tickets: tickets,
            branch_statuses: branches,
        };
        let out = render_board(&input);

        let proxy_row = out
            .lines()
            .find(|l| l.starts_with("proxy"))
            .expect("proxy row");
        assert!(
            proxy_row.contains("todo") && !proxy_row.contains("(no task file)"),
            "placeholder must not stand in for a ticket status: {proxy_row}"
        );
        assert!(
            out.contains("## in flight (worktree branches)") && out.contains("(no task file)"),
            "placeholder still belongs in the in-flight section: {out}"
        );
    }

    #[test]
    fn test_epics_and_stories_never_marked() {
        // Only tasks get worktree branches; a slug collision must not mark an
        // epic or story row.
        let tickets = vec![
            t("shared", "epic", None, "todo", vec![]),
            t("shared", "task", None, "todo", vec![]),
        ];
        let branches = vec![BranchStatus {
            branch: "plan/shared".to_string(),
            status: "review".to_string(),
            slug: "shared".to_string(),
        }];
        let input = BoardInput {
            trunk_tickets: tickets,
            branch_statuses: branches,
        };
        let out = render_board(&input);

        let epics_section = out
            .split("## tasks")
            .next()
            .expect("epics section precedes tasks");
        assert!(
            epics_section.contains("## epics") && !epics_section.contains("review"),
            "epic row must not take a branch status: {epics_section}"
        );
        // The task of the same name still gets the marker.
        assert!(
            out.contains("review *"),
            "task row should still be marked: {out}"
        );
    }

    #[test]
    fn test_in_flight_section() {
        let tickets = vec![t("proxy", "task", Some("net"), "in_progress", vec![])];
        let branches = vec![BranchStatus {
            branch: "plan/proxy".to_string(),
            status: "in_progress".to_string(),
            slug: "proxy".to_string(),
        }];
        let input = BoardInput {
            trunk_tickets: tickets,
            branch_statuses: branches,
        };
        let out = render_board(&input);
        assert!(out.contains("## in flight (worktree branches)"));
        assert!(out.contains("plan/proxy"));
    }
}
