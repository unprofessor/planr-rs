//! `planr next` -- the 0.4 typed-graph model, behind a subcommand.
//!
//! 0.3's commands are untouched and keep working; everything here is
//! additive, so the two can coexist until a migration path exists. The model
//! is experimental and the schema is deliberately in-tree and unpinned.

pub mod check;
pub mod events;
pub mod fold;
pub mod plumbing;
pub mod schema;
pub mod verb;

use schema::Schema;
use schema::SLUG_MAX;
use verb::Ctx;

pub fn load_ctx(plan_dir: &str, trunk: &str) -> Result<Ctx, String> {
    let schema = Schema::load(std::path::Path::new(plan_dir))?;
    Ok(Ctx {
        plan_dir: plan_dir.to_string(),
        trunk: trunk.to_string(),
        schema,
    })
}

/// `new` is genesis, not a lifecycle mutation -- there is no prior node and no
/// from-transition -- so it stays fixed tooling rather than a verb.
pub fn new_ticket(
    ctx: &Ctx,
    kind: &str,
    slug: &str,
    title: &str,
    parent: Option<&str>,
) -> Result<String, String> {
    if !ctx.schema.kinds.iter().any(|k| k.name == kind) {
        return Err(format!("unknown kind '{kind}'"));
    }
    check_slug(slug)?;
    let path = ctx.ticket_path(slug);

    // The base is read FIRST, and everything below is evaluated against it.
    //
    // Reserving a slug and then reading the tip separately is a TOCTOU: the
    // final compare-and-swap asserts the tip, so a racer whose reservation ran
    // before the winner's ref move but whose tip read ran after it commits on
    // top and its CAS SUCCEEDS. Both processes then report success, one
    // ticket file survives, and two `Planr-Verb: new` records exist for one
    // slug -- exactly the invariant the reservation is here to hold. Reading
    // the base once and asserting THAT base makes the reservation and the
    // write a single atomic step.
    let base = plumbing::rev_parse(&ctx.trunk)?;
    if plumbing::show(&base, &path).is_ok() {
        return Err(format!("ticket '{slug}' already exists"));
    }
    reserve_slug(slug, &base)?;

    let mut fm = format!("kind: {kind}\ntitle: \"{}\"", title.replace('"', "'"));
    if let Some(p) = parent {
        fm.push_str(&format!("\nparent: {p}"));
    }
    let body = ctx
        .schema
        .templates
        .get(kind)
        .map(|t| t.body.clone())
        .unwrap_or_default();
    let blob = format!("---\n{fm}\n---\n\n# {title}\n\n{body}");

    let index = plumbing::ScratchIndex::from_ref(&base)?;
    index.put(&path, &blob)?;
    let tree = index.write_tree()?;
    // The trailer the whole model is attributed by, written through the same
    // constant the walk stops on. Spelling it out here once let the floor and
    // the record that creates it drift apart in silence: a renamed constant
    // would leave the walk stopping on a name nothing writes, and every
    // never-transitioned ticket would walk to the root.
    let genesis = events::GENESIS;
    let commit = plumbing::commit_tree(
        &tree,
        &[&base],
        &format!("plan: {genesis} {slug}\n\nPlanr-Verb: {genesis}\nPlanr-Ticket: {slug}\n"),
    )?;
    // The CAS asserts the base the reservation was evaluated against, so a
    // concurrent creation loses here rather than landing a second genesis.
    plumbing::update_ref(&ctx.trunk, &commit, &base).map_err(|_| {
        format!(
            "cannot create '{slug}': {} moved while the slug was being reserved -- another \
             '{genesis}' or verb landed first, so this one is refused rather than stacked on \
             top of a reservation it did not see. Nothing was created; re-run, and if the slug \
             was taken in the meantime the re-run refuses it by name",
            ctx.trunk
        )
    })?;
    // Past the compare-and-swap the ticket EXISTS: the commit is on trunk and
    // its genesis trailer is the ticket's identity. Reconciling this worktree
    // is a convenience, and a convenience must not be able to report failure
    // for an operation that has already succeeded -- `docs/semantics.md`
    // section 3.1 says the workspace is never history and step 7 cannot fail
    // the step. Propagating this error told the user their `new` had failed
    // when it had not, which is worse than a dirty worktree: it invites a
    // retry that is then correctly refused by name.
    //
    // Syncing BEFORE the ref move is not the alternative it looks like -- it
    // would put the worktree ahead of the ref it is checked out on, which is
    // the same inconsistency a step earlier and with no commit to point at.
    let mut out = format!("new {kind} '{slug}' at {} ({})\n  {path}", &commit[..7], {
        fold::initial_state(&ctx.schema, kind)?
    });
    if let Err(e) = plumbing::sync_path(&ctx.trunk, &commit, &path) {
        out.push_str(&format!(
            "\n  warning: the ticket is committed, but this worktree could not be \
             updated to match: {e}\n  run `git restore --source={} -- {path}` once the \
             cause is cleared",
            &commit[..7]
        ));
    }
    Ok(out)
}

