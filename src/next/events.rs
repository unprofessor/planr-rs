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
//! * [`on_ref`] -- the fast path for a single slug. A claimed ticket's events
//!   all live on `plan/<kind>/<slug>`, so the ref name bounds the walk to that
//!   ticket's own short history.
//! * [`scan`] -- the authoritative path. Walks a commit range reading
//!   trailers, which is the only thing that still works once a ticket has been
//!   archived and its file no longer exists in any tree.

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

/// Fast path: every event reachable from `ref_` but not from `exclude`.
///
/// For a claimed ticket this is `plan/<kind>/<slug>` excluding trunk, which
/// bounds the walk to the ticket's own branch.
pub fn on_ref(ref_: &str, exclude: Option<&str>) -> Result<Vec<Event>, String> {
    let format = format!("--format={}", log_format());
    let range = match exclude {
        Some(base) => format!("{base}..{ref_}"),
        None => ref_.to_string(),
    };
    let out = git::log_raw(&[&format, &range])?;
    Ok(parse_log(&out))
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
    let mut events = scan(trunk, Some(slug))?;

    if git::ref_exists(&own) {
        // The branch carries everything since it was cut; trunk carries what
        // came before. Concatenating is correct because the branch's events
        // are strictly newer than the trunk events it descends from.
        let mut branch_events = on_ref(&own, Some(trunk))?;
        branch_events.retain(|e| e.ticket == slug);
        events.append(&mut branch_events);
        return Ok((events, "branch-ref fast path + trunk scan"));
    }

    Ok((events, "trunk trailer scan"))
}
