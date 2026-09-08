//! Event enumeration -- how a ticket's declarations are found.
//!
//! State is folded from events, and an event is a commit carrying a
//! `Planr-Verb` trailer. The subtlety that drives this module's shape: a verb
//! may declare without changing any bytes (`submit` asserts "ready for
//! review" and writes nothing), and an empty commit touches no path. So
//! `git log -- <ticket path>` silently SKIPS exactly those declarations.
//! Enumeration must therefore go through trailers, never paths.
//!
//! ONE ref answers for a ticket, chosen by the authority rule -- its own
//! branch while that branch is live and unintegrated, and trunk otherwise. See
//! [`authority`] for why that is a correctness property and not a shortcut.
//! Trailers are the only thing that still works once a ticket has been archived
//! and its file no longer exists in any tree, which is why enumeration never
//! gets to use a pathspec.
//!
//! The walk is BOUNDED: it stops at the newest event that decides the answer
//! instead of reading history back to the ticket's birth. See [`for_ticket`]
//! for why that is a theorem rather than a heuristic.

use std::collections::{BTreeMap, BTreeSet};

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

/// Every walk in this module passes it, and the difference from git's default
/// is the whole stop rule.
///
/// `git log` with no ordering flag emits in PURE committer-date order, which
/// can place a commit before its own ancestors -- a graft, an imported history,
/// or two machines whose clocks disagree all produce that. The backwards scan
/// then meets a ticket's `new` record before the declarations that descend from
/// it, floors there, and reports the initial state for a ticket that has moved
/// several times. `--date-order` keeps committer date as the tiebreak and adds
/// the constraint that decides it: no parent is emitted before its children.
///
/// The bound is a theorem about the event SEQUENCE, so the sequence has to
/// respect the graph before the theorem says anything. Do not drop this flag.
const DATE_ORDER: &str = "--date-order";

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

/// Which single ref answers for a ticket.
///
/// A ticket's own branch while that branch is live and carries commits trunk
/// cannot reach; trunk once it does not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Authority {
    /// `refs/heads/plan/<kind>/<slug>`, live and not yet integrated.
    Branch { ref_: String },
    /// No branch of that name, or one trunk has already integrated.
    Trunk { ref_: String },
}

impl Authority {
    pub fn ref_(&self) -> &str {
        match self {
            Authority::Branch { ref_ } | Authority::Trunk { ref_ } => ref_,
        }
    }
}

/// The ref name the authority rule looks for. One spelling, so nothing can
/// look for a ticket's branch under a name nothing else uses.
pub fn own_ref(kind: &str, slug: &str) -> String {
    format!("refs/heads/plan/{kind}/{slug}")
}

/// Has trunk already taken everything this ref carries?
///
/// The single definition of "integrated", used by every caller of the authority
/// rule. `git for-each-ref --merged` answers the same question in bulk and was
/// the obvious way to make board cheaper; it is deliberately not used, because
/// two implementations of one predicate is how the last several defects in this
/// module were built -- they agree until the day they do not.
pub fn integrated(trunk: &str, ref_: &str) -> Result<bool, String> {
    Ok(git::rev_list(&[ref_.to_string()], &[trunk.to_string()])?.is_empty())
}

