//! The verb runner: `require` -> build a tree from BASE -> move the TARGET ref.
//!
//! There is no ordering key and no ordered step list, because content steps
//! and ref movement are different phases rather than peers. That is the whole
//! reason `claim` and `close` stopped needing opposite orders.

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::parse::{extract_section, parse_frontmatter, split_frontmatter};

use super::events;
use super::fold;
use super::plumbing as git;
use super::schema::{BareContent, Base, Content, Effect, Schema, Verb, WorktreeAction};

pub struct Ctx {
    pub plan_dir: String,
    pub trunk: String,
    pub schema: Schema,
}

impl Ctx {
    pub fn ticket_path(&self, slug: &str) -> String {
        format!("{}/tickets/{slug}.md", self.plan_dir)
    }

    /// The ticket's own ref, **fully qualified**.
    ///
    /// `git rev-parse <name>` searches `refs/<name>`, then `refs/tags/<name>`,
    /// then `refs/heads/<name>`, so the short form resolves a *tag* of the same
    /// name in preference to the branch. A tag named after a ticket then
    /// shadowed its branch everywhere a ref was resolved: the verb runner built
    /// on the tag's tree, so `submit` could not see a `## Validation` section
    /// the branch demonstrably had, and the ticket froze in a state no verb
    /// could advance. Naming `refs/heads/` removes git's resolution order --
    /// which the ref backend may change -- from every answer.
    ///
    /// Use [`Ctx::own_ref_short`] for anything a human reads, and for
    /// `git worktree add`, which wants a branch name rather than a ref path.
    pub fn own_ref(&self, kind: &str, slug: &str) -> String {
        format!("refs/heads/plan/{kind}/{slug}")
    }

    /// The same ref as a branch name, for display and for `worktree add`.
    pub fn own_ref_short(&self, kind: &str, slug: &str) -> String {
        format!("plan/{kind}/{slug}")
    }
}

/// A ticket as it exists at some ref.
pub struct Ticket {
    pub slug: String,
    pub kind: String,
    pub fm: BTreeMap<String, String>,
    pub deps: Vec<String>,
    pub body: String,
}

pub fn read_ticket(ctx: &Ctx, ref_: &str, slug: &str) -> Result<Ticket, String> {
    let blob = git::show(ref_, &ctx.ticket_path(slug))
        .map_err(|_| format!("no ticket '{slug}' at {ref_}"))?;
    parse_ticket(slug, &blob)
}

pub fn parse_ticket(slug: &str, blob: &str) -> Result<Ticket, String> {
    let split = split_frontmatter(blob);
    let value = parse_frontmatter(&split.fm)
        .map_err(|e| format!("ticket '{slug}' has invalid frontmatter: {e}"))?
        .ok_or_else(|| format!("ticket '{slug}' has no frontmatter"))?;

    let mut fm = BTreeMap::new();
    let mut deps = Vec::new();
    if let Some(map) = value.as_mapping() {
        for (k, v) in map {
            let Some(key) = k.as_str() else { continue };
            if key == "depends_on" {
                if let Some(seq) = v.as_sequence() {
                    deps = seq
                        .iter()
                        .filter_map(|d| d.as_str().map(|s| s.to_string()))
                        .collect();
                }
                continue;
            }
            if let Some(s) = v.as_str() {
                fm.insert(key.to_string(), s.to_string());
            }
        }
    }

    // Frontmatter carries only what git cannot derive. A stored `status` is
    // the one thing the model actively forbids, so say so rather than ignore
    // it -- a half-migrated backlog should fail loudly.
    if fm.contains_key("status") {
        return Err(format!(
            "ticket '{slug}' carries a 'status' field. State is derived from commit events in this model; remove the field (see `planr next state {slug}`)"
        ));
    }

    let kind = fm
        .get("kind")
        .ok_or_else(|| format!("ticket '{slug}' has no 'kind'"))?
        .clone();

    Ok(Ticket {
        slug: slug.to_string(),
        kind,
        fm,
        deps,
        body: split.body,
    })
}

/// When the backwards scan may stop, expressed over verb NAMES because
/// `events` is schema-agnostic and must stay that way.
///
/// The rule has to accept exactly the verbs [`fold::fold_state`] acts on: it
/// skips any `(name, kind)` the schema does not resolve, so those denote `id`
/// and cannot end the scan. A looser rule here stops on an event the fold then
/// ignores, and the bounded walk answers with a state the unbounded one does
/// not.
fn terminator<'a>(schema: &'a Schema, kind: &'a str) -> impl Fn(&str) -> bool + 'a {
    move |name| schema.verb(name, kind).is_some_and(|v| v.to.is_some())
}

