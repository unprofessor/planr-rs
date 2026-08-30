//! Deriving state from the schema and a ticket's events.
//!
//! Nothing here reads a `status` field, because none exists. Three things are
//! derived: the initial state, the terminal set, and a ticket's current state.

use std::collections::BTreeSet;

use super::events::Event;
use super::schema::Schema;

/// The initial state.
///
/// Derived where the state graph allows it: the state that appears as some
/// verb's `from` but is never any verb's `to`. That works only while the
/// graph is acyclic, and a lifecycle with rework is deliberately NOT acyclic
/// -- `request-changes` returns to `in_progress`, `yield` returns to `todo`.
/// Once a cycle reaches back to the entry state, the graph genuinely no
/// longer contains the answer, because creation is `new` -- fixed tooling,
/// not a verb -- so its resulting state was never an edge in the first place.
///
/// So: derive when unambiguous, and require `templates.<kind>.initial` when
/// not. The common case stays declaration-free and the ambiguous case fails
/// with the fix in the message.
pub fn initial_state(schema: &Schema, kind: &str) -> Result<String, String> {
    if let Some(declared) = schema.templates.get(kind).and_then(|t| t.initial.as_ref()) {
        return Ok(declared.clone());
    }

    let mut froms = BTreeSet::new();
    let mut tos = BTreeSet::new();
    for verb in schema.verbs_for(kind) {
        if let Some(f) = &verb.from {
            froms.insert(f.clone());
        }
        if let Some(t) = &verb.to {
            tos.insert(t.clone());
        }
    }
    let mut candidates: Vec<String> = froms.difference(&tos).cloned().collect();
    match candidates.len() {
        1 => Ok(candidates.remove(0)),
        0 => Err(format!(
            "kind '{kind}' has no derivable initial state: every 'from' is also a 'to', which is what a rework cycle does. Declare it as templates.{kind}.initial"
        )),
        _ => Err(format!(
            "kind '{kind}' has {} candidate initial states ({}). Declare the intended one as templates.{kind}.initial",
            candidates.len(),
            candidates.join(", ")
        )),
    }
}

/// Terminal states, derived by stratification: pass one considers only
/// explicit-`from` transitions, so a `from`-less verb (abandon) can never
/// contribute to the terminal set -- it only consumes it.
pub fn terminal_states(schema: &Schema, kind: &str) -> BTreeSet<String> {
    let mut all = BTreeSet::new();
    let mut has_exit = BTreeSet::new();
    for verb in schema.verbs_for(kind) {
        if let Some(f) = &verb.from {
            all.insert(f.clone());
            has_exit.insert(f.clone());
        }
        if let Some(t) = &verb.to {
            all.insert(t.clone());
        }
    }
    all.difference(&has_exit).cloned().collect()
}

/// A ticket's state: the `to` of its most recent declaration, seeded by the
/// derived initial state. Events that name no verb in this kind's machine are
/// skipped rather than guessed at.
pub fn fold_state(schema: &Schema, kind: &str, events: &[Event]) -> Result<String, String> {
    let mut state = initial_state(schema, kind)?;
    for event in events {
        let Some(verb) = schema.verb(&event.verb, kind) else {
            continue;
        };
        if let Some(to) = &verb.to {
            state = to.clone();
        }
    }
    Ok(state)
}

/// Render a kind's derived sub-machine as text, so it can be matched against
/// intent by inspection rather than trusted.
pub fn render_lifecycle(schema: &Schema, kind: &str) -> Result<String, String> {
    let initial = initial_state(schema, kind)?;
    let terminal = terminal_states(schema, kind);
    let mut out = String::new();

    out.push_str(&format!("lifecycle for kind '{kind}'\n"));
    out.push_str(&format!("  initial:  {initial}\n"));
    out.push_str(&format!(
        "  terminal: {}\n",
        if terminal.is_empty() {
            "(none)".to_string()
        } else {
            terminal.iter().cloned().collect::<Vec<_>>().join(", ")
        }
    ));
    out.push_str("  transitions:\n");

    for verb in schema.verbs_for(kind) {
        let Some(to) = &verb.to else {
            continue;
        };
        let from = match &verb.from {
            Some(f) => f.clone(),
            None => format!("(any non-terminal, {} states)", {
                let mut all = BTreeSet::new();
                for v in schema.verbs_for(kind) {
                    if let Some(f) = &v.from {
                        all.insert(f.clone());
                    }
                    if let Some(t) = &v.to {
                        all.insert(t.clone());
                    }
                }
                all.difference(&terminal).count()
            }),
        };
        let gate = if verb.require.is_empty() {
            String::new()
        } else {
            let mut parts = Vec::new();
            for (k, v) in &verb.require.self_ {
                parts.push(format!("self.{k}={v}"));
            }
            for (k, v) in &verb.require.neighbors {
                parts.push(format!("all {k} {v}"));
            }
            if !verb.require.sections.is_empty() {
                parts.push(format!("sections[{}]", verb.require.sections.join(",")));
            }
            format!("   requires {}", parts.join(" AND "))
        };
        out.push_str(&format!(
            "    {from} --{}--> {to}{gate}\n",
            verb.name.clone()
        ));
    }

    // Verbs that change no state still belong in the picture -- they are
    // capability, and a board lists available actions from exactly this set.
    let stateless: Vec<&str> = schema
        .verbs_for(kind)
        .filter(|v| v.to.is_none())
        .map(|v| v.name.as_str())
        .collect();
    if !stateless.is_empty() {
        out.push_str(&format!("  stateless verbs: {}\n", stateless.join(", ")));
    }
    Ok(out)
}
