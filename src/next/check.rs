//! The integration check: does every ticket's history still support the fold?
//!
//! `new` enforces the identity invariant at creation, against the refs one
//! clone can see. That is all a creation-time check can do -- two clones each
//! creating the same slug both pass legitimately, and their merge is *clean*,
//! because archival deleted the file on one side and a deletion and an addition
//! do not conflict. The invariant only becomes checkable again once the two
//! histories are in one repository, which is here.
//!
//! `docs/semantics.md` section 6, assumption 3 states it once:
//!
//! > every event for a slug descends from exactly one genesis reachable from
//! > the ref set the fold reads
//!
//! and this module is the half of that enforcement `new` cannot do. It reports;
//! it never repairs. Every finding is a history that already exists, and the
//! fixes are history surgery, a schema decision, or a conversation between two
//! people -- none of which a tool should pick on its own.
//!
//! **Nothing here trusts a committer clock.** The reservation asks about
//! presence, and presence is clock-free; ordering is not. Each question below
//! is answered by asking git for reachability, which is what the commit graph
//! actually knows.

use std::collections::{BTreeMap, BTreeSet};

use super::events::{self, Event};
use super::plumbing as git;
use super::schema::Schema;
use super::verb::Ctx;

/// What went wrong with one slug's history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    /// Two or more `new` records. The floor of a bounded walk is whichever one
    /// the backwards scan meets first, so the ticket's state is clock-chosen.
    DuplicateGenesis,
    /// Events with no reachable `new`. The fold answers for them, so the slug
    /// is neither free nor explicable.
    Severed,
    /// Two state-changing declarations the commit graph declines to order.
    /// Date order picks the winner, and skew between two machines flips it.
    Divergent,
    /// The authority rule is deciding: a live branch shadows a trunk-lane
    /// declaration. Correct, deliberate, and worth a human's attention.
    Shadowed,
}

impl Kind {
    fn code(self) -> &'static str {
        match self {
            Kind::DuplicateGenesis => "duplicate-genesis",
            Kind::Severed => "severed",
            Kind::Divergent => "divergent",
            Kind::Shadowed => "shadowed",
        }
    }

    /// Whether a finding of this kind means the repository is broken.
    ///
    /// `Shadowed` is the one that does not: it is the authority rule working
    /// as specified, and reporting it is how the design's promise that a
    /// two-lane clash "should surface to a human" is kept. Exiting non-zero
    /// for it would make the check unusable in the workflow that produces it
    /// on purpose.
    fn is_fault(self) -> bool {
        !matches!(self, Kind::Shadowed)
    }
}

#[derive(Debug, Clone)]
pub struct Finding {
    pub slug: String,
    pub kind: Kind,
    pub detail: String,
}

/// Every slug's events, bucketed, WITHOUT the authority narrowing.
///
/// The check needs the union that `all_by_ticket` narrows away: a shadowed
/// declaration is invisible to the fold by design, and reporting what the fold
/// cannot see is the entire job. Duplicating the walk here rather than adding a
/// flag to `all_by_ticket` keeps the reader's ref set a single expression --
/// the reader must never gain a mode in which it sees more.
fn union_by_ticket(trunk: &str) -> Result<BTreeMap<String, Vec<Event>>, String> {
    let mut refs: Vec<String> = vec![trunk.to_string()];
    refs.extend(git::for_each_ref("refs/heads/plan/")?);
    events::bucket_over(&refs)
}

/// A slug's kind, for every ticket that still has a file on trunk.
///
/// An archived ticket has none, and gets no entry: its branch is gone, so trunk
/// answers for it, which is what a missing entry means downstream.
fn kinds_on_trunk(ctx: &Ctx) -> Result<BTreeMap<String, String>, String> {
    let dir = format!("{}/tickets", ctx.plan_dir);
    let files = crate::git::ls_tree_md(&ctx.trunk, &dir)?;
    let specs: Vec<String> = files.iter().map(|f| format!("{}:{f}", ctx.trunk)).collect();
    let blobs = git::cat_file_batch(&specs)?;

    let mut kinds = BTreeMap::new();
    for (file, blob) in files.iter().zip(blobs) {
        let Some(slug) = std::path::Path::new(file)
            .file_stem()
            .and_then(|s| s.to_str())
        else {
            continue;
        };
        let Some(blob) = blob else { continue };
        // A ticket that will not parse is not this check's business -- board
        // already reports it, and guessing a kind here would invent an
        // authority ref that names nothing.
        if let Ok(t) = super::verb::parse_ticket(slug, &blob) {
            kinds.insert(slug.to_string(), t.kind);
        }
    }
    Ok(kinds)
}

/// Does this event change state under its ticket's kind?
///
/// Exactly the fold's question, asked through the schema, so a verb the kind's
/// machine does not resolve is the identity here as well. Ordering only matters
/// among events that can decide the answer; an `annotate` that two lanes both
/// wrote is not a disagreement about state.
fn declares_state(schema: &Schema, kind: Option<&String>, event: &Event) -> bool {
    let Some(kind) = kind else {
        // No kind means no sub-machine to resolve against. Fall back to "any
        // verb that carries a `to` under some kind", which over-reports rather
        // than under-reports -- the failure this check exists to catch must not
        // be silenced by an unreadable ticket.
        return schema
            .verbs
            .iter()
            .any(|v| v.name == event.verb && v.to.is_some());
    };
    schema
        .verb(&event.verb, kind)
        .is_some_and(|v| v.to.is_some())
}

