//! Event enumeration -- how a ticket's declarations are found.
//!
//! State is folded from events, and an event is a commit carrying a
//! `Planr-Verb` trailer. The subtlety that drives this module's shape: a verb
//! may declare without changing any bytes (`submit` asserts "ready for
//! review" and writes nothing), and an empty commit touches no path. So
//! `git log -- <ticket path>` silently SKIPS exactly those declarations.
//! Enumeration must therefore go through trailers, never paths.
//!
//! Two ref sets, deliberately both:
//!
//! * a union walk over trunk and the ticket's own ref -- the fast path for a
//!   single slug. It must be ONE walk: ordering has to come from the commit
//!   graph, because trunk can move after a branch is cut and a later trunk
//!   declaration must not be folded before an earlier branch one.
//! * trunk alone, for a ticket with no ref. Trailers are the only thing that
//!   still works once a ticket has been archived and its file no longer exists
//!   in any tree, which is why enumeration never gets to use a pathspec.
//!
//! Both are BOUNDED: the walk stops at the newest event that decides the
//! answer instead of reading history back to the ticket's birth. See
//! [`for_ticket`] for why that is a theorem rather than a heuristic.

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

/// `new` is genesis, not a schema verb -- but it writes `Planr-Verb: new`, so
/// the creation commit is already a record in this stream. That makes it the
/// floor for a ticket that has never transitioned, at no extra cost.
///
/// The name is therefore RESERVED, and reserved in the schema rather than in
/// prose: `Schema::validate` rejects a verb of this name, because a verb
/// called `new` would put a second, meaningless floor into every walk. Worse,
/// it would do so silently -- a stateless verb would report itself as a
/// transition, since `verb::run` reads the before and after states through
/// this same bounded walk.
pub const GENESIS: &str = "new";

fn parse_record(record: &str) -> Option<Event> {
    let record = record.trim_matches(['\n', '\r']);
    if record.is_empty() {
        return None;
    }
    let mut fields = record.split(FS);
    let (Some(commit), Some(verb), Some(ticket)) = (fields.next(), fields.next(), fields.next())
    else {
        return None;
    };
    let verb = verb.trim();
    // A commit with no Planr-Verb trailer is not an event -- ordinary code
    // commits share these branches and must be ignored, not guessed at.
    if verb.is_empty() {
        return None;
    }
    Some(Event {
        commit: commit.trim().to_string(),
        verb: verb.to_string(),
        ticket: ticket.trim().to_string(),
    })
}

fn parse_log(out: &str) -> Vec<Event> {
    let mut events: Vec<Event> = out.split(RS).filter_map(parse_record).collect();
    // git log is newest-first; a fold wants oldest-first.
    events.reverse();
    events
}

/// Commits in a log's output, event-bearing or not -- the cost of the walk.
fn count_records(out: &str) -> usize {
    out.split(RS)
        .filter(|r| !r.trim_matches(['\n', '\r']).is_empty())
        .count()
}

/// One walk's result: the events it found oldest-first, which ref set
/// answered, and how many commits it had to read to answer.
pub struct Walk {
    pub events: Vec<Event>,
    pub how: &'static str,
    pub scanned: usize,
}

/// The refs a ticket's events can live on, plus a label for the pair.
///
/// ONE walk over the union, date-ordered. An earlier version walked the two
/// refs separately and concatenated, on the reasoning that a branch's events
/// are strictly newer than the trunk events it descends from. That is false
/// the moment trunk moves after the branch was cut -- which is exactly what an
/// integration-lane verb on a claimed ticket does, and the authority rule
/// explicitly allows. Concatenating then ordered a later trunk declaration
/// BEFORE an earlier branch one, and the fold silently took the wrong winner.
/// Ordering has to come from the commit graph, never from which ref an event
/// was read through.
fn walk_refs<'a>(trunk: &'a str, own: &'a str) -> (Vec<&'a str>, bool) {
    if git::ref_exists(own) {
        (vec!["--date-order", trunk, own], true)
    } else {
        (vec![trunk], false)
    }
}