/// The authority rule: a claimed unit's branch is authoritative for its
/// lifecycle state until it is integrated.
///
/// **This is what makes a read's answer come from the commit graph rather than
/// from committer clocks.** The previous ref set was the union of trunk and the
/// branch, date-ordered, and a union has no total order to offer: the two lanes
/// are concurrent by construction -- that is the entire point of cutting a
/// branch -- so `--date-order` was arbitrating between a worker's `submit` and
/// a leader's trunk-lane declaration, and skew between two machines' clocks
/// flipped the ticket's state. Under a terminating backwards scan that is not
/// one wrong event among many; it is the whole answer.
///
/// Reading ONE ref does not make the history a chain -- a merge commit puts two
/// incomparable declarations under a single tip -- but it removes the source of
/// incomparability this model manufactures on every claim. What remains is
/// genuinely anomalous, and `planr next check` reports it rather than letting
/// the fold quietly pick a winner.
///
/// The trunk-lane declarations a live branch shadows are not lost, only
/// deferred: every integration verb builds a merge commit that descends from
/// both lanes, so once the branch is integrated the graph orders them and the
/// fold sees all of them. A clash that genuinely disagrees -- a leader
/// abandoning while a worker submits -- is meant to reach a human, and the
/// check is where it does.
///
/// **Fully qualified, deliberately.** `git rev-parse <name>` searches
/// `refs/<name>`, then `refs/tags/<name>`, then `refs/heads/<name>`, so an
/// unqualified `plan/<kind>/<slug>` resolves a *tag* of that name in
/// preference to the branch. That made the reader's ref set larger than the
/// reservation's, which enumerates `refs/heads/plan/` -- so a tag named after
/// a ticket was invisible to the check and live to the read, and once the
/// ticket was claimed the tag permanently shadowed its real branch, freezing
/// the ticket in a state no verb could advance. Naming `refs/heads/` here
/// makes every ref-set computation the same set by construction rather than by
/// agreement, and takes git's resolution order -- which the ref backend may
/// change -- out of the answer entirely.
pub fn authority(kind: &str, slug: &str, trunk: &str) -> Result<Authority, String> {
    let own = own_ref(kind, slug);
    // Unintegrated, not merely present: a branch left standing after its work
    // landed carries nothing trunk does not already have, so letting it stay
    // authoritative would hide trunk-lane declarations forever rather than
    // until integration.
    if git::ref_exists(&own) && !integrated(trunk, &own)? {
        return Ok(Authority::Branch { ref_: own });
    }
    Ok(Authority::Trunk {
        ref_: trunk.to_string(),
    })
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
/// section 6, assumption 2): a misordering under an unbounded fold was one
/// wrong event among many, and here it is the whole answer. [`authority`] is
/// what keeps that assumption from being load-bearing in the ordinary case --
/// the walk reads one ref, so the two lanes a claim creates are no longer
/// asked to be ordered by committer date.
pub fn for_ticket(
    slug: &str,
    kind: &str,
    trunk: &str,
    terminates: impl Fn(&str) -> bool,
) -> Result<Walk, String> {
    let authority = authority(kind, slug, trunk)?;
    let format = format!("--format={}", log_format());
    let args: Vec<&str> = vec![&format, DATE_ORDER, authority.ref_()];

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
        how: label(&authority, true),
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
/// The sharp case is the ordering assumption, and it stays out of reach here:
/// both walks read the same ref through the same date order, so a history whose
/// declarations the commit graph declines to order makes them agree on the same
/// arbitrary winner. That question belongs to a different mechanism -- see
/// [`crate::next::check`], which asks the graph directly.
///
/// Reached by tests through `PLANR_NEXT_ORACLE`; it is not a CLI mode.
pub fn for_ticket_unbounded(slug: &str, kind: &str, trunk: &str) -> Result<Walk, String> {
    let authority = authority(kind, slug, trunk)?;
    let format = format!("--format={}", log_format());
    let args: Vec<&str> = vec![&format, DATE_ORDER, authority.ref_()];

    let out = git::log_raw(&args)?;
    let mut events = parse_log(&out);
    events.retain(|e| e.ticket == slug);
    Ok(Walk {
        events,
        how: label(&authority, false),
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
///
/// **The ref set has to be the fold's, not just the caller's tip.** Reads walk
/// trunk unioned with the ticket's own ref, and `all_by_ticket` walks trunk
/// plus every `plan/*`; a reservation that walked one rev was asking about a
/// strict subset, so a slug could be unused to the check and live to the
/// reader. It needs no rewrite to hit: cut a release branch, create and claim a
/// ticket on the mainline, and `new` against the release branch accepts a slug
/// whose `plan/<kind>/<slug>` ref is sitting right there. Every `plan/*` ref
/// ending in this slug is included rather than only the one for the kind being
/// created, because a stale branch under a DIFFERENT kind collides just as
/// hard and the kind is not knowable from the slug.
pub fn lineage(slug: &str, rev: &str) -> Result<Lineage, String> {
    let format = format!("--format={}", log_format());
    let mut latest: Option<Event> = None;
    let mut genesis: Option<Event> = None;

    // Fatal rather than ignored: a swallowed failure here silently narrows the
    // ref set back to the single rev, which is exactly the bug this widening
    // fixes. Absence must not be mistaken for proof anywhere in this check.
    let suffix = format!("/{slug}");
    let mut refs: Vec<String> = vec![rev.to_string()];
    let listed = git::for_each_ref("refs/heads/plan/")
        .map_err(|e| format!("cannot list 'plan/' refs, so a slug cannot be shown unused: {e}"))?;
    // `for_each_ref` already returns full ref names, so the suffix filter runs
    // against `refs/heads/plan/<kind>/<slug>` and needs no reconstruction.
    refs.extend(listed.into_iter().filter(|r| r.ends_with(&suffix)));
    let mut args: Vec<&str> = vec![&format, DATE_ORDER];
    args.extend(refs.iter().map(String::as_str));

    git::log_streaming(&args, RS as u8, |record| {
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

fn label(authority: &Authority, bounded: bool) -> &'static str {
    match (authority, bounded) {
        (Authority::Branch { .. }, true) => "branch trailer scan (authoritative)",
        (Authority::Branch { .. }, false) => "branch trailer scan (authoritative), UNBOUNDED",
        (Authority::Trunk { .. }, true) => "trunk trailer scan",
        (Authority::Trunk { .. }, false) => "trunk trailer scan, UNBOUNDED",
    }
}

/// Every event reachable from `refs`, bucketed by ticket, oldest-first within
/// each bucket.
///
/// One walk, one date order, one parser -- shared by the board and by the
/// integrity check so that "what events exist" has a single answer and only
/// "which of them count" differs between them.
pub fn bucket_over(refs: &[String]) -> Result<BTreeMap<String, Vec<Event>>, String> {
    let format = format!("--format={}", log_format());
    let mut args: Vec<&str> = vec![&format, DATE_ORDER];
    args.extend(refs.iter().map(String::as_str));

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

/// Every event in the repository, bucketed by ticket, from a SINGLE walk --
/// then narrowed per ticket to what its authoritative ref can reach.
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
///
/// **The narrowing is not an optimization; it is what makes board agree with
/// `state`.** [`for_ticket`] reads exactly one ref, chosen by [`authority`],
/// and a union walk reads more than that -- so without this pass a claimed
/// ticket whose trunk lane also declared would fold a different set of events
/// in the two commands, and the board would report a state no `from` gate
/// would accept. `kinds` supplies each slug's kind because that is what names
/// its branch; a slug the caller cannot type is answered by trunk, which is
/// what [`for_ticket`] does when no branch of that name exists.
///
/// Cost is one extra `rev-list` for the whole trunk-authoritative population
/// and one per live branch -- O(claimed), not O(tickets), and each over a
/// handful of commit ids rather than a history.
pub fn all_by_ticket(
    trunk: &str,
    kinds: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, Vec<Event>>, String> {
    // `for-each-ref`, not `git branch --list`: branch porcelain prefixes a
    // ref checked out in ANOTHER worktree with "+ ", which is every claimed
    // ticket, and the marker travelled into the revision list as part of the
    // name. Board then failed outright whenever any ticket was claimed.
    // Fully qualified, so this is the same ref set the reservation and the
    // single-ticket walk compute rather than merely a set that agrees with
    // them today. The names come from `for_each_ref` already qualified --
    // reconstructing them from a short name is what broke when a tag shadowed
    // a branch and git lengthened the short form to disambiguate it.
    let mut refs: Vec<String> = vec![trunk.to_string()];
    if let Ok(listed) = git::for_each_ref("refs/heads/plan/") {
        refs.extend(listed);
    }

    let mut buckets = bucket_over(&refs)?;

    // The refs that exist, so membership answers "is there a branch" without a
    // process per slug. Everything else about authority goes through
    // `integrated`, which is the one definition of the predicate.
    let live: BTreeSet<String> = refs.iter().skip(1).cloned().collect();

    // Everything the union saw that trunk cannot reach. ONE process settles
    // every trunk-authoritative ticket at once -- including a ticket whose
    // events were declared from some other ticket's branch, which is a slug
    // with no branch of its own and events that trunk has never seen.
    let all: Vec<String> = buckets
        .values()
        .flat_map(|evs| evs.iter().map(|e| e.commit.clone()))
        .collect();
    let off_trunk: BTreeSet<String> = git::rev_list(&all, &[trunk.to_string()])?
        .into_iter()
        .collect();

    for (slug, events) in buckets.iter_mut() {
        let own = kinds.get(slug).map(|kind| own_ref(kind, slug));
        let branch = match own {
            Some(own) if live.contains(&own) && !integrated(trunk, &own)? => Some(own),
            _ => None,
        };
        match branch {
            Some(branch) => {
                let commits: Vec<String> = events.iter().map(|e| e.commit.clone()).collect();
                let drop: BTreeSet<String> =
                    git::rev_list(&commits, &[branch])?.into_iter().collect();
                events.retain(|e| !drop.contains(&e.commit));
            }
            None => events.retain(|e| !off_trunk.contains(&e.commit)),
        }
    }
    Ok(buckets)
}
