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
//! fixes are history surgery, a workflow decision, or a conversation between two
//! people -- none of which a tool should pick on its own.
//!
//! **Nothing here trusts a committer clock.** The reservation asks about
//! presence, and presence is clock-free; ordering is not. Each question below
//! is answered by asking git for reachability, which is what the commit graph
//! actually knows.

use std::collections::{BTreeMap, BTreeSet};

use super::events::{self, Event};
use super::plumbing as git;
use super::verb::Ctx;
use super::workflow::Workflow;

/// What went wrong with one slug's history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FindingKind {
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

impl FindingKind {
    fn code(self) -> &'static str {
        match self {
            FindingKind::DuplicateGenesis => "duplicate-genesis",
            FindingKind::Severed => "severed",
            FindingKind::Divergent => "divergent",
            FindingKind::Shadowed => "shadowed",
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
        !matches!(self, FindingKind::Shadowed)
    }
}

#[derive(Debug, Clone)]
pub struct Finding {
    pub slug: String,
    pub kind: FindingKind,
    pub detail: String,
}

/// Every slug's events, WITHOUT the authority narrowing.
///
/// The check needs the union that `all_by_ticket` narrows away: a shadowed
/// declaration is invisible to the fold by design, and reporting what the fold
/// cannot see is the entire job. Walking here rather than adding a flag to
/// `all_by_ticket` keeps the reader's ref set a single expression -- the reader
/// must never gain a mode in which it sees more.
///
/// Trunk plus the UNINTEGRATED branches is the same commit set as trunk plus
/// every `plan/` ref, since an integrated branch's commits are trunk's.
fn union_by_ticket(
    trunk: &str,
    unintegrated: &BTreeSet<String>,
) -> Result<BTreeMap<String, Vec<Event>>, String> {
    let mut refs: Vec<String> = vec![trunk.to_string()];
    refs.extend(unintegrated.iter().cloned());
    events::bucket_over(&refs)
}

/// Each ticket's kind, for every ticket that still has a file on trunk.
///
/// The kind selects which verbs a ticket's declarations resolve against. An
/// archived ticket has no file and gets no entry, and [`declares`] resolves
/// its declarations without one. Finding a ticket's branch needs only the
/// slug.
fn ticket_kinds(ctx: &Ctx) -> Result<BTreeMap<String, String>, String> {
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
        // already reports it, and guessing a kind here would resolve its
        // verbs against the wrong machine.
        if let Ok(t) = super::verb::parse_ticket(slug, &blob) {
            kinds.insert(slug.to_string(), t.kind);
        }
    }
    Ok(kinds)
}

/// The state this event declares, or `None` if it declares none.
///
/// Exactly the fold's question, asked through the workflow, so a verb the kind's
/// machine does not resolve is the identity here as well. Ordering only matters
/// among events that can decide the answer; an `annotate` that two lanes both
/// wrote is not a disagreement about state.
fn declares(workflow: &Workflow, kind: Option<&String>, event: &Event) -> Option<String> {
    let Some(kind) = kind else {
        // No kind means no sub-machine to resolve against. Fall back to any
        // verb of that name carrying a `to`, which over-reports rather than
        // under-reports -- the failure this check exists to catch must not be
        // silenced by a ticket nothing can type.
        return workflow
            .verbs
            .iter()
            .find(|v| v.name == event.verb && v.to.is_some())
            .and_then(|v| v.to.clone());
    };
    workflow.verb(&event.verb, kind).and_then(|v| v.to.clone())
}

