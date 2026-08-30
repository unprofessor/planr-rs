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

    pub fn own_ref(&self, kind: &str, slug: &str) -> String {
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

/// Current state of a ticket, plus which enumeration strategy answered.
pub fn state_of(ctx: &Ctx, slug: &str) -> Result<(String, &'static str, usize), String> {
    let ticket = read_ticket(ctx, &ctx.trunk, slug)?;
    let (evs, how) = events::for_ticket(slug, &ticket.kind, &ctx.trunk)?;
    let state = fold::fold_state(&ctx.schema, &ticket.kind, &evs)?;
    Ok((state, how, evs.len()))
}

fn state_at(ctx: &Ctx, slug: &str, kind: &str) -> Result<String, String> {
    let (evs, _) = events::for_ticket(slug, kind, &ctx.trunk)?;
    fold::fold_state(&ctx.schema, kind, &evs)
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

fn children_of(ctx: &Ctx, slug: &str) -> Result<Vec<String>, String> {
    let dir = format!("{}/tickets", ctx.plan_dir);
    let listing = git::log_raw(&["-0"]).ok(); // keep git warm; ignored
    let _ = listing;
    let files = crate::git::ls_tree_md(&ctx.trunk, &dir)?;
    let mut out = Vec::new();
    for f in files {
        let Some(stem) = PathBuf::from(&f)
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
        else {
            continue;
        };
        if stem == slug {
            continue;
        }
        if let Ok(t) = read_ticket(ctx, &ctx.trunk, &stem) {
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
    base_ref: &str,
    index: &git::ScratchIndex,
    message: &str,
) -> Result<bool, String> {
    if verb.content.is_empty() {
        return Ok(false);
    }
    let path = ctx.ticket_path(&ticket.slug);
    let mut blob = git::show(base_ref, &path).unwrap_or_default();
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
                    "refuse {verb_name}: '{slug}' has no branch {own} -- it is not claimed"
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
    } else {
        // from-less verbs are the "any non-terminal state" case; the absorbing
        // rule keeps them from firing on an already-terminal ticket.
        if fold::terminal_states(&ctx.schema, &kind).contains(&current) {
            return Err(format!(
                "refuse {verb_name}: '{slug}' is already '{current}' (terminal)"
            ));
        }
    }

    check_require(ctx, &verb, &ticket, &base_ref)?;

    // ---- build the tree, then the commit; nothing is referenced yet ----
    let base_sha = git::rev_parse(&base_ref)?;
    let index = git::ScratchIndex::from_ref(&base_sha)?;
    let touched = apply_content(ctx, &verb, &ticket, &base_ref, &index, message)?;
    let tree = index.write_tree()?;

    let subject = format!("plan: {verb_name} {slug}");
    let mut body = String::new();
    if !message.is_empty() && verb.content.is_empty() {
        body.push_str(&format!("\n\n{message}"));
    }
    let commit_msg = format!("{subject}{body}\n\nPlanr-Verb: {verb_name}\nPlanr-Ticket: {slug}\n");
    let commit = git::commit_tree(&tree, &[&base_sha], &commit_msg)?;

    // ---- one ref movement ----
    let mut report = Vec::new();
    match verb.effect {
        Effect::Advance => {
            git::update_ref(&base_ref, &commit)?;
            report.push(format!("{base_ref} -> {}", &commit[..7]));
        }
        Effect::Create => {
            git::create_ref(&own, &commit)?;
            report.push(format!("created {own} at {}", &commit[..7]));
        }
        Effect::Merge => {
            let merge_msg = format!(
                "plan: {verb_name} {slug}\n\nPlanr-Verb: {verb_name}\nPlanr-Ticket: {slug}\n"
            );
            let merged = git::merge_into(&ctx.trunk, &commit, &merge_msg)?;
            report.push(format!("merged into {} at {}", ctx.trunk, &merged[..7]));
            git::delete_ref(&own)?;
            report.push(format!("released {own}"));
        }
        Effect::Delete => {
            git::delete_ref(&own)?;
            report.push(format!("deleted {own}"));
        }
    }

    // ---- workspace, which is never history ----
    match verb.worktree {
        Some(WorktreeAction::Create) => {
            let path = worktree_path(ctx, &kind, slug);
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            git::worktree_add(&path.to_string_lossy(), &own)?;
            report.push(format!("worktree {}", path.display()));
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