/// Current state of a ticket, with the walk that produced it -- which ref set
/// answered and how many commits it read.
///
/// `bounded` is false only for the differential oracle: the unbounded walk is
/// kept reachable so the bound can be checked against it rather than trusted.
/// It covers framing and the stop rule and nothing else -- see
/// [`events::for_ticket_unbounded`] for what it cannot see.
pub fn state_of(ctx: &Ctx, slug: &str, bounded: bool) -> Result<(String, events::Walk), String> {
    let ticket = read_ticket_or_archived(ctx, slug)?;
    let walk = if bounded {
        events::for_ticket(
            slug,
            &ticket.kind,
            &ctx.trunk,
            terminator(&ctx.schema, &ticket.kind),
        )?
    } else {
        events::for_ticket_unbounded(slug, &ticket.kind, &ctx.trunk)?
    };
    let state = fold::fold_state(&ctx.schema, &ticket.kind, &walk.events)?;
    Ok((state, walk))
}

/// Read a ticket from trunk, falling back to the last commit that still had
/// it.
///
/// Folding needs the KIND, because the kind selects the sub-machine -- and the
/// kind lives in the file that `archive` deletes. Enumeration survives
/// archival (trailers are in the messages), so without this the events are
/// still there but nothing can interpret them.
///
/// The pathspec is safe here in a way it is NOT for enumeration: an archived
/// ticket's file was demonstrably created and deleted, so it touches the path.
/// The lookup only runs on the miss, so a live ticket pays nothing.
fn read_ticket_or_archived(ctx: &Ctx, slug: &str) -> Result<Ticket, String> {
    let path = ctx.ticket_path(slug);
    // Present-but-invalid is NOT a miss: a ticket carrying a stored `status`
    // must report that, not be silently searched for in history and then
    // reported as never having existed.
    if let Ok(blob) = git::show(&ctx.trunk, &path) {
        return parse_ticket(slug, &blob);
    }
    let removed = git::log_raw(&["-1", "--format=%H", &ctx.trunk, "--", &path])
        .ok()
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| format!("no ticket '{slug}', now or in history"))?;
    let blob = git::show(&format!("{}~1", removed.trim()), &path)
        .map_err(|_| format!("no ticket '{slug}', now or in history"))?;
    parse_ticket(slug, &blob)
}

/// A gate's view of state. Always bounded, and deliberately without the
/// oracle's escape hatch: `state_of` can be asked for the unbounded walk, this
/// cannot, so every `from` check and every `require` in the runner reads the
/// bounded answer and only the bounded answer. The differential proves the two
/// agree; it does not make the gate switchable.
///
/// The walk is a SUFFIX of the ticket's history --
/// everything before its last transition is annihilated -- which the fold is
/// fine with because it seeds from `initial_state` and the suffix begins with
/// a `const`. Any caller that wanted the full event list would not be.
fn state_at(ctx: &Ctx, slug: &str, kind: &str) -> Result<String, String> {
    let walk = events::for_ticket(slug, kind, &ctx.trunk, terminator(&ctx.schema, kind))?;
    fold::fold_state(&ctx.schema, kind, &walk.events)
}

/// Evaluate a verb's precondition. Structural only -- never a judgement about
/// whether the work is any good.
fn check_require(ctx: &Ctx, verb: &Verb, ticket: &Ticket, base_ref: &str) -> Result<(), String> {
    // self: attributes of THIS ticket, including its folded state.
    for (field, want) in &verb.require.self_ {
        let have = if field == "status" {
            state_at(ctx, &ticket.slug, &ticket.kind)?
        } else {
            ticket.fm.get(field).cloned().unwrap_or_default()
        };
        let ok = if want == "terminal" {
            fold::terminal_states(&ctx.schema, &ticket.kind).contains(&have)
        } else {
            &have == want
        };
        if !ok {
            return Err(format!(
                "refuse {}: '{}' has {field} '{have}', needs '{want}'",
                verb.name, ticket.slug
            ));
        }
    }

    // sections: existence, never content. A worker satisfies this with an
    // empty section -- deliberately, since content quality is the reviewer's
    // semantic call and not the tool's.
    if !verb.require.sections.is_empty() {
        let body = read_ticket(ctx, base_ref, &ticket.slug)
            .map(|t| t.body)
            .unwrap_or_else(|_| ticket.body.clone());
        for section in &verb.require.sections {
            if extract_section(&body, section).trim().is_empty()
                && !body
                    .lines()
                    .any(|l| l.trim().eq_ignore_ascii_case(&format!("## {section}")))
            {
                return Err(format!(
                    "refuse {}: '{}' has no '## {section}' section",
                    verb.name, ticket.slug
                ));
            }
        }
    }

    // neighbors: universally quantified over one direct edge. No exists, no
    // counts, no transitive closure.
    for (role, want) in &verb.require.neighbors {
        let neighbours: Vec<String> = match role.as_str() {
            "depends_on" => ticket.deps.clone(),
            "children" => children_of(ctx, &ticket.slug)?,
            other => {
                return Err(format!(
                    "verb '{}' gates on unknown edge role '{other}'",
                    verb.name
                ))
            }
        };
        let terminal = fold::terminal_states(&ctx.schema, &ticket.kind);
        let mut blockers = Vec::new();
        for n in neighbours {
            let Ok(nt) = read_ticket(ctx, &ctx.trunk, &n) else {
                blockers.push(format!("{n}(missing)"));
                continue;
            };
            let have = state_at(ctx, &n, &nt.kind)?;
            let ok = if want == "terminal" {
                terminal.contains(&have)
            } else {
                &have == want
            };
            if !ok {
                blockers.push(format!("{n}({have})"));
            }
        }
        if !blockers.is_empty() {
            return Err(format!(
                "refuse {}: '{}' has {role} not {want}: {}",
                verb.name,
                ticket.slug,
                blockers.join(" ")
            ));
        }
    }
    Ok(())
}

