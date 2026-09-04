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

/// Whether a ticket may answer the question "what is the status of <slug>?".
///
/// Two things have to be true, and guarding either one alone leaves the other
/// open. The ticket has to be one some table shows, because a status the
/// reader cannot find a row for is not checkable; and the slug has to be the
/// ticket's own, declared in its frontmatter, rather than one the reader
/// synthesised from the filename. A synthesised slug is a guess about a file
/// that may be sitting next to a real ticket claiming that slug, and the
/// status attached to it is just as likely to be the parser's default `todo`
/// as anything the author wrote.
///
/// Guarding on the kind alone was the mistake this replaces: a file with
/// valid frontmatter, a real `kind` and no `id` passed it, re-entered the
/// slug-to-status namespace under a name it had never claimed, and overwrote
/// a real ticket's `done` -- silently, because every warning was keyed on the
/// kind too.
fn contributes_status(t: &ParsedTicket) -> bool {
    t.kind.is_some() && !t.id_from_filename && !t.id.is_empty()
}

/// Build a lookup map: slug -> status, from the tickets entitled to answer
/// for that slug.
///
/// A ticket that is not entitled contributes nothing, and neither does a slug
/// two tickets both claim: the board cannot tell which of them the dependency
/// column is asking about, and picking the one that happened to be read last
/// is how a broken duplicate of a finished ticket overwrote its `done` -- with
/// every task depending on it showing BLOCKED-BY on the same screen where the
/// ticket itself read `done`. Unknown is the honest answer, and `blocked_by`
/// already treats an unknown dependency as unmet.
fn trunk_status_map(tickets: &[ParsedTicket]) -> std::collections::HashMap<String, String> {
    let mut m: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut contested: std::collections::HashSet<String> = std::collections::HashSet::new();
    for t in tickets.iter().filter(|t| contributes_status(t)) {
        if m.insert(t.id.clone(), t.status.clone()).is_some() {
            contested.insert(t.id.clone());
        }
    }
    for slug in &contested {
        m.remove(slug);
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

/// Stand-ins the branch scan reports when it could not read a ticket at all.
/// They describe the *branch*, not the task, so they must never be displayed
/// or counted as a ticket status. Every other value the scan produces is a
/// status string read verbatim out of a real file -- valid or not.
pub const NO_TASK_FILE: &str = "(no task file)";
pub const UNREADABLE: &str = "(unreadable)";

fn is_placeholder(status: &str) -> bool {
    status == NO_TASK_FILE || status == UNREADABLE
}

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
        Some(branch_status) if crate::ticket::VALID_STATUSES.contains(branch_status) => {
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

/// How a warning names one ticket: its slug, and the file it came from when
/// the reader knows it.
///
/// Two broken files of the same slug otherwise produce two identical lines
/// that tell the reader nothing about either.
fn ticket_name(t: &ParsedTicket) -> String {
    let slug = if t.id.is_empty() {
        "(no id)"
    } else {
        t.id.as_str()
    };
    match &t.source_file {
        Some(file) => format!("'{slug}' ({file})"),
        None => format!("'{slug}'"),
    }
}

/// Warnings about every trunk ticket the board declines to place, to count,
/// or to take a status from.
///
/// There are three ways to fall short of a full row and each has to be said,
/// because the board is silent about all of them otherwise and silence is
/// what makes them dangerous. The three tables filter on `kind`, and the
/// summary counts only what they show, so a ticket whose kind is missing or
/// unrecognized -- including one whose frontmatter failed to parse, which
/// reads as every field absent -- is rendered nowhere and counted nowhere. A
/// ticket that carries no `id` of its own is shown and counted under a slug
/// the reader synthesised from its filename, but it answers for no slug in
/// the dependency column. And a slug two tickets both claim answers for
/// nothing either, because the board cannot tell which of them it belongs to.
///
/// Keying these on the kind alone was how the last two went out in silence: a
/// file with valid frontmatter, a real kind and no `id` passed every filter,
/// shadowed a real ticket's `done`, and warned about nothing at all.
pub fn ticket_warnings(trunk_tickets: &[ParsedTicket]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for t in trunk_tickets {
        let name = ticket_name(t);
        if t.frontmatter_error.is_some() {
            out.push(format!(
                "warning: ticket {name}: frontmatter did not parse, so the board cannot \
                 tell what kind it is -- it is shown in no table, counted nowhere, and \
                 answers for no slug; run `planr lint`"
            ));
        } else if t.kind.is_none() {
            out.push(format!(
                "warning: ticket {name}: no recognized kind (want epic, story, or task) \
                 -- it is shown in no table, counted nowhere, and answers for no slug; \
                 run `planr lint`"
            ));
        } else if t.id_from_filename {
            let slug = &t.id;
            out.push(format!(
                "warning: ticket {name}: no id in its frontmatter, so the board named it \
                 after its file -- it is shown and counted, but it is not the ticket \
                 that answers for '{slug}' in the dependency column; run `planr lint`"
            ));
        }
    }

    // One line per contested slug, not one per ticket: the point is that the
    // board cannot tell these tickets apart, so naming them together is the
    // only description of the problem that is true.
    let mut order: Vec<&str> = Vec::new();
    let mut claimants: std::collections::HashMap<&str, Vec<String>> =
        std::collections::HashMap::new();
    for t in trunk_tickets.iter().filter(|t| contributes_status(t)) {
        let entry = claimants.entry(t.id.as_str()).or_default();
        if entry.is_empty() {
            order.push(t.id.as_str());
        }
        entry.push(match &t.source_file {
            Some(file) => file.clone(),
            None => format!("ticket '{}'", t.id),
        });
    }
    for slug in order {
        let files = &claimants[slug];
        if files.len() > 1 {
            let n = files.len();
            let list = files.join(", ");
            out.push(format!(
                "warning: {n} tickets claim the slug '{slug}' ({list}) -- the board \
                 cannot tell which status belongs to it, so it takes none of them, and \
                 a task that depends on '{slug}' reads as blocked; run `planr lint`"
            ));
        }
    }
    out
}

/// Warnings about `plan/*` branches the board could not take a status from.
///
/// No case is an error, and all of them fall back to the trunk status, but
/// they have different causes and different fixes, so they say different
/// things. A missing ticket detaches the branch from its file; an
/// unrecognized status means the ticket is there and its frontmatter is
/// wrong. Reporting the second as the first sends the reader looking for a
/// file that is sitting right where they left it.
///
/// For the same reason no warning names a cause the board did not establish.
/// "I did not find this slug in the list I was handed" has several causes it
/// cannot tell apart, two of which are not the branch's fault at all: a
/// ticket whose frontmatter did not parse is absent from the list even though
/// the file is committed and present, and a list that came back empty says
/// nothing about any individual slug. Both are excluded here, and each gets a
/// warning that names what actually happened instead -- per branch for the
/// first, once for the second, which is a fact about the read and not about
/// any one branch.
pub fn branch_warnings(branches: &[BranchStatus], trunk_tickets: &[ParsedTicket]) -> Vec<String> {
    let trunk_task_slugs: std::collections::HashSet<&str> = trunk_tickets
        .iter()
        .filter(|t| t.kind == Some(Kind::Task))
        .map(|t| t.id.as_str())
        .collect();
    // Slugs of tickets that are on trunk but that the board could not place,
    // and why. `kind` is what the tables filter on, so a ticket without one
    // is missing from the set above for a reason that has nothing to do with
    // the branch -- whether the kind was swallowed by a failed parse, absent
    // because the file has no frontmatter at all, or simply not a kind planr
    // recognizes. Keying this on the parse error covered one of the three and
    // sent the other two to the arm below, which then said no ticket of that
    // slug was read on the line after the one saying it was.
    let unplaceable_slugs: std::collections::HashMap<&str, bool> = trunk_tickets
        .iter()
        .filter(|t| t.kind.is_none())
        .map(|t| (t.id.as_str(), t.frontmatter_error.is_some()))
        .collect();
    // An empty list is not evidence that a particular ticket is absent from
    // it -- it is evidence that the board read no tickets at all.
    let read_any = !trunk_tickets.is_empty();
    let mut out: Vec<String> = Vec::new();
    // Which is worth saying once. Suppressing the per-branch warning was
    // right and left nothing in its place: the board printed in-flight rows
    // against a total of zero in complete silence, with no hint that the gap
    // came from the read rather than from the branches.
    if !read_any && !branches.is_empty() {
        let n = branches.len();
        let subject = if n == 1 {
            "1 in-flight branch counts".to_string()
        } else {
            format!("{n} in-flight branches count")
        };
        out.push(format!(
            "warning: the board read no tickets at all, so {subject} towards nothing \
             -- check the plan directory and the ref the board read"
        ));
    }
    let per_branch = branches.iter().filter_map(|b| {
        let status = b.status.as_str();
        // A slug that is both a real trunk task and the recovered id of
        // some unreadable file is not detached from anything: the task is
        // there, in the tasks table, counted. Saying it "counts towards
        // nothing" would be false, and it swallowed the warning about the
        // branch that the reader can actually act on.
        if let (false, Some(&unparsed)) = (
            trunk_task_slugs.contains(b.slug.as_str()),
            unplaceable_slugs.get(b.slug.as_str()),
        ) {
            let why = if unparsed {
                "its frontmatter did not parse"
            } else {
                "it carries no recognized kind"
            };
            Some(format!(
                "warning: {}: the ticket for '{}' is present but {why}, so the board \
                 cannot place it -- the branch is listed but counts towards nothing; \
                 run `planr lint`",
                b.branch, b.slug
            ))
        } else if read_any && !trunk_task_slugs.contains(b.slug.as_str()) {
            // The branch reads a status fine, but names a task that is
            // not among the tickets the board read. It counts towards
            // nothing, so without this it would simply be absent from the
            // summary with no explanation.
            Some(format!(
                "warning: {}: no task '{}' among the tickets the board read; \
                 the branch is listed but counts towards nothing; run `planr lint`",
                b.branch, b.slug
            ))
        } else if is_placeholder(status) {
            Some(format!(
                "warning: {}: no readable task file for '{}' -- renamed, removed, or not yet committed",
                b.branch, b.slug
            ))
        } else if !crate::ticket::VALID_STATUSES.contains(&status) {
            Some(format!(
                "warning: {}: task '{}' has an invalid status '{}' -- counting the trunk status; run `planr lint`",
                b.branch, b.slug, status
            ))
        } else {
            None
        }
    });
    out.extend(per_branch);
    out
}

fn render_summary(
    trunk_tickets: &[ParsedTicket],
    branches: &[BranchStatus],
    status_map: &std::collections::HashMap<String, String>,
) -> String {
    // Only a branch that reports a real ticket status takes over the count
    // for its task. A placeholder means the scan learned nothing about the
    // task, so trunk stays the authority -- otherwise the trunk loop skips
    // the ticket, the branch loop declines to count the placeholder, and the
    // task drops out of the totals entirely.
    let in_flight_slugs: std::collections::HashSet<&str> = branches
        .iter()
        .filter(|b| crate::ticket::VALID_STATUSES.contains(&b.status.as_str()))
        .map(|b| b.slug.as_str())
        .collect();

    let mut t_todo = 0usize;
    let mut t_ip = 0;
    let mut t_review = 0;
    let mut t_done = 0;
    let mut t_blocked = 0;
    let mut t_abandoned = 0;

    for t in trunk_tickets {
        // Only a ticket some table shows may be counted. All three tables
        // filter on `kind`, so a ticket whose kind is missing or unrecognized
        // -- which is also how a ticket reads when its frontmatter failed to
        // parse -- is rendered nowhere, and counting it put a row in the
        // summary that the reader cannot find above it. It is not dropped in
        // silence: `ticket_warnings` names the file on stderr.
        let Some(kind) = t.kind.as_ref() else {
            continue;
        };

        // Skip trunk entry if there's an in-flight branch for this slug (only tasks)
        if *kind == Kind::Task && in_flight_slugs.contains(t.id.as_str()) {
            continue;
        }

        // Check if a non-done, non-abandoned task is blocked by unmet deps.
        // Abandoned remains visible as its own terminal outcome even when it
        // has an abandoned dependency.
        if *kind == Kind::Task && t.status != "done" && t.status != "abandoned" {
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

    // Count in-flight branch statuses -- but only for a branch that stands in
    // for a task the ticket tables actually show. A branch whose slug names
    // no trunk task is listed in the in-flight table, but the tasks table is
    // built from trunk, so no row there describes it; counting it anyway
    // would add to `total` and to a status bucket for a ticket the reader
    // cannot find in any ticket table -- the mirror image of the drop this
    // counting was reworked to fix, and just as confusing to read.
    let trunk_task_slugs: std::collections::HashSet<&str> = trunk_tickets
        .iter()
        .filter(|t| t.kind == Some(Kind::Task))
        .map(|t| t.id.as_str())
        .collect();
    for b in branches {
        if !trunk_task_slugs.contains(b.slug.as_str()) {
            continue;
        }
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

/// Name a ticket with no `id` of its own after the file it came from.
///
/// A ticket can arrive anonymous three ways: its frontmatter failed to parse,
/// which swallows every field, `id` included; it has no frontmatter at all; or
/// it has frontmatter that simply omits `id`. All three read the same to every
/// caller downstream, and an anonymous ticket cannot be matched to the
/// `plan/<slug>` branch that names it or named in a warning -- which made the
/// board report a present, committed file as a task that is not there, and
/// then warn about "(no id)" once per file, telling a reader with two broken
/// files nothing about either. Guarding on the parse error covered only the
/// first of the three; the other two stayed anonymous while `lint` next door
/// named their paths.
///
/// The filename is the last readable evidence of the slug, and `lint` already
/// treats it as authoritative (it is an error for `id` to disagree with it).
/// Nothing else is invented, and the recovery is recorded rather than hidden:
/// `id_from_filename` says the slug is the reader's guess and not the
/// ticket's own claim, so nothing keyed on it can shadow a ticket that
/// declared that slug for real. Widening the recovery without recording it
/// re-opened exactly that hole one axis over -- a file with a real `kind` and
/// no `id` took the slug of a finished ticket and overwrote its `done`.
fn name_from_file(mut ticket: ParsedTicket, file: &str) -> ParsedTicket {
    ticket.source_file = Some(file.to_string());
    if ticket.id.is_empty() {
        ticket.id = crate::ticket::slug_from_filename(file);
        ticket.id_from_filename = true;
    }
    ticket
}

/// A backlog read: the tickets planr parsed, and how many ticket files it
/// found to parse.
pub struct TrunkTickets {
    pub tickets: Vec<ParsedTicket>,
    /// Ticket files found, whether or not they could be read.
    ///
    /// A board built from part of a backlog renders the same way one built
    /// from all of it does, and the count of files found is the only thing
    /// that separates them. Keep it covering everything the walk returned;
    /// `tickets.len()` is how many of those planr actually opened.
    pub ticket_files: usize,
}

/// Parse whatever the backlog walk managed to read.
fn parse_backlog(found: Vec<crate::backlog::TicketFile>) -> TrunkTickets {
    let ticket_files = found.len();
    let mut tickets = Vec::new();
    for tf in found {
        // A file the walk could not read has no ticket in it to parse. It
        // still counts as found -- see `unread_warning`.
        let Some(blob) = tf.blob else {
            continue;
        };
        tickets.push(name_from_file(crate::ticket::parse_ticket(&blob), &tf.file));
    }
    TrunkTickets {
        tickets,
        ticket_files,
    }
}

/// Gather trunk tickets from a git ref using the git wrappers.
pub fn read_ref_tickets(ref_: &str, plan_dir: &str) -> TrunkTickets {
    parse_backlog(crate::backlog::read_backlog(
        crate::backlog::Source::Ref(ref_),
        plan_dir,
    ))
}

/// Gather trunk tickets from the local working tree.
pub fn read_working_tree_tickets(plan_dir: &str) -> TrunkTickets {
    parse_backlog(crate::backlog::read_backlog(
        crate::backlog::Source::WorkingTree,
        plan_dir,
    ))
}

/// Say that the board was built from fewer tickets than the ref holds.
///
/// The reader skips a ticket file whose blob it cannot show, so a backlog of
/// ordinary `.md` tickets that planr opened none of renders as an empty
/// board -- the same shape as a backlog that holds nothing. The two counts
/// are what separate them, and the caller cannot see the difference without
/// being told. `lint <ref>` says this already; the board owes its reader the
/// same.
///
/// Say which of the two happened rather than folding them into one message,
/// because they call for different things of the reader: a partial board is
/// right about what it shows and silent about the rest, while an empty one
/// establishes nothing at all.
///
/// Nothing found is not a failed read. A repository that has scaffolded
/// `<plan-dir>/{epics,stories,tasks}` and written no tickets yet finds zero
/// ticket files and fails to read none of them, and warning about that state
/// is the false positive this must not become. Do not widen the guard to
/// cover an empty backlog.
pub fn unread_warning(read: &TrunkTickets, ref_: &str, plan_dir: &str) -> Option<String> {
    let tickets_read = read.tickets.len();
    if tickets_read >= read.ticket_files {
        return None;
    }
    if tickets_read > 0 {
        return Some(format!(
            "warning: planr read {} of the {} ticket file(s) under '{plan_dir}' at \
             '{ref_}' -- what it could not read is missing from this board",
            tickets_read, read.ticket_files
        ));
    }
    Some(format!(
        "warning: planr read none of the {} ticket file(s) under '{plan_dir}' at \
         '{ref_}' -- this board is empty because nothing opened, which is not the \
         same as there being nothing to show",
        read.ticket_files
    ))
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
        // A branch with no tasks directory at all -- one predating the plan
        // directory, or left over from a `--plan-dir` rename -- still gets a
        // row. Dropping it here would erase the branch from the in-flight
        // section, from the counts, and from the warnings (which can only
        // report rows that exist): the same silent-drop that hid every
        // worktree branch from the board in the first place.
        let files = match crate::git::ls_tree_md(b, &format!("{plan_dir}/tasks")) {
            Ok(f) => f,
            Err(_) => {
                results.push(BranchStatus {
                    branch: b.clone(),
                    status: NO_TASK_FILE.to_string(),
                    slug: slug.to_string(),
                });
                continue;
            }
        };
        // Match /[0-9]+-<slug>.md$
        let re_str = format!(r"/[0-9]+-{}\.md$", regex::escape(slug));
        let re = match regex::Regex::new(&re_str) {
            Ok(r) => r,
            Err(_) => {
                results.push(BranchStatus {
                    branch: b.clone(),
                    status: NO_TASK_FILE.to_string(),
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
                            status: UNREADABLE.to_string(),
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
                    status: NO_TASK_FILE.to_string(),
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
            id_from_filename: false,
            source_file: None,
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
    fn test_unreadable_branch_falls_back_to_trunk_status() {
        // A branch that cannot report a status must not consume the task's
        // count: the trunk loop skips such a task and the branch loop will
        // not count a placeholder, so the ticket used to vanish from totals.
        let tickets = vec![
            t("e", "epic", None, "todo", vec![]),
            t("proxy", "task", Some("net"), "in_progress", vec![]),
            t("cache", "task", Some("net"), "todo", vec![]),
        ];
        let branches = vec![BranchStatus {
            branch: "plan/cache".to_string(),
            status: "(no task file)".to_string(),
            slug: "cache".to_string(),
        }];
        let input = BoardInput {
            trunk_tickets: tickets,
            branch_statuses: branches,
        };
        let out = render_board(&input);

        let count = |label: &str| -> Option<String> {
            out.lines()
                .find(|l| l.starts_with(label))
                .and_then(|l| l.split_whitespace().last().map(String::from))
        };
        assert_eq!(
            count("total"),
            Some("3".to_string()),
            "no ticket may be dropped: {out}"
        );
        // cache falls back to its trunk status of todo; proxy keeps in_progress.
        assert_eq!(count("todo"), Some("2".to_string()), "epic + cache: {out}");
        assert_eq!(count("in_progress"), Some("1".to_string()), "proxy: {out}");
    }

    #[test]
    fn test_summary_skips_branch_naming_no_trunk_task() {
        // The tasks table is built from trunk, so a branch whose slug names
        // no task there shows up in no table at all. Counting it anyway put a
        // ticket in the summary that the reader cannot find above it -- the
        // mirror image of the drop the test above covers.
        let tickets = vec![
            t("proxy", "task", Some("net"), "todo", vec![]),
            t("cache", "task", Some("net"), "todo", vec![]),
        ];
        let branches = vec![
            BranchStatus {
                branch: "plan/cache".to_string(),
                status: "review".to_string(),
                slug: "cache".to_string(),
            },
            BranchStatus {
                branch: "plan/ghost".to_string(),
                status: "in_progress".to_string(),
                slug: "ghost".to_string(),
            },
        ];
        let input = BoardInput {
            trunk_tickets: tickets,
            branch_statuses: branches,
        };
        let out = render_board(&input);

        let count = |label: &str| -> Option<String> {
            out.lines()
                .find(|l| l.starts_with(label))
                .and_then(|l| l.split_whitespace().last().map(String::from))
        };
        assert_eq!(
            count("total"),
            Some("2".to_string()),
            "only the tasks the tables show may count: {out}"
        );
        assert_eq!(
            count("in_progress"),
            Some("0".to_string()),
            "a branch with no task on trunk must count towards nothing: {out}"
        );
        // The other branch does name a trunk task, so it still counts -- the
        // skip must not swallow the healthy case.
        assert_eq!(count("review"), Some("1".to_string()), "cache: {out}");
        assert_eq!(count("todo"), Some("1".to_string()), "proxy: {out}");
        assert!(
            out.contains("plan/ghost"),
            "the branch is still listed in flight: {out}"
        );
    }

    #[test]
    fn test_branch_warning_for_slug_with_no_trunk_task() {
        // A branch counted towards nothing would otherwise be absent from the
        // summary with no explanation. A story of the same name does not
        // rescue it: the tasks table is what the branch stands in for.
        let branches = vec![
            BranchStatus {
                branch: "plan/proxy".to_string(),
                status: "in_progress".to_string(),
                slug: "proxy".to_string(),
            },
            BranchStatus {
                branch: "plan/ghost".to_string(),
                status: "in_progress".to_string(),
                slug: "ghost".to_string(),
            },
        ];
        let trunk = vec![
            t("proxy", "task", None, "todo", vec![]),
            t("ghost", "story", None, "todo", vec![]),
        ];
        let warnings = branch_warnings(&branches, &trunk);
        assert_eq!(
            warnings.len(),
            1,
            "only the detached branch warns: {warnings:?}"
        );
        let w = &warnings[0];
        assert!(
            w.contains("plan/ghost") && w.contains("no task 'ghost'"),
            "warning should name the branch and the slug: {w}"
        );
        assert!(
            w.contains("counts towards nothing"),
            "warning should say the branch is uncounted: {w}"
        );
    }

    #[test]
    fn test_branch_warning_distinguishes_invalid_status_from_missing_file() {
        // A typo'd status means the ticket is right where the reader left it,
        // with bad frontmatter. Reporting that as "no readable task file"
        // sends them hunting for a file that is not missing.
        let branches = vec![BranchStatus {
            branch: "plan/proxy".to_string(),
            status: "in-progress".to_string(), // hyphen: lint rejects this
            slug: "proxy".to_string(),
        }];
        let trunk = vec![t("proxy", "task", None, "todo", vec![])];
        let warnings = branch_warnings(&branches, &trunk);
        assert_eq!(
            warnings.len(),
            1,
            "invalid status should warn: {warnings:?}"
        );
        let w = &warnings[0];
        assert!(
            w.contains("invalid status") && w.contains("in-progress"),
            "warning should name the bad status: {w}"
        );
        assert!(
            !w.contains("no readable task file"),
            "warning must not blame a missing file: {w}"
        );
    }

    #[test]
    fn test_branch_warnings_flag_unreadable_branches_only() {
        let branches = vec![
            BranchStatus {
                branch: "plan/proxy".to_string(),
                status: "in_progress".to_string(),
                slug: "proxy".to_string(),
            },
            BranchStatus {
                branch: "plan/cache".to_string(),
                status: "(no task file)".to_string(),
                slug: "cache".to_string(),
            },
            BranchStatus {
                branch: "plan/ghost".to_string(),
                status: "(unreadable)".to_string(),
                slug: "ghost".to_string(),
            },
        ];
        let trunk = vec![
            t("proxy", "task", None, "todo", vec![]),
            t("cache", "task", None, "todo", vec![]),
            t("ghost", "task", None, "todo", vec![]),
        ];
        let warnings = branch_warnings(&branches, &trunk);
        assert_eq!(
            warnings.len(),
            2,
            "only unreadable branches warn: {warnings:?}"
        );
        assert!(
            warnings
                .iter()
                .any(|w| w.contains("plan/cache") && w.contains("cache")),
            "cache warning missing: {warnings:?}"
        );
        assert!(
            warnings.iter().any(|w| w.contains("plan/ghost")),
            "ghost warning missing: {warnings:?}"
        );
        assert!(
            !warnings.iter().any(|w| w.contains("plan/proxy")),
            "a healthy branch must not warn: {warnings:?}"
        );
    }

    /// A ticket whose frontmatter failed to parse: every field reads absent,
    /// so `kind` is `None` and `id` is whatever the reader recovered from the
    /// filename.
    fn unparsed(id: &str) -> ParsedTicket {
        let mut t = t(id, "none", None, "todo", vec![]);
        t.frontmatter_error = Some("mapping values are not allowed".to_string());
        t.id_from_filename = true;
        t
    }

    #[test]
    fn test_branch_warning_does_not_blame_an_unparsed_ticket_on_the_branch() {
        // The ticket file is committed and sitting right there; only its
        // frontmatter is broken, which drops it out of the kind-filtered
        // slug set. Reporting that as "no task of this slug" sends the reader
        // hunting for a file nobody moved.
        let branches = vec![BranchStatus {
            branch: "plan/proxy".to_string(),
            status: "in_progress".to_string(),
            slug: "proxy".to_string(),
        }];
        let trunk = vec![unparsed("proxy")];
        let warnings = branch_warnings(&branches, &trunk);
        assert_eq!(warnings.len(), 1, "one warning: {warnings:?}");
        let w = &warnings[0];
        assert!(
            !w.contains("no task 'proxy'") && !w.contains("not committed"),
            "must not claim the ticket is missing: {w}"
        );
        assert!(
            w.contains("frontmatter did not parse") && w.contains("plan/proxy"),
            "warning should name the real cause: {w}"
        );
    }

    #[test]
    fn test_branch_warnings_blame_the_read_not_the_branch_when_nothing_was_read() {
        // An empty ticket list is evidence about the read, not about any
        // individual slug: every branch would otherwise be reported as
        // detached from a file that may be exactly where it belongs. Saying
        // nothing at all is not the answer either -- the board then prints
        // in-flight rows against a total of zero in silence.
        let branches = vec![BranchStatus {
            branch: "plan/proxy".to_string(),
            status: "in_progress".to_string(),
            slug: "proxy".to_string(),
        }];
        let warnings = branch_warnings(&branches, &[]);
        assert_eq!(
            warnings.len(),
            1,
            "one warning about the read: {warnings:?}"
        );
        let w = &warnings[0];
        assert!(
            w.contains("read no tickets at all") && w.contains("1 in-flight branch counts"),
            "the warning should blame the read and count the branches: {w}"
        );
        assert!(
            !w.contains("no task 'proxy'") && !w.contains("plan/proxy"),
            "nothing is established about 'proxy' in particular: {w}"
        );
    }

    #[test]
    fn test_branch_warnings_count_every_branch_when_nothing_was_read() {
        // The one warning stands in for every branch, so it has to say how
        // many rows the reader is looking at.
        let branches = vec![
            BranchStatus {
                branch: "plan/proxy".to_string(),
                status: "in_progress".to_string(),
                slug: "proxy".to_string(),
            },
            BranchStatus {
                branch: "plan/cache".to_string(),
                status: "review".to_string(),
                slug: "cache".to_string(),
            },
        ];
        let warnings = branch_warnings(&branches, &[]);
        assert!(
            warnings
                .iter()
                .any(|w| w.contains("2 in-flight branches count towards nothing")),
            "the warning should count both branches: {warnings:?}"
        );
    }

    #[test]
    fn test_no_branches_and_no_tickets_warns_about_nothing() {
        // Nothing read and nothing in flight is an empty backlog, not a
        // discrepancy. `main` says the plan directory is missing when it is.
        assert!(branch_warnings(&[], &[]).is_empty());
    }

    #[test]
    fn test_branch_warning_does_not_call_a_real_task_uncounted() {
        // 'proxy' is a task on trunk *and* the recovered id of an unreadable
        // story of the same slug. The task is in the tasks table and in the
        // totals, so "counts towards nothing" would be false -- and it hid
        // the invalid status the reader can actually act on.
        let branches = vec![BranchStatus {
            branch: "plan/proxy".to_string(),
            status: "wip".to_string(), // not a valid status
            slug: "proxy".to_string(),
        }];
        let trunk = vec![t("proxy", "task", None, "todo", vec![]), unparsed("proxy")];
        let warnings = branch_warnings(&branches, &trunk);
        assert_eq!(warnings.len(), 1, "one warning: {warnings:?}");
        let w = &warnings[0];
        assert!(
            w.contains("invalid status") && w.contains("wip"),
            "the actionable warning must survive: {w}"
        );
        assert!(
            !w.contains("counts towards nothing"),
            "the task is shown and counted, so this would be false: {w}"
        );
    }

    /// Every way a ticket can end up carrying a slug it never claimed.
    ///
    /// All three arrive at the same place -- an id synthesised from the
    /// filename, next to a real ticket of that slug -- so a guard that
    /// catches one of them and not the others is not a fix, it is a smaller
    /// version of the same bug. Round 15 guarded the parse error; the file
    /// with a real `kind` and no `id` walked straight through it, and the
    /// tests written for the fix used only the variant the guard caught.
    fn anonymous_variants() -> Vec<(&'static str, &'static str, &'static str)> {
        vec![
            (
                "frontmatter that does not parse",
                "---\nid: dep\nkind: task\ntitle: Dep: broken\n---\n",
                ".plan/tasks/02-dep.md",
            ),
            (
                "no frontmatter at all",
                "# Dep notes\n\nnothing to see\n",
                ".plan/tasks/02-dep.md",
            ),
            (
                "valid frontmatter, a real kind, no id",
                "---\nkind: task\nparent: s1\nstatus: todo\ntitle: Broken dup\n---\n",
                ".plan/tasks/02-dep.md",
            ),
        ]
    }

    #[test]
    fn test_status_map_takes_nothing_from_a_ticket_that_never_named_itself() {
        // A duplicate of a finished ticket arrives carrying the id recovered
        // from its filename and, in two of the three cases, the parser's
        // default `todo`. Letting that overwrite the real `done` turned every
        // dependent task BLOCKED-BY on the same screen where the ticket
        // itself read `done`.
        for (what, blob, file) in anonymous_variants() {
            let tickets = vec![
                t("dep", "task", Some("s1"), "done", vec![]),
                name_from_file(crate::ticket::parse_ticket(blob), file),
                t("user", "task", Some("s1"), "todo", vec!["dep"]),
            ];
            assert_eq!(tickets[1].id, "dep", "{what}: named after its file");
            let input = BoardInput {
                trunk_tickets: tickets,
                branch_statuses: vec![],
            };
            let map = trunk_status_map(&input.trunk_tickets);
            assert_eq!(
                map.get("dep"),
                Some(&"done".to_string()),
                "{what}: the real ticket keeps its status: {map:?}"
            );
            assert_eq!(
                blocked_by(&input.trunk_tickets[2], &map),
                "",
                "{what}: 'dep' is done, so 'user' is not blocked"
            );

            let out = render_board(&input);
            let count = |label: &str| -> Option<String> {
                out.lines()
                    .find(|l| l.starts_with(label))
                    .and_then(|l| l.split_whitespace().last().map(String::from))
            };
            assert_eq!(
                count("blocked"),
                Some("0".to_string()),
                "{what}: nothing is blocked: {out}"
            );

            // And never in silence: the board says what it did with the file,
            // whichever way the file was broken.
            let warnings = ticket_warnings(&input.trunk_tickets);
            assert!(
                warnings.iter().any(|w| w.contains(".plan/tasks/02-dep.md")),
                "{what}: the board must name the file it declined to trust: {warnings:?}"
            );
        }
    }

    #[test]
    fn test_two_tickets_claiming_one_slug_answer_for_neither() {
        // Both declared `id: dep`, so neither id is a guess -- and that is
        // exactly why the board cannot pick between them. Taking the one read
        // last is the same last-write-wins shadowing by another route.
        let mut done = t("dep", "task", Some("s1"), "done", vec![]);
        done.source_file = Some(".plan/tasks/01-dep.md".to_string());
        let mut todo = t("dep", "task", Some("s1"), "todo", vec![]);
        todo.source_file = Some(".plan/tasks/02-dep.md".to_string());
        let tickets = vec![
            done,
            todo,
            t("user", "task", Some("s1"), "todo", vec!["dep"]),
        ];

        let map = trunk_status_map(&tickets);
        assert_eq!(
            map.get("dep"),
            None,
            "neither claimant answers for the contested slug: {map:?}"
        );
        assert_eq!(
            blocked_by(&tickets[2], &map),
            "dep",
            "an unanswered dependency is unmet, which is the honest answer"
        );
        let warnings = ticket_warnings(&tickets);
        assert!(
            warnings.iter().any(|w| {
                w.contains("2 tickets claim the slug 'dep'")
                    && w.contains(".plan/tasks/01-dep.md")
                    && w.contains(".plan/tasks/02-dep.md")
            }),
            "the contested slug must be named, with both files: {warnings:?}"
        );
    }

    #[test]
    fn test_ticket_warnings_speak_for_every_anonymity_path() {
        // The silent path was the one with a real kind: it passed the kind
        // filter, so the kind-keyed warning never fired for it.
        for (what, blob, file) in anonymous_variants() {
            let ticket = name_from_file(crate::ticket::parse_ticket(blob), file);
            let warnings = ticket_warnings(std::slice::from_ref(&ticket));
            assert_eq!(warnings.len(), 1, "{what}: exactly one line: {warnings:?}");
            assert!(
                warnings[0].contains(file) && warnings[0].contains("planr lint"),
                "{what}: name the file and point at lint: {warnings:?}"
            );
        }
    }

    #[test]
    fn test_name_from_file_names_a_ticket_with_no_frontmatter_at_all() {
        // Guarding on the parse error left two other ways to arrive
        // anonymous: no frontmatter at all, and frontmatter that omits `id`.
        // Both then warned as "(no id)", telling a reader with two broken
        // files nothing about either.
        let no_front = name_from_file(
            crate::ticket::parse_ticket("# Notes\n\nnothing to see\n"),
            ".plan/tasks/notes.md",
        );
        assert_eq!(no_front.id, "notes");
        assert!(no_front.id_from_filename, "the slug is the reader's guess");
        let no_id = name_from_file(
            crate::ticket::parse_ticket("---\nkind: task\nstatus: todo\n---\n"),
            ".plan/tasks/02-scratch.md",
        );
        assert_eq!(no_id.id, "scratch");
        assert!(no_id.id_from_filename, "the slug is the reader's guess");
        let declared = name_from_file(
            crate::ticket::parse_ticket("---\nid: real\nkind: task\nstatus: todo\n---\n"),
            ".plan/tasks/03-real.md",
        );
        assert!(
            !declared.id_from_filename,
            "a ticket that named itself is not a guess"
        );

        let warnings = ticket_warnings(&[no_front, no_id, declared]);
        assert_eq!(
            warnings.len(),
            2,
            "one per ticket that arrived anonymous, and no more: {warnings:?}"
        );
        assert!(
            warnings[0].contains("'notes'") && !warnings[0].contains("(no id)"),
            "the warning should name the file it came from: {warnings:?}"
        );
        assert!(
            warnings[1].contains("'scratch'") && warnings[1].contains("no id"),
            "a real kind is no reason to stay quiet about a missing id: {warnings:?}"
        );
    }

    #[test]
    fn test_branch_warning_covers_every_ticket_the_board_could_not_place() {
        // Keying this on the parse error alone contradicted the ticket
        // warning printed on the line above it: the first said a ticket named
        // 'foo' had been read, the second said no such ticket was among the
        // tickets the board read.
        for (what, blob) in [
            ("no frontmatter at all", "# Foo notes\n"),
            (
                "frontmatter that does not parse",
                "---\nid: foo\nkind: task\ntitle: Foo: broken\n---\n",
            ),
            ("an unrecognized kind", "---\nid: foo\nkind: chore\n---\n"),
        ] {
            let trunk = vec![name_from_file(
                crate::ticket::parse_ticket(blob),
                ".plan/tasks/01-foo.md",
            )];
            let branches = vec![BranchStatus {
                branch: "plan/foo".to_string(),
                status: "in_progress".to_string(),
                slug: "foo".to_string(),
            }];
            let warnings = branch_warnings(&branches, &trunk);
            assert_eq!(warnings.len(), 1, "{what}: one warning: {warnings:?}");
            assert!(
                warnings[0].contains("is present but"),
                "{what}: the file is there, so say so: {warnings:?}"
            );
            assert!(
                !warnings[0].contains("no task 'foo' among the tickets the board read"),
                "{what}: the board did read it -- it could not place it: {warnings:?}"
            );
        }
    }

    #[test]
    fn test_ticket_warnings_name_two_broken_files_apart() {
        let a = name_from_file(crate::ticket::parse_ticket("# Notes\n"), "x/notes.md");
        let b = name_from_file(crate::ticket::parse_ticket("# More\n"), "x/scratch.md");
        let warnings = ticket_warnings(&[a, b]);
        assert_eq!(warnings.len(), 2, "one each: {warnings:?}");
        assert_ne!(
            warnings[0], warnings[1],
            "two files must not produce the same line: {warnings:?}"
        );
    }

    #[test]
    fn test_summary_skips_a_ticket_no_table_shows() {
        // All three tables filter on kind, so a ticket with an unrecognized
        // kind is rendered nowhere. Counting it put a row in the summary the
        // reader cannot find above it.
        let tickets = vec![
            t("e", "epic", None, "todo", vec![]),
            t("odd", "none", None, "todo", vec![]),
            unparsed("broken"),
        ];
        let input = BoardInput {
            trunk_tickets: tickets,
            branch_statuses: vec![],
        };
        let out = render_board(&input);

        let count = |label: &str| -> Option<String> {
            out.lines()
                .find(|l| l.starts_with(label))
                .and_then(|l| l.split_whitespace().last().map(String::from))
        };
        assert_eq!(
            count("total"),
            Some("1".to_string()),
            "only the epic is shown, so only the epic counts: {out}"
        );
        assert_eq!(count("todo"), Some("1".to_string()), "the epic: {out}");
        assert!(
            !out.contains("odd") && !out.contains("broken"),
            "neither ticket is rendered in any table: {out}"
        );
    }

    #[test]
    fn test_ticket_warnings_name_what_no_table_shows() {
        // Counted nowhere and shown nowhere is worse than counted oddly, so
        // a ticket the board cannot place has to be named somewhere.
        let tickets = vec![
            t("e", "epic", None, "todo", vec![]),
            t("odd", "none", None, "todo", vec![]),
            unparsed("broken"),
        ];
        let warnings = ticket_warnings(&tickets);
        assert_eq!(
            warnings.len(),
            2,
            "one per unplaceable ticket: {warnings:?}"
        );
        assert!(
            warnings
                .iter()
                .any(|w| w.contains("'odd'") && w.contains("no recognized kind")),
            "the odd kind should be named: {warnings:?}"
        );
        assert!(
            warnings
                .iter()
                .any(|w| w.contains("'broken'") && w.contains("frontmatter did not parse")),
            "the unparsed ticket should be named: {warnings:?}"
        );
        assert!(
            warnings.iter().all(|w| w.contains("planr lint")),
            "each warning should point at the command that explains it: {warnings:?}"
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