/// Run every check over the whole backlog.
pub fn run(ctx: &Ctx) -> Result<Vec<Finding>, String> {
    // Absence is not proof in a truncated history, and most of the findings
    // below are absence claims -- "no second genesis", "no genesis at all", "no
    // ancestor relation". A shallow clone answers each of them the same way a
    // complete one does, so the check refuses rather than certifying a
    // repository it cannot see.
    if git::is_shallow()? {
        return Err(
            "cannot check: this is a shallow clone, so a missing record cannot be told from a \
             record the walk never reached. Run `git fetch --unshallow` first"
                .to_string(),
        );
    }

    let unintegrated = events::unintegrated(&ctx.trunk)?;
    let buckets = union_by_ticket(&ctx.trunk, &unintegrated)?;
    let kinds = ticket_kinds(ctx)?;
    let mut findings = Vec::new();

    // Which of these commits trunk cannot reach, for the whole backlog in one
    // process. Every ticket without a live branch is answered from this set,
    // so the check costs O(claimed) processes rather than O(tickets) -- the
    // same trick `all_by_ticket` uses, for the same reason.
    let every_commit: Vec<String> = buckets
        .values()
        .flat_map(|evs| evs.iter().map(|e| e.commit.clone()))
        .collect();
    let off_trunk: BTreeSet<String> =
        git::rev_list(&every_commit, std::slice::from_ref(&ctx.trunk))?
            .into_iter()
            .collect();

    for (slug, all) in &buckets {
        let kind = kinds.get(slug);

        // ---- genesis: exactly one, per assumption 3 ----
        let geneses: Vec<&Event> = all.iter().filter(|e| e.verb == events::GENESIS).collect();
        match geneses.len() {
            1 => {}
            0 => findings.push(Finding {
                slug: slug.clone(),
                kind: FindingKind::Severed,
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
                kind: FindingKind::DuplicateGenesis,
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

        // ---- ordering: is the fold's answer the graph's answer? ----

        // The reader's own authority rule, through the reader's own function.
        let ref_ = events::authoritative_ref(&ctx.trunk, &unintegrated, slug);

        // What the fold can actually see, in the same one-ref terms it reads.
        let unreachable: BTreeSet<String> = if ref_ == ctx.trunk {
            all.iter()
                .map(|e| e.commit.clone())
                .filter(|c| off_trunk.contains(c))
                .collect()
        } else {
            let commits: Vec<String> = all.iter().map(|e| e.commit.clone()).collect();
            git::rev_list(&commits, std::slice::from_ref(&ref_))?
                .into_iter()
                .collect()
        };

        // Declarations the fold cannot reach. Reported for a trunk-authoritative
        // ticket too: a declaration sitting on some other ticket's branch is
        // just as invisible, and the guard that once limited this to claimed
        // tickets is what let an archived ticket's surviving branch go
        // unmentioned.
        let hidden: Vec<&Event> = all
            .iter()
            .filter(|e| {
                unreachable.contains(&e.commit) && declares(&ctx.workflow, kind, e).is_some()
            })
            .collect();
        if !hidden.is_empty() {
            findings.push(Finding {
                slug: slug.clone(),
                kind: FindingKind::Shadowed,
                detail: format!(
                    "{ref_} answers for this ticket and cannot reach {} state-changing \
                     declaration(s) on another lane: {}. That is the authority rule working -- \
                     the other lane is applied when the two are integrated -- unless they \
                     disagree, which is a decision for the people who made them",
                    hidden.len(),
                    hidden
                        .iter()
                        .map(|e| format!("{} at {}", e.verb, short(&e.commit)))
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
            });
        }

        // The visible declarations, each with the state it declares.
        //
        // **Concurrency is not the fault; DISAGREEMENT is.** The fold is pure
        // last-`to`-wins, so two incomparable events declaring the same state
        // fold identically in either order -- two clones that both abandoned a
        // ticket are a merge, not an ambiguity. Reporting on ancestry alone
        // faulted those repositories and told them their state was
        // clock-dependent when it demonstrably was not.
        //
        // The set that has to agree is the MAXIMAL one -- see
        // `plumbing::merge_base_independent` for why a walk always lands on one
        // of those. If they all declare the same state the answer is the same
        // whatever the clock says; if two declare different states, a
        // topological order can end on either.
        //
        // **The genesis is one of them.** `for_ticket` stops on it as well as on
        // a state-changing verb, and it denotes `const initial` -- so a `new`
        // record that no declaration descends from competes with that
        // declaration for the floor, and which one the walk lands on is the
        // clock's choice. Leaving it out let a lane cut BEFORE the creation
        // commit read `todo` from `state`, `abandoned` from `board`, and clean
        // from here.
        //
        // This reaches the descent half of assumption 3 only for declarations
        // the answering ref CAN see. The genesis count above runs over the
        // union, so a branch that cannot reach its own ticket's creation still
        // counts one and still reports clean -- narrow claim, deliberately:
        // closing that needs the count moved below `unreachable` and `severed`
        // widened, and no verb can produce the history.
        let declared: Vec<(&Event, String)> = all
            .iter()
            .filter(|e| !unreachable.contains(&e.commit))
            .filter_map(|e| {
                if e.verb == events::GENESIS {
                    let kind = kind?;
                    return super::fold::initial_state(&ctx.workflow, kind)
                        .ok()
                        .map(|to| (e, to));
                }
                declares(&ctx.workflow, kind, e).map(|to| (e, to))
            })
            .collect();
        let maximal: BTreeSet<String> = git::merge_base_independent(
            &declared
                .iter()
                .map(|(e, _)| e.commit.clone())
                .collect::<Vec<_>>(),
        )?
        .into_iter()
        .collect();
        let contenders: Vec<&(&Event, String)> = declared
            .iter()
            .filter(|(e, _)| maximal.contains(&e.commit))
            .collect();
        let outcomes: BTreeSet<&str> = contenders.iter().map(|(_, to)| to.as_str()).collect();
        if outcomes.len() > 1 {
            findings.push(Finding {
                slug: slug.clone(),
                kind: FindingKind::Divergent,
                detail: format!(
                    "{} declarations the commit graph does not order, and they disagree: {}. \
                     The state is whichever committer clock ran later, so reading the same \
                     repository on another machine can give the other answer",
                    contenders.len(),
                    contenders
                        .iter()
                        .map(|(e, to)| format!("{} at {} -> {to}", e.verb, short(&e.commit)))
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
            });
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