/// The tickets naming `slug` as their parent.
///
/// The edge is stored once, on the child, so finding children means reading
/// every ticket's frontmatter. That is O(T) blobs but must not be O(T)
/// processes: one `ls-tree` and one `cat-file --batch`, the same shape board
/// uses. A ticket whose frontmatter does not parse is skipped rather than
/// fatal -- one malformed ticket should not make a parent unreadable.
fn children_of(ctx: &Ctx, slug: &str) -> Result<Vec<String>, String> {
    let dir = format!("{}/tickets", ctx.plan_dir);
    let files = crate::git::ls_tree_md(&ctx.trunk, &dir)?;

    let stems: Vec<String> = files
        .iter()
        .filter_map(|f| {
            PathBuf::from(f)
                .file_stem()
                .and_then(|s| s.to_str())
                .map(String::from)
        })
        .collect();
    let specs: Vec<String> = files.iter().map(|f| format!("{}:{f}", ctx.trunk)).collect();
    let blobs = git::cat_file_batch(&specs)?;

    let mut out = Vec::new();
    for (stem, blob) in stems.into_iter().zip(blobs) {
        if stem == slug {
            continue;
        }
        let Some(blob) = blob else { continue };
        if let Ok(t) = parse_ticket(&stem, &blob) {
            if t.fm.get("parent").map(|p| p == slug).unwrap_or(false) {
                out.push(stem);
            }
        }
    }
    Ok(out)
}

fn apply_content(
    ctx: &Ctx,
    verb: &Verb,
    ticket: &Ticket,
    content_ref: &str,
    index: &git::ScratchIndex,
    message: &str,
) -> Result<bool, String> {
    if verb.content.is_empty() {
        return Ok(false);
    }
    let path = ctx.ticket_path(&ticket.slug);
    let mut blob = git::show(content_ref, &path).unwrap_or_default();
    let mut removed = false;

    for step in &verb.content {
        match step {
            Content::Bare(BareContent::Remove) => {
                index.remove(&path)?;
                removed = true;
            }
            Content::Annotate { annotate } => {
                let body = annotate.body.replace("$message", message);
                blob = append_section(&blob, &annotate.section, &body);
            }
            Content::Edge { edge } => {
                for (op, assignment) in edge {
                    for (field, target) in assignment {
                        let target = target.replace("$target", message);
                        blob = apply_edge(&blob, op, field, &target)?;
                    }
                }
            }
        }
    }
    if !removed {
        index.put(&path, &blob)?;
    }
    Ok(true)
}

fn append_section(blob: &str, section: &str, body: &str) -> String {
    let mut out = blob.trim_end().to_string();
    out.push_str(&format!("\n\n## {section}\n\n{}\n", body.trim()));
    out
}

