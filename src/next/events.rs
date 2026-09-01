//! Event enumeration -- how a ticket's declarations are found.
//!
//! State is folded from events, and an event is a commit carrying a
//! `Planr-Verb` trailer. The subtlety that drives this module's shape: a verb
//! may declare without changing any bytes (`submit` asserts "ready for
//! review" and writes nothing), and an empty commit touches no path. So
//! `git log -- <ticket path>` silently SKIPS exactly those declarations.
//! Enumeration must therefore go through trailers, never paths.
//!
//! Two strategies, deliberately both:
//!
//! * a union walk over trunk and the ticket's own ref -- the fast path for a
//!   single slug, bounded by the ticket's own short history. It must be ONE
//!   walk: ordering has to come from the commit graph, because trunk can move
//!   after a branch is cut and a later trunk declaration must not be folded
//!   before an earlier branch one.
//! * [`scan`] -- the authoritative path. Walks a commit range reading
//!   trailers, which is the only thing that still works once a ticket has been
//!   archived and its file no longer exists in any tree.

use std::collections::BTreeMap;

use super::plumbing as git;

/// One declaration: a commit, the verb it declared, and the ticket it targets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub commit: String,
    pub verb: String,
    pub ticket: String,
}

// Field/record separators chosen to be absent from commit metadata.
const FS: char = '\x1f';
const RS: char = '\x1e';

fn log_format() -> String {
    format!(
        "%H{FS}%(trailers:key=Planr-Verb,valueonly,separator=%x00){FS}%(trailers:key=Planr-Ticket,valueonly,separator=%x00){RS}"
    )
}

fn parse_log(out: &str) -> Vec<Event> {
    let mut events = Vec::new();
    for record in out.split(RS) {
        let record = record.trim_matches(['\n', '\r']);
        if record.is_empty() {
            continue;
        }
        let mut fields = record.split(FS);
        let (Some(commit), Some(verb), Some(ticket)) =
            (fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        let verb = verb.trim();
        let ticket = ticket.trim();
        // A commit with no Planr-Verb trailer is not an event -- ordinary code
        // commits share these branches and must be ignored, not guessed at.
        if verb.is_empty() {
            continue;
        }
        events.push(Event {
            commit: commit.trim().to_string(),
            verb: verb.to_string(),
            ticket: ticket.to_string(),
        });
    }
    // git log is newest-first; a fold wants oldest-first.
    events.reverse();
    events
}

/// Authoritative path: scan a commit range for trailers, optionally filtered
/// to one slug. Works for archived tickets, whose files exist in no tree.
pub fn scan(range: &str, slug: Option<&str>) -> Result<Vec<Event>, String> {
    let format = format!("--format={}", log_format());
    let out = git::log_raw(&[&format, range])?;
    let mut events = parse_log(&out);
    if let Some(slug) = slug {
        events.retain(|e| e.ticket == slug);
    }
    Ok(events)
}

/// Every event for one ticket, using the cheap path when the ticket's own ref
/// exists and falling back to the authoritative scan otherwise.
///
/// Returns the events oldest-first, plus which strategy answered -- the spike
/// reports that so the cost of each is observable rather than assumed.
pub fn for_ticket(
    slug: &str,
    kind: &str,
    trunk: &str,
) -> Result<(Vec<Event>, &'static str), String> {
    let own = format!("plan/{kind}/{slug}");

    if git::ref_exists(&own) {
        // ONE walk over the union of both refs, date-ordered.
        //
        // An earlier version walked them separately and concatenated, on the
        // reasoning that a branch's events are strictly newer than the trunk
        // events it descends from. That is false the moment trunk moves after
        // the branch was cut -- which is exactly what an integration-lane verb
        // on a claimed ticket does, and the authority rule explicitly allows.
        // Concatenating then ordered a later trunk declaration BEFORE an
        // earlier branch one, and the fold silently took the wrong winner.
        // Ordering has to come from the commit graph, never from which ref an
        // event was read through.
        let format = format!("--format={}", log_format());
        let out = git::log_raw(&[&format, "--date-order", trunk, &own])?;
        let mut events = parse_log(&out);
        events.retain(|e| e.ticket == slug);
        return Ok((events, "branch-ref fast path (union walk)"));
    }

    Ok((scan(trunk, Some(slug))?, "trunk trailer scan"))
}

/// Every event in the repository, bucketed by ticket, from a SINGLE walk.
///
/// This is what a board wants. Folding tickets one at a time costs a full
/// history walk each -- O(tickets x commits) -- because an event carries no
/// path to limit the walk by. Walking once and bucketing by `Planr-Ticket`
/// costs O(commits + events), which is the same order as git's own log and
/// the bound archival was supposed to buy.
///
/// The walk covers trunk plus every in-flight `plan/*` ref, so a claimed
/// ticket's branch-lane declarations are included. Git deduplicates commits
/// reachable from several refs, and `--date-order` keeps the ordering from the
/// commit graph rather than from which ref reached a commit first.
pub fn all_by_ticket(trunk: &str) -> Result<BTreeMap<String, Vec<Event>>, String> {
    let mut refs: Vec<String> = vec![trunk.to_string()];
    if let Ok(branches) = crate::git::branch_list(Some("plan/*")) {
        refs.extend(branches);
    }

    let format = format!("--format={}", log_format());
    let mut args: Vec<&str> = vec![&format, "--date-order"];
    args.extend(refs.iter().map(|s| s.as_str()));

    let out = git::log_raw(&args)?;
    let mut buckets: BTreeMap<String, Vec<Event>> = BTreeMap::new();
    for event in parse_log(&out) {
        if event.ticket.is_empty() {
            continue;
        }
        buckets.entry(event.ticket.clone()).or_default().push(event);
    }
    Ok(buckets)
}