/// A slug has to be exactly what comes back out of its own `Planr-Ticket`
/// trailer.
///
/// That is the invariant; [`schema::SLUG_PATTERN`] is how it is enforced.
/// Nothing checked this before, and the gap was not exotic -- a trailing space
/// from a shell paste or an agent assembling arguments is enough. Git's
/// trailer reader trims, and so does the record parser, so `new task "foo "`
/// wrote `.plan/tickets/foo .md` while declaring `Planr-Ticket: foo`: two
/// files, one identity. `abandon "foo "` then reported `todo -> todo` because
/// it could not see its own effect, `board` printed two rows called `foo`, and
/// the ticket that actually went terminal was the one nobody had touched.
///
/// It also means the reservation cannot be sidestepped by decoration: a slug
/// that trims to a taken one is rejected here, before the walk that compares
/// them.
///
/// **Two obligations, not one.** The pattern covers surviving the trailer: it
/// admits only characters git neither trims nor treats as structure, so a slug
/// reads back byte for byte. It says nothing about being a usable *filename*,
/// and a 253-character slug passed validation, committed its genesis to trunk,
/// and only then failed in `sync_path` with ENAMETOOLONG -- leaving the
/// identity written in history, the slug burned, the working tree permanently
/// dirty, and every subsequent `git clone` unable to check out at all.
/// Recovering from that needs history surgery, which the reservation now
/// refuses to reason about.
///
/// [`schema::SLUG_MAX`] is well under the 255-byte filesystem limit on
/// `<slug>.md`, because the slug is also interpolated into
/// `plan/<kind>/<slug>` ref names and into worktree paths, each sitting inside
/// a directory of unknown depth. Nothing real is at risk of hitting it: it is
/// a guard against a generated or pasted name, not a budget to plan against.
fn check_slug(slug: &str) -> Result<(), String> {
    let pattern = regex::Regex::new(schema::SLUG_PATTERN)
        .map_err(|e| format!("the slug pattern does not compile: {e}"))?;
    if pattern.is_match(slug) {
        if slug.len() > SLUG_MAX {
            return Err(format!(
                "slug is {} characters, over the {SLUG_MAX} allowed: it is also a filename \
                 ('{slug}.md'), a ref name and a worktree path, and a name too long for the \
                 filesystem commits its own genesis before the write fails -- leaving a \
                 repository no clone can check out",
                slug.len()
            ));
        }
        return Ok(());
    }
    Err(format!(
        "invalid slug '{slug}': must match {} -- lowercase letters, digits, '-' and '_', \
         starting with a letter or digit\n\
         the slug is both the ticket's filename and its identity in the event log, and trailers \
         are trimmed when they are read back, so anything else would name one ticket on disk and \
         a different one in history",
        schema::SLUG_PATTERN
    ))
}