fn apply_edge(blob: &str, op: &str, field: &str, target: &str) -> Result<String, String> {
    let split = split_frontmatter(blob);
    let mut lines: Vec<String> = split.fm.lines().map(|l| l.to_string()).collect();
    match op {
        "set" => {
            let entry = format!("{field}: {target}");
            match lines
                .iter()
                .position(|l| l.starts_with(&format!("{field}:")))
            {
                Some(i) => lines[i] = entry,
                None => lines.push(entry),
            }
        }
        "add" => {
            // Multi-valued edges are block lists, one target per line, so
            // concurrent additions of different targets land on different
            // lines and merge cleanly.
            if !lines.iter().any(|l| l.starts_with(&format!("{field}:"))) {
                lines.push(format!("{field}:"));
            }
            let at = lines
                .iter()
                .position(|l| l.starts_with(&format!("{field}:")))
                .unwrap();
            let item = format!("  - {target}");
            if !lines.contains(&item) {
                lines.insert(at + 1, item);
            }
        }
        "remove" => {
            let item = format!("  - {target}");
            lines.retain(|l| l.trim_end() != item.trim_end());
        }
        other => return Err(format!("unknown edge operation '{other}'")),
    }
    Ok(format!("---\n{}\n---\n{}", lines.join("\n"), split.body))
}

fn worktree_path(ctx: &Ctx, kind: &str, slug: &str) -> PathBuf {
    PathBuf::from(
        ctx.schema
            .worktrees
            .replace("$kind", kind)
            .replace("$slug", slug),
    )
}