/// Every event that can still affect one ticket's folded state, oldest-first.
///
/// **Bounded.** An event denotes `const s` when its verb declares `to: s` and
/// `id` otherwise, and the fold is composition; `const s . f = const s`, so
/// every event before the most recent one carrying a `to` is annihilated. A
/// backwards scan may therefore stop at the first such event. This is the
/// absorption lemma of `docs/semantics.md` section 4, not a heuristic: the
/// result is the same as folding the whole history, and the cost falls from
/// commits-since-the-ticket-existed to commits-since-the-ticket-last-moved.
///
/// Three things the stop rule depends on, each load-bearing:
///
/// * **Stream order is fold order reversed.** `git log` emits newest-first and
///   [`parse_log`] reverses; stopping at the FIRST qualifying record in stream
///   order is stopping at the LAST one in fold order. Reverse either and the
///   walk stops at the wrong end of history.
/// * **The slug filter comes first.** Another ticket's `close` carries a `to`
///   and would otherwise terminate this ticket's walk.
/// * **`terminates` must accept exactly the verbs the fold acts on.** A verb
///   the kind's machine does not resolve is `id` in the fold, so it must not
///   terminate here either, or bounded and unbounded disagree. The rule is
///   passed in rather than derived here to keep this module schema-agnostic.
///
/// Termination also concentrates the ordering assumption (`docs/semantics.md`
/// section 6, assumption 2): committer-date skew across machines already made
/// `--date-order` a guess, but under an unbounded fold a misordering was one
/// wrong event among many, and here it is the whole answer.
pub fn for_ticket(
    slug: &str,
    kind: &str,
    trunk: &str,
    terminates: impl Fn(&str) -> bool,
) -> Result<Walk, String> {
    let own = format!("plan/{kind}/{slug}");
    let format = format!("--format={}", log_format());
    let (refs, union) = walk_refs(trunk, &own);

    let mut args: Vec<&str> = vec![&format];
    args.extend(refs);

    let mut events = Vec::new();
    let scanned = git::log_streaming(&args, RS as u8, |record| {
        let Some(event) = parse_record(record) else {
            return true;
        };
        if event.ticket != slug {
            return true;
        }
        let stop = event.verb == GENESIS || terminates(&event.verb);
        events.push(event);
        !stop
    })?;
    events.reverse();

    Ok(Walk {
        events,
        how: label(union, true),
        scanned,
    })
}

/// The same enumeration with no stop rule, kept as the differential oracle for
/// [`for_ticket`]: the two must fold to the same state on every history.
///
/// **What the oracle is worth, exactly.** It is a separate READ path -- one
/// `git log` read to completion and parsed in one go, rather than
/// [`for_ticket`] with a rule that never fires -- so it catches bugs in the
/// streaming reader's framing and in the stop rule. That is all it catches.
/// Both walks call [`walk_refs`], use the same format string, and consume the
/// same `git log` stream in the same order, so a wrong ref set, a wrong
/// ordering flag, a wrong format or separator, and any [`parse_record`] bug
/// are invisible to it BY CONSTRUCTION: the two would agree on the same wrong
/// answer.
///
/// The sharp case is the ordering assumption. Give a trunk declaration and a
/// branch declaration that are neither ancestor nor descendant a committer
/// date skew, and flipping one date flips the folded state -- with both walks
/// still agreeing, the bounded one after reading a single commit. Catching
/// that needs an oracle that derives order from the commit graph rather than
/// from dates, which is a different mechanism and not this one.
///
/// Reached by tests through `PLANR_NEXT_ORACLE`; it is not a CLI mode.
pub fn for_ticket_unbounded(slug: &str, kind: &str, trunk: &str) -> Result<Walk, String> {
    let own = format!("plan/{kind}/{slug}");
    let format = format!("--format={}", log_format());
    let (refs, union) = walk_refs(trunk, &own);

    let mut args: Vec<&str> = vec![&format];
    args.extend(refs);

    let out = git::log_raw(&args)?;
    let mut events = parse_log(&out);
    events.retain(|e| e.ticket == slug);
    Ok(Walk {
        events,
        how: label(union, false),
        scanned: count_records(&out),
    })
}