/// Run every check over the whole backlog.
pub fn run(ctx: &Ctx) -> Result<Vec<Finding>, String> {
    // Absence is not proof in a truncated history, and three of the four
    // findings below are absence claims -- "no second genesis", "no genesis at
    // all", "no ancestor relation". A shallow clone answers all three the same
    // way a complete one does, so the check refuses rather than certifying a
    // repository it cannot see.
    if git::is_shallow()? {
        return Err(
            "cannot check: this is a shallow clone, so a missing record cannot be told from a \
             record the walk never reached. Run `git fetch --unshallow` first"
                .to_string(),
        );
    }

    let kinds = kinds_on_trunk(ctx)?;
    let buckets = union_by_ticket(&ctx.trunk)?;
    let mut findings = Vec::new();

    for (slug, all) in &buckets {
        if slug.is_empty() {
            continue;
        }
        let kind = kinds.get(slug);

        // ---- genesis: exactly one, per assumption 3 ----
        let geneses: Vec<&Event> = all.iter().filter(|e| e.verb == events::GENESIS).collect();
        match geneses.len() {
            1 => {}
            0 => findings.push(Finding {
                slug: slug.clone(),
                kind: Kind::Severed,
                detail: format!(
                    "{} event(s) and no '{}' record: last is '{}' at {}. The fold answers for \
                     these events, so the slug is neither free nor explicable -- the history was \
                     grafted or rewritten above the creation commit",
                    all.len(),
                    events::GENESIS,
                    all[all.len() - 1].verb,
                    short(&all[all.len() - 1].commit),
                ),
            }),
            n => findings.push(Finding {
                slug: slug.clone(),
                kind: Kind::DuplicateGenesis,
                detail: format!(
                    "{n} '{}' records: {}. Two lineages each created this slug and their merge \
                     was clean, because archival deleted the file on one side. A bounded walk \
                     stops at whichever creation it meets first, so the ticket's floor -- and \
                     with it its state -- is decided by a committer clock",
                    events::GENESIS,
                    geneses
                        .iter()
                        .map(|e| short(&e.commit))
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
            }),
        }

        // ---- ordering: is the fold's winner the graph's winner? ----
        let authority = match kind {
            Some(kind) => events::authority(kind, slug, &ctx.trunk)?,
            None => events::Authority::Trunk {
                ref_: ctx.trunk.clone(),
            },
        };

        // What the fold can actually see, in the same one-ref terms it reads.
        let commits: Vec<String> = all.iter().map(|e| e.commit.clone()).collect();
        let unreachable: BTreeSet<String> =
            git::rev_list(&commits, &[authority.ref_().to_string()])?
                .into_iter()
                .collect();

        let hidden: Vec<&Event> = all
            .iter()
            .filter(|e| unreachable.contains(&e.commit) && declares_state(&ctx.schema, kind, e))
            .collect();
        if !hidden.is_empty() && authority.is_branch() {
            findings.push(Finding {
                slug: slug.clone(),
                kind: Kind::Shadowed,
                detail: format!(
                    "{} is authoritative and cannot reach {} state-changing declaration(s) on \
                     another lane: {}. The branch wins by the authority rule, and the trunk lane \
                     is applied when the branch integrates -- unless the two disagree, which is \
                     a decision for the people who made them",
                    authority.ref_(),
                    hidden.len(),
                    hidden
                        .iter()
                        .map(|e| format!("{} at {}", e.verb, short(&e.commit)))
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
            });
        }

        let visible: Vec<&Event> = all
            .iter()
            .filter(|e| !unreachable.contains(&e.commit) && declares_state(&ctx.schema, kind, e))
            .collect();
        // The fold's winner is the last state-changing event in the stream, and
        // the stream is date-ordered. If it descends from every other one, the
        // graph agrees and the clock was never consulted.
        if let Some((winner, rest)) = visible.split_last() {
            if !rest.is_empty() {
                let others: Vec<String> = rest.iter().map(|e| e.commit.clone()).collect();
                let concurrent = git::rev_list(&others, std::slice::from_ref(&winner.commit))?;
                if !concurrent.is_empty() {
                    let named: Vec<String> = rest
                        .iter()
                        .filter(|e| concurrent.contains(&e.commit))
                        .map(|e| format!("{} at {}", e.verb, short(&e.commit)))
                        .collect();
                    findings.push(Finding {
                        slug: slug.clone(),
                        kind: Kind::Divergent,
                        detail: format!(
                            "'{}' at {} is folded last, but does not descend from {}. The commit \
                             graph does not order these, so the state is whichever committer \
                             clock ran later -- and reading the same repository on another \
                             machine can give the other answer",
                            winner.verb,
                            short(&winner.commit),
                            named.join(", "),
                        ),
                    });
                }
            }
        }
    }

    findings.sort_by(|a, b| (a.kind, &a.slug).cmp(&(b.kind, &b.slug)));
    Ok(findings)
}

fn short(commit: &str) -> String {
    commit.chars().take(7).collect()
}

/// The report, and whether anything in it is a fault.
pub fn report(findings: &[Finding]) -> (String, bool) {
    if findings.is_empty() {
        return (
            "ok: every ticket has exactly one reachable creation, and every state its \
             commit graph orders"
                .to_string(),
            false,
        );
    }
    let faults = findings.iter().filter(|f| f.kind.is_fault()).count();
    let mut out = format!("{} finding(s), {faults} of them faults\n", findings.len());
    for f in findings {
        out.push_str(&format!(
            "\n  {} {}\n    {}\n",
            f.kind.code(),
            f.slug,
            f.detail
        ));
    }
    (out, faults > 0)
}