/// Run one verb. Returns a human-readable report of what it did.
pub fn run(ctx: &Ctx, verb_name: &str, slug: &str, message: &str) -> Result<String, String> {
    let ticket = read_ticket(ctx, &ctx.trunk, slug)?;
    let kind = ticket.kind.clone();
    let own = ctx.own_ref(&kind, slug);
    // Everything a human reads, and `worktree add`, take the branch name; every
    // resolution takes the qualified one.
    let own_short = ctx.own_ref_short(&kind, slug);

    let verb = ctx
        .schema
        .verb(verb_name, &kind)
        .ok_or_else(|| format!("no verb '{verb_name}' applies to kind '{kind}'"))?
        .clone();

    let base_ref = match verb.base {
        Base::Home => ctx.trunk.clone(),
        Base::Own => {
            if !git::ref_exists(&own) {
                return Err(format!(
                    "refuse {verb_name}: '{slug}' has no branch {own_short} -- there is nothing on a branch to work from"
                ));
            }
            own.clone()
        }
    };

    // The state machine: `from` is structural and checked before `require`.
    let current = state_at(ctx, slug, &kind)?;
    if let Some(from) = &verb.from {
        if &current != from {
            return Err(format!(
                "refuse {verb_name}: '{slug}' is '{current}', not '{from}'"
            ));
        }
    } else if !verb.require.self_.contains_key("status") {
        // from-less verbs are the "any non-terminal state" case; the absorbing
        // rule keeps them from firing on an already-terminal ticket.
        //
        // Unless the verb states its own status precondition, in which case
        // the explicit rule wins. `archive` is exactly that: from-less, and
        // `require: { self: { status: terminal } }`. The implicit rule made it
        // contradict itself and it could never fire -- which is also why the
        // archived read path went unexercised for so long.
        if fold::terminal_states(&ctx.schema, &kind).contains(&current) {
            return Err(format!(
                "refuse {verb_name}: '{slug}' is already '{current}' (terminal)"
            ));
        }
    }

    check_require(ctx, &verb, &ticket, &base_ref)?;

    // Releasing a ref never destroys anything: a ticket-only merge records the
    // branch as a second parent, so its commits stay reachable from home even
    // though the tree does not carry them. There is deliberately no
    // destructive variant -- see the note on Effect::TicketOnly.
    let releases_ref = verb.effect == Effect::TicketOnly;
    let mut preserving: Option<(String, usize)> = None;
    if releases_ref && git::ref_exists(&own) {
        let ahead = git::count_unreachable(&ctx.trunk, &own)?;
        if ahead > 0 {
            preserving = Some((git::rev_parse(&own)?, ahead));
        }
    }

    // ---- build the tree, then the commit; nothing is referenced yet ----
    let base_sha = git::rev_parse(&base_ref)?;
    let index = git::ScratchIndex::from_ref(&base_sha)?;
    // Under TicketOnly the tree comes from home while the TICKET comes from
    // the branch -- that is precisely how the worker's rationale reaches trunk
    // without the work coming with it.
    let content_ref = match (verb.effect, preserving.is_some()) {
        (Effect::TicketOnly, true) => own.clone(),
        _ => base_ref.clone(),
    };
    let touched = apply_content(ctx, &verb, &ticket, &content_ref, &index, message)?;
    let tree = index.write_tree()?;

    let subject = format!("plan: {verb_name} {slug}");
    let mut body = String::new();
    if !message.is_empty() && verb.content.is_empty() {
        body.push_str(&format!("\n\n{message}"));
    }
    if let Some((tip, ahead)) = &preserving {
        body.push_str(&format!(
            "\n\nPreserved {ahead} unmerged commit(s) from {own_short} at {tip} in history; \
             the work itself is not applied to {}.",
            ctx.trunk
        ));
    }
    let commit_msg = format!("{subject}{body}\n\nPlanr-Verb: {verb_name}\nPlanr-Ticket: {slug}\n");
    let commit = git::commit_tree(&tree, &[&base_sha], &commit_msg)?;

    // ---- one ref movement ----
    let mut report = Vec::new();

    // Reconciling this worktree cannot fail the step -- `docs/semantics.md`
    // section 3.1: "The workspace is never history. Step 7 cannot fail the
    // step, and nothing in R or G depends on it."
    //
    // Propagating it did, and under `merge` the damage was not merely a
    // misleading message: the sync sits BETWEEN the merge and the ref release,
    // so a read-only checkout left trunk moved, the ticket transitioned, and
    // the branch and worktree leaked -- with no way to finish, because the
    // only verb that releases the ref then refused on its own `from` gate.
    // A half-applied verb with no completion path is much worse than a dirty
    // worktree.
    let sync = |report: &mut Vec<String>, ref_: &str, commit: &str| {
        let path = ctx.ticket_path(slug);
        if let Err(e) = git::sync_path(ref_, commit, &path) {
            report.push(format!(
                "warning: this worktree could not be updated to match: {e}\n  \
                 run `git restore --source={} -- {path}` once the cause is cleared",
                &commit[..7]
            ));
        }
    };

    match verb.effect {
        Effect::Advance => {
            git::update_ref(&base_ref, &commit, &base_sha)?;
            sync(&mut report, &base_ref, &commit);
            report.push(format!("{base_ref} -> {}", &commit[..7]));
        }
        Effect::Create => {
            git::create_ref(&own, &commit)?;
            report.push(format!("created {own_short} at {}", &commit[..7]));
        }
        Effect::Merge => {
            let merge_msg = format!(
                "plan: {verb_name} {slug}\n\nPlanr-Verb: {verb_name}\nPlanr-Ticket: {slug}\n"
            );
            let merged = git::merge_into(&ctx.trunk, &commit, &merge_msg)?;
            sync(&mut report, &ctx.trunk, &merged);
            report.push(format!("merged into {} at {}", ctx.trunk, &merged[..7]));
            git::delete_ref(&own)?;
            report.push(format!("released {own_short}"));
        }
        Effect::TicketOnly => {
            if let Some((_tip, ahead)) = &preserving {
                let merged = git::merge_ticket_only(&ctx.trunk, &own, &tree, &commit_msg)?;
                sync(&mut report, &ctx.trunk, &merged);
                report.push(format!(
                    "{} -> {} (ticket only: {ahead} commit(s) from {own_short} preserved in history, not applied)",
                    ctx.trunk,
                    &merged[..7]
                ));
            } else {
                // Nothing in flight -- an ordinary advance on home.
                git::update_ref(&base_ref, &commit, &base_sha)?;
                sync(&mut report, &base_ref, &commit);
                report.push(format!("{base_ref} -> {}", &commit[..7]));
            }
            if git::ref_exists(&own) {
                let path = worktree_path(ctx, &kind, slug);
                if path.exists() {
                    git::worktree_remove(&path.to_string_lossy())?;
                    report.push(format!("removed worktree {}", path.display()));
                }
                git::delete_ref(&own)?;
                report.push(format!("released {own_short}"));
            }
        }
    }

    // ---- workspace, which is never history ----
    match verb.worktree {
        Some(WorktreeAction::Create) => {
            let path = worktree_path(ctx, &kind, slug);
            // Idempotent, as the design says worktree actions are in both
            // directions. It matters for re-dispatch: `yield` leaves the
            // worktree standing, so a `resume` run where the worker never left
            // finds it already there, while one run elsewhere has to make it.
            if path.exists() {
                report.push(format!("worktree {} (already present)", path.display()));
            } else {
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                git::worktree_add(&path.to_string_lossy(), &own_short)?;
                report.push(format!("worktree {}", path.display()));
            }
        }
        Some(WorktreeAction::Remove) => {
            let path = worktree_path(ctx, &kind, slug);
            if path.exists() {
                git::worktree_remove(&path.to_string_lossy())?;
                report.push(format!("removed worktree {}", path.display()));
            }
        }
        None => {}
    }

    let new_state = state_at(ctx, slug, &kind)?;
    let content_note = if touched { "" } else { " (no content change)" };
    Ok(format!(
        "{verb_name} {slug}: {current} -> {new_state}{content_note}\n  {}",
        report.join("\n  ")
    ))
}
