//! `planr next` -- the 0.4 typed-graph model, behind a subcommand.
//!
//! 0.3's commands are untouched and keep working; everything here is
//! additive, so the two can coexist until a migration path exists. The model
//! is experimental and the schema is deliberately in-tree and unpinned.

pub mod events;
pub mod fold;
pub mod plumbing;
pub mod schema;
pub mod verb;

use schema::Schema;
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
    let path = ctx.ticket_path(slug);
    if plumbing::show(&ctx.trunk, &path).is_ok() {
        return Err(format!("ticket '{slug}' already exists"));
    }
    reserve_slug(ctx, slug, &path)?;

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

    let base = plumbing::rev_parse(&ctx.trunk)?;
    let index = plumbing::ScratchIndex::from_ref(&base)?;
    index.put(&path, &blob)?;
    let tree = index.write_tree()?;
    let commit = plumbing::commit_tree(
        &tree,
        &[&base],
        &format!("plan: new {slug}\n\nPlanr-Verb: new\nPlanr-Ticket: {slug}\n"),
    )?;
    plumbing::update_ref(&ctx.trunk, &commit, &base)?;
    plumbing::sync_path(&ctx.trunk, &commit, &path)?;

    let state = fold::initial_state(&ctx.schema, kind)?;
    Ok(format!(
        "new {kind} '{slug}' at {} ({state})\n  {path}",
        &commit[..7]
    ))
}

/// Refuse a slug that has EVER named a ticket, not merely one that names one
/// now.
///
/// The ticket's file path is the primary key, and the mapping from slug to
/// path is one-to-one and permanent. Archival deletes the file; it does not
/// release the name. `Planr-Ticket: <slug>` is the identity every event is
/// attributed by, so a reused slug makes one identifier name two tickets, and
/// the whole event model comes apart: a re-created ticket folded its dead
/// predecessor's events, `board` reported a ticket seconds old as `abandoned`,
/// and a bounded read and an unbounded one disagreed about the same ticket
/// because they stopped at different `new` commits. It also falsifies
/// `docs/semantics.md` section 6 assumption 3 -- that a ticket's events all
/// descend from its creation commit -- which is what makes that commit a valid
/// floor for the backwards walk. Enforcing uniqueness here is what makes the
/// assumption true rather than hoped for.
///
/// This is the one question in the model that is genuinely path-shaped, so it
/// is the one place git's changed-path filters apply: the walk is
/// `--diff-filter` over a single path, not a trailer scan. The cost lands on
/// `new`, once per ticket, instead of on every state read.
fn reserve_slug(ctx: &Ctx, slug: &str, path: &str) -> Result<(), String> {
    let Some((created, _)) = plumbing::last_touch(&ctx.trunk, path, "A")? else {
        return Ok(());
    };
    let removed = plumbing::last_touch(&ctx.trunk, path, "D")?;
    let fate = match removed {
        Some((commit, subject)) => format!("removed at {commit} ({subject})"),
        // Added and now absent, with no deletion on this ref: the file left
        // trunk some way archival did not, so name what is known and no more.
        None => format!("and {} no longer carries it", ctx.trunk),
    };
    Err(format!(
        "slug '{slug}' has been used: created at {created}, {fate}\n\
         a slug is a ticket's identity in the event log, and archiving does not release it -- \
         a new ticket here would fold the archived one's events into its own state. Choose another slug."
    ))
}

/// The test seam for the differential oracle, and deliberately not a flag.
///
/// The oracle has to run against a real repository, and this crate is a binary
/// with no library target, so the only way in is the process. An env var keeps
/// it out of the CLI surface: there is one state-read mode, `planr next state`
/// has one documented behaviour, and nothing here is reachable by a user who
/// has not gone looking for it. See [`verb::state_of`] for what the oracle is
/// worth -- it is narrower than it looks.
const ORACLE_ENV: &str = "PLANR_NEXT_ORACLE";

/// Fold one ticket's state, reporting what the walk cost.
///
/// Commits scanned is the number the bound exists to hold down, so it is
/// printed rather than asserted: it is `R` in the cost table -- commits since
/// the ticket last moved -- and no longer the length of history. Events folded
/// is what survived the backwards scan, not the ticket's whole life.
pub fn cmd_state(ctx: &Ctx, slug: &str) -> Result<String, String> {
    let bounded = std::env::var_os(ORACLE_ENV).is_none();
    let (state, walk) = verb::state_of(ctx, slug, bounded)?;
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
/// Two git processes for the whole board, regardless of ticket count: one
/// history walk ([`events::all_by_ticket`]) and one `cat-file --batch` for the
/// ticket blobs. Folding per ticket cost a walk each; reading per ticket cost
/// a spawn each, and once the walk was shared the spawns were what remained.
pub fn cmd_board(ctx: &Ctx) -> Result<String, String> {
    let dir = format!("{}/tickets", ctx.plan_dir);
    let files = crate::git::ls_tree_md(&ctx.trunk, &dir)?;
    let events = events::all_by_ticket(&ctx.trunk)?;

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

    let mut rows = Vec::new();
    for (slug, blob) in slugs.iter().zip(blobs) {
        // The kind still has to be read, because it selects the sub-machine
        // the fold runs against -- but it is a tree read, not a history walk,
        // and now not a process either.
        let row = match blob
            .ok_or_else(|| format!("no blob for '{slug}'"))
            .and_then(|b| verb::parse_ticket(slug, &b))
        {
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