/// What the event log knows about a slug.
///
/// Three answers, not two. "Created" and "never seen" are the cases anyone
/// expects; the third is a history in which the slug's events survive and its
/// creation does not, and collapsing it into "never seen" is what let a
/// rewritten history hand out a slug that a fold still answers for.
pub enum Lineage {
    /// No commit reachable from the rev declares anything about this slug.
    Unused,
    /// Created, and whatever was last declared about it.
    Created { genesis: Event, latest: Event },
    /// Events exist and no creation record is reachable. A grafted or
    /// rewritten history, not a name collision -- and NOT the same thing as
    /// unused: the fold answers for these events, so a ticket created here
    /// would inherit them.
    Severed { latest: Event },
}

/// Has any commit reachable from `rev` created this slug -- and if so, what
/// has it done since?
///
/// This is what reserving a slug asks, and it deliberately asks it the way the
/// FOLD asks: over `Planr-Ticket` trailers, not over the ticket's file path.
/// A path-shaped reservation is a different question wearing the same clothes,
/// and every gap between the two is a way to reuse a slug while its events
/// survive: renaming the plan directory (`git mv .plan .planr`, or one
/// `PLANR_DIR` export), purging a path with `filter-branch --index-filter`
/// while every trailer stays, or creating from a shallow clone. Each one
/// restored the divergence the reservation exists to prevent. One question,
/// one mechanism.
///
/// The price is git's changed-path filters: a trailer scan cannot use them, so
/// a slug that has never existed costs a full walk. That is the miss case and
/// it lands on `new`, which happens once per ticket.
///
/// Newest-first, so the walk stops at the genesis record it is looking for and
/// the first record it sees for the slug is the latest declaration.
///
/// Every event for the slug counts, not only the genesis: the fold reacts to
/// ANY event, so a reservation that reacted only to a creation record drifted
/// from it exactly where a history had been rewritten. `git replace --graft`
/// over the creation commit, or a `filter-branch` that drops it, leaves the
/// later trailers reachable and the `new` record not -- and the walk was
/// already holding the evidence when it threw it away.
pub fn lineage(slug: &str, rev: &str) -> Result<Lineage, String> {
    let format = format!("--format={}", log_format());
    let mut latest: Option<Event> = None;
    let mut genesis: Option<Event> = None;

    git::log_streaming(&[&format, rev], RS as u8, |record| {
        let Some(event) = parse_record(record) else {
            return true;
        };
        if event.ticket != slug {
            return true;
        }
        if latest.is_none() {
            latest = Some(event.clone());
        }
        if event.verb == GENESIS {
            genesis = Some(event);
            return false; // the oldest record that can matter; stop here
        }
        true
    })?;

    // A genesis record is itself an event for the slug, so it sets `latest`
    // on the way past: no events means no genesis either.
    Ok(match (latest, genesis) {
        (None, _) => Lineage::Unused,
        (Some(latest), None) => Lineage::Severed { latest },
        (Some(latest), Some(genesis)) => Lineage::Created { genesis, latest },
    })
}

fn label(union: bool, bounded: bool) -> &'static str {
    match (union, bounded) {
        (true, true) => "branch-ref fast path (union walk)",
        (true, false) => "branch-ref union walk, UNBOUNDED",
        (false, true) => "trunk trailer scan",
        (false, false) => "trunk trailer scan, UNBOUNDED",
    }
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
    // `for-each-ref`, not `git branch --list`: branch porcelain prefixes a
    // ref checked out in ANOTHER worktree with "+ ", which is every claimed
    // ticket, and the marker travelled into the revision list as part of the
    // name. Board then failed outright whenever any ticket was claimed.
    let mut refs: Vec<String> = vec![trunk.to_string()];
    if let Ok(listed) = git::for_each_ref("refs/heads/plan/") {
        refs.extend(listed);
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