/// Refuse a slug that has EVER named a ticket, not merely one that names one
/// now.
///
/// The slug is the ticket's identity and it is never released. Archival
/// deletes the file; the events remain, attributed by `Planr-Ticket: <slug>`,
/// so a reused slug makes one identifier name two tickets and the model comes
/// apart: the re-created ticket folds its dead predecessor's events, `board`
/// reports a ticket seconds old as `abandoned`, and a bounded read and an
/// unbounded one disagree about it because they stop at different `new`
/// commits. Every `from` gate reads the bounded answer, so a terminal ticket
/// becomes re-enterable.
///
/// **Keyed on the trailer stream, not on the file path.** An earlier version
/// asked whether the ticket's path appeared in trunk's history, which is a
/// different question wearing the same clothes -- and every gap between the
/// two was a way to reuse a slug whose events survived: `git mv .plan .planr`
/// (or a `PLANR_DIR` export) moves the path and keeps every trailer, a
/// `filter-branch --index-filter` purge removes the path from history while
/// leaving the commits, and a shallow clone can see neither. Asking the
/// question the fold asks is the only version that cannot drift from it.
///
/// The price is git's changed-path filters, which a trailer scan cannot use:
/// a fresh slug costs a full walk, because proving absence means reaching the
/// root. That lands on `new`, once per ticket, and never on a read.
///
/// `rev` is the tip the caller will compare-and-swap against, not the trunk
/// ref -- see [`new_ticket`] for why those must be the same commit.
fn reserve_slug(slug: &str, rev: &str) -> Result<(), String> {
    // A shallow clone cannot answer this, and it fails in the direction that
    // matters: the walk runs out of history and reports the slug free. A read
    // in a shallow clone is wrong too, but transiently -- deepen the clone and
    // it is right again. A duplicate genesis record is permanent, so this is
    // the one operation that refuses rather than guesses.
    if plumbing::is_shallow()? {
        return Err(format!(
            "cannot create '{slug}': this is a shallow clone, so a slug cannot be shown to be \
             unused -- the walk runs out of history rather than reaching the ticket's creation. \
             Run `git fetch --unshallow` first. (Reads are affected too: a fold that cannot \
             reach a ticket's creation commit reports its initial state.)"
        ));
    }

    match events::lineage(slug, rev)? {
        events::Lineage::Unused => Ok(()),
        events::Lineage::Created { genesis, latest } => {
            let since = if latest.commit == genesis.commit {
                "nothing has been declared about it since".to_string()
            } else {
                format!("last declared '{}' at {}", latest.verb, &latest.commit[..7])
            };
            Err(format!(
                "slug '{slug}' has been used: created at {}, {since}\n\
                 a slug is a ticket's identity in the event log, and archiving does not release \
                 it -- a new ticket here would fold the old one's events into its own state. \
                 Choose another slug.",
                &genesis.commit[..7]
            ))
        }
        // Absence is not proof, the same way it is not in a shallow clone --
        // except that here the walk reached the root and found the evidence
        // rather than running out of history. Reporting this slug as free
        // would hand the new ticket the old one's events.
        events::Lineage::Severed { latest } => Err(format!(
            "slug '{slug}' has events but no reachable creation: last declared '{}' at {}, and \
             no '{}' record for it is reachable\n\
             the history may have been rewritten or grafted, or these events may predate the \
             ticket's creation -- a seeded migration chain does that legitimately. Either way \
             this repository cannot say whether the slug is free, and a ticket created here \
             would fold those events into its own state. Restore the history carrying its \
             creation, or choose another slug.",
            latest.verb,
            &latest.commit[..7],
            events::GENESIS
        )),
    }
}

/// The test seam for the differential oracle, and deliberately not a flag.
///
/// The oracle has to run against a real repository, and this crate is a binary
/// with no library target, so the only way in is the process. See
/// [`verb::state_of`] for what the oracle is worth -- it is narrower than it
/// looks, and it covers `planr next state` alone.
const ORACLE_ENV: &str = "PLANR_NEXT_ORACLE";

/// Whether the caller asked for the unbounded walk.
///
/// The VALUE decides, never mere presence. Presence-testing made
/// `PLANR_NEXT_ORACLE=0` and `=false` turn the oracle ON, so anyone exporting
/// it to disable the thing enabled it -- and an env var is inherited by every
/// child process and CI shell, so one line in a profile would have degraded
/// every state read to a full history walk with nothing but a word in the cost
/// line to show for it. An unrecognized value is an error rather than a shrug,
/// because the whole failure mode here is a setting that does the opposite of
/// what its author intended, silently.
fn oracle_requested() -> Result<bool, String> {
    let Some(raw) = std::env::var_os(ORACLE_ENV) else {
        return Ok(false);
    };
    match raw.to_string_lossy().trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "" | "0" | "false" | "no" | "off" => Ok(false),
        other => Err(format!(
            "{ORACLE_ENV} is set to '{other}', which is not a yes or a no. It is a diagnostic \
             seam that reads state by walking the WHOLE history; set it to 1 to use it, or \
             unset it"
        )),
    }
}

/// Fold one ticket's state, reporting what the walk cost.
///
/// Commits scanned is the number the bound exists to hold down, so it is
/// printed rather than asserted: it is `R` in the cost table -- commits since
/// the ticket last moved -- and no longer the length of history. Events folded
/// is what survived the backwards scan, not the ticket's whole life.
pub fn cmd_state(ctx: &Ctx, slug: &str) -> Result<String, String> {
    let oracle = oracle_requested()?;
    if oracle {
        // Loud, because the cost line's "UNBOUNDED" is too quiet a tell for a
        // setting that can arrive by inheritance from a shell nobody is
        // looking at.
        eprintln!("{ORACLE_ENV} is set: reading state UNBOUNDED, which walks the whole history");
    }
    let (state, walk) = verb::state_of(ctx, slug, !oracle)?;
    Ok(format!(
        "{slug}: {state}\n  {} commit(s) scanned, {} event(s) folded -- {}",
        walk.scanned,
        walk.events.len(),
        walk.how
    ))
}

