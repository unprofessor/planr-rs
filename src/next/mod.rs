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

/// Fold one ticket's state, reporting which enumeration strategy answered so
/// the cost of each is observable rather than assumed.
pub fn cmd_state(ctx: &Ctx, slug: &str) -> Result<String, String> {
    let (state, how, count) = verb::state_of(ctx, slug)?;
    Ok(format!("{slug}: {state}\n  {count} event(s) via {how}"))
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