pub fn cmd_lifecycle(ctx: &Ctx, kind: Option<&str>) -> Result<String, String> {
    let mut out = String::new();
    match kind {
        Some(k) => out.push_str(&fold::render_lifecycle(&ctx.schema, k)?),
        None => {
            for k in &ctx.schema.kinds {
                out.push_str(&fold::render_lifecycle(&ctx.schema, &k.name)?);
                out.push('\n');
            }
            if let Some(unit) = ctx.schema.unit() {
                out.push_str(&format!(
                    "unit: {unit} (derived -- the kind whose verbs create a worktree)\n"
                ));
            }
        }
    }
    Ok(out)
}

/// A minimal board: every live ticket with its folded state.
///
/// A FIXED number of git processes whatever the backlog holds, plus one
/// `rev-list` per CLAIMED ticket: `rev-parse` for the repository root, `ls-tree`
/// and `cat-file --batch` for the ticket files, `for-each-ref` and a `rev-list`
/// resolving the authority rule for every ticket at once, one history walk, one
/// `rev-list` settling ref reachability for the whole unclaimed population, and
/// one `rev-list` per live branch to narrow it. Folding per ticket cost a walk
/// each; reading per ticket cost a spawn each, and once the walk was shared the
/// spawns were what remained.
///
/// The shape is the claim worth making. An exact total has been written down
/// wrong twice, because it depends on which calls short-circuit on an empty
/// backlog; `docs/typed-graph-design.md` carries the measured figures.
///
/// The blobs are read BEFORE the walk, because each ticket's kind names the
/// branch the authority rule asks about. Reading them afterward would leave the
/// walk unable to tell which ref answers for which slug, and board would fold a
/// wider event set than `state` does.
pub fn cmd_board(ctx: &Ctx) -> Result<String, String> {
    let dir = format!("{}/tickets", ctx.plan_dir);
    let files = crate::git::ls_tree_md(&ctx.trunk, &dir)?;

    let slugs: Vec<String> = files
        .iter()
        .filter_map(|f| {
            std::path::Path::new(f)
                .file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string())
        })
        .collect();
    let specs: Vec<String> = files.iter().map(|f| format!("{}:{f}", ctx.trunk)).collect();
    let blobs = plumbing::cat_file_batch(&specs)?;

    // The kind still has to be read, because it selects the sub-machine the
    // fold runs against and the branch the authority rule looks for -- but it
    // is a tree read, not a history walk, and not a process per ticket either.
    let parsed: Vec<(String, Result<verb::Ticket, String>)> = slugs
        .iter()
        .cloned()
        .zip(blobs)
        .map(|(slug, blob)| {
            let ticket = blob
                .ok_or_else(|| format!("no blob for '{slug}'"))
                .and_then(|b| verb::parse_ticket(&slug, &b));
            (slug, ticket)
        })
        .collect();
    let kinds: std::collections::BTreeMap<String, String> = parsed
        .iter()
        .filter_map(|(slug, t)| t.as_ref().ok().map(|t| (slug.clone(), t.kind.clone())))
        .collect();

    let events = events::all_by_ticket(&ctx.trunk, &kinds)?;

    let mut rows = Vec::new();
    for (slug, ticket) in &parsed {
        let row = match ticket {
            Ok(ticket) => {
                let ticket_events = events.get(slug).map(Vec::as_slice).unwrap_or(&[]);
                match fold::fold_state(&ctx.schema, &ticket.kind, ticket_events) {
                    Ok(state) => format!(
                        "  {slug:<24} {:<8} {state:<14} {} event(s)",
                        ticket.kind,
                        ticket_events.len()
                    ),
                    Err(e) => format!("  {slug:<24} !! {e}"),
                }
            }
            Err(e) => format!("  {slug:<24} !! {e}"),
        };
        rows.push(row);
    }

    if rows.is_empty() {
        return Ok("no tickets".to_string());
    }
    Ok(format!("{} ticket(s)\n{}", rows.len(), rows.join("\n")))
}

/// The integration half of the identity invariant -- see [`check`].
///
/// Returns the report and whether it contains a fault, so the caller can exit
/// non-zero. The distinction matters: a shadowed declaration is the authority
/// rule working, and a check that failed on it would fail on every claimed
/// ticket whose leader touched trunk.
pub fn cmd_check(ctx: &Ctx) -> Result<(String, bool), String> {
    let findings = check::run(ctx)?;
    Ok(check::report(&findings))
}
