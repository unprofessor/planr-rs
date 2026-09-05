//! Schema loading for the 0.4 typed-graph model.
//!
//! The schema is data: kinds are a containment spine, verbs are declarations
//! with a base/content/effect shape. Nothing here is pinned to a published
//! URL yet -- the in-tree schema is deliberately unadvertised while the model
//! is still being experimented with.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

/// Which ref a verb's commit is built on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Base {
    /// The ticket's integration ref -- trunk, or the nearest ancestor holding
    /// an open integration branch. Trunk is the base case of that walk.
    #[default]
    Home,
    /// The ticket's own ref, `plan/<kind>/<slug>`.
    Own,
}

/// The single ref movement a verb performs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Effect {
    /// The commit lands on `base` and the base ref moves.
    #[default]
    Advance,
    /// Cut `plan/<kind>/<slug>` at the new commit (atomic create-or-fail).
    Create,
    /// Integrate the new commit into `home`.
    Merge,
    /// Integrate the ticket, not the work: the commit's tree is home's tree
    /// with this ticket taken from its own ref, and its parents are home and
    /// that ref. An `ours`-style merge -- SVN calls the same thing a
    /// record-only merge -- so the branch's commits become reachable from
    /// home, preserved in history and absent from the tree, and the ref can
    /// then be released without destroying anything.
    ///
    /// Named for the mechanics rather than the intent, because intent belongs
    /// to the verb. `abandon` supplies the sentiment here; a future verb with
    /// different sentiment must still be able to read this effect and have it
    /// mean the right thing.
    TicketOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorktreeAction {
    Create,
    Remove,
}

/// A pure tree transformation. Content steps know nothing of refs or HEAD.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Content {
    /// Bare `remove` -- delete the ticket file. This is what archival is.
    Bare(BareContent),
    Annotate {
        annotate: Annotate,
    },
    Edge {
        edge: BTreeMap<String, BTreeMap<String, String>>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BareContent {
    Remove,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Annotate {
    pub section: String,
    #[serde(default)]
    pub body: String,
}

/// The three reserved structural operators, combined by implicit AND.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Require {
    #[serde(default, rename = "self")]
    pub self_: BTreeMap<String, String>,
    #[serde(default)]
    pub neighbors: BTreeMap<String, String>,
    #[serde(default)]
    pub sections: Vec<String>,
}

impl Require {
    pub fn is_empty(&self) -> bool {
        self.self_.is_empty() && self.neighbors.is_empty() && self.sections.is_empty()
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Verb {
    pub name: String,
    #[serde(rename = "applies-to")]
    pub applies_to: Vec<String>,
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub to: Option<String>,
    #[serde(default)]
    pub require: Require,
    #[serde(default)]
    pub content: Vec<Content>,
    #[serde(default)]
    pub base: Base,
    #[serde(default)]
    pub effect: Effect,
    #[serde(default)]
    pub worktree: Option<WorktreeAction>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum KindsSpec {
    /// List form -- sugar for a totally-ordered hierarchy.
    List(Vec<String>),
    /// Object form -- explicit adjacency.
    Adjacency(BTreeMap<String, KindEntry>),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct KindEntry {
    parents: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct SchemaFile {
    kinds: KindsSpec,
    verbs: Vec<Verb>,
    #[serde(default = "default_worktrees")]
    worktrees: String,
    #[serde(default, rename = "$schema")]
    _schema: Option<String>,
    #[serde(default)]
    templates: BTreeMap<String, Template>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Template {
    #[serde(default)]
    pub body: String,
    /// The kind's initial state. Omit it when the state graph is acyclic --
    /// it is derived then. Required once a rework cycle reaches back to the
    /// entry state, because creation is fixed tooling rather than a verb, so
    /// the entry state is not an edge in the verb graph at all.
    #[serde(default)]
    pub initial: Option<String>,
}

fn default_worktrees() -> String {
    ".plan/worktrees/$kind/$slug".to_string()
}

#[derive(Debug, Clone)]
pub struct Kind {
    pub name: String,
    /// Adjacency is loaded and validated but not yet traversed: the parent
    /// chain is what `base: home` will walk once a container can hold an
    /// integration branch. Until then every home resolves to trunk.
    #[allow(dead_code)]
    pub parents: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Schema {
    pub kinds: Vec<Kind>,
    pub verbs: Vec<Verb>,
    pub worktrees: String,
    pub templates: BTreeMap<String, Template>,
}

impl Schema {
    pub fn load(plan_dir: &Path) -> Result<Schema, String> {
        let path = plan_dir.join("schema.yml");
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        Schema::parse(&text)
    }

    pub fn parse(text: &str) -> Result<Schema, String> {
        let file: SchemaFile =
            serde_yaml::from_str(text).map_err(|e| format!("schema is not valid: {e}"))?;

        // The list form desugars to adjacency, so the engine always speaks
        // adjacency: each element's parent is the one before it.
        let kinds = match file.kinds {
            KindsSpec::List(names) => {
                let mut out = Vec::new();
                let mut prev: Option<String> = None;
                for name in names {
                    out.push(Kind {
                        name: name.clone(),
                        parents: prev.into_iter().collect(),
                    });
                    prev = Some(name);
                }
                out
            }
            KindsSpec::Adjacency(map) => map
                .into_iter()
                .map(|(name, entry)| Kind {
                    name,
                    parents: entry.parents,
                })
                .collect(),
        };

        let schema = Schema {
            kinds,
            verbs: file.verbs,
            worktrees: file.worktrees,
            templates: file.templates,
        };
        schema.validate()?;
        Ok(schema)
    }

    fn validate(&self) -> Result<(), String> {
        if self.kinds.is_empty() {
            return Err("schema declares no kinds".to_string());
        }
        let known: Vec<&str> = self.kinds.iter().map(|k| k.name.as_str()).collect();
        for verb in &self.verbs {
            if verb.applies_to.is_empty() {
                return Err(format!("verb '{}' applies to no kind", verb.name));
            }
            for kind in &verb.applies_to {
                if !known.contains(&kind.as_str()) {
                    return Err(format!(
                        "verb '{}' applies to unknown kind '{kind}'",
                        verb.name
                    ));
                }
            }
            // The ref algebra, enforced here as well as in the published schema.
            if verb.effect == Effect::Create && verb.base != Base::Home {
                return Err(format!(
                    "verb '{}': effect 'create' requires base 'home'",
                    verb.name
                ));
            }
            if verb.effect == Effect::Merge && verb.base != Base::Own {
                return Err(format!(
                    "verb '{}': effect 'merge' requires base 'own'",
                    verb.name
                ));
            }
            if verb.effect == Effect::TicketOnly && verb.base != Base::Home {
                return Err(format!(
                    "verb '{}': effect 'ticket-only' requires base 'home'",
                    verb.name
                ));
            }
            // A worktree needs a ref to attach to AFTER the step, not merely
            // before it. `effect: create` cuts one; `base: own` requires an
            // existing one (re-dispatching a yielded ticket is that shape).
            // But `merge` releases the ref it just validated, so `base: own`
            // alone is not enough -- see docs/semantics.md section 2.1. That
            // combination does not error; it leaves a worktree pointing at a
            // deleted branch and reports success.
            if verb.worktree == Some(WorktreeAction::Create) {
                let establishes = verb.effect == Effect::Create || verb.base == Base::Own;
                let releases = verb.effect == Effect::Merge || verb.effect == Effect::TicketOnly;
                if !establishes {
                    return Err(format!(
                        "verb '{}': worktree 'create' needs a ref to attach to -- either effect 'create' to cut one, or base 'own' to require an existing one",
                        verb.name
                    ));
                }
                if releases {
                    return Err(format!(
                        "verb '{}': worktree 'create' cannot follow effect '{}', which releases the ticket's ref -- the worktree would point at a deleted branch",
                        verb.name,
                        if verb.effect == Effect::Merge { "merge" } else { "ticket-only" }
                    ));
                }
            }
        }
        // Overlapping applies-to for one verb name is ambiguous dispatch.
        for (i, a) in self.verbs.iter().enumerate() {
            for b in self.verbs.iter().skip(i + 1) {
                if a.name != b.name {
                    continue;
                }
                if let Some(dup) = a.applies_to.iter().find(|k| b.applies_to.contains(k)) {
                    return Err(format!(
                        "verb '{}' is defined twice for kind '{dup}'",
                        a.name
                    ));
                }
            }
        }
        Ok(())
    }

    /// Resolve `(name, kind)` to exactly one verb definition.
    pub fn verb(&self, name: &str, kind: &str) -> Option<&Verb> {
        self.verbs
            .iter()
            .find(|v| v.name == name && v.applies_to.iter().any(|k| k == kind))
    }

    /// The unit is DERIVED: the kind whose verbs create a worktree.
    pub fn unit(&self) -> Option<&str> {
        self.verbs
            .iter()
            .find(|v| v.worktree == Some(WorktreeAction::Create))
            .and_then(|v| v.applies_to.first())
            .map(|s| s.as_str())
    }

    pub fn verbs_for<'a>(&'a self, kind: &'a str) -> impl Iterator<Item = &'a Verb> + 'a {
        self.verbs
            .iter()
            .filter(move |v| v.applies_to.iter().any(|k| k == kind))
    }
}

#[cfg(test)]
mod wf {
    //! The forcing function for `docs/semantics.md` section 2.
    //!
    //! Well-formedness is stated there as five POSITIVE rules over
    //! `base x effect`, plus a side condition on `worktree`. Prohibitions
    //! leave gaps by construction -- you cannot tell by reading them whether
    //! the list is complete -- so this enumerates the entire product space and
    //! pins every cell. A rule changed in one place and not the other fails
    //! here.

    use super::*;

    fn verb(base: &str, effect: &str, worktree: Option<&str>) -> String {
        let wt = worktree
            .map(|w| format!("\n    worktree: {w}"))
            .unwrap_or_default();
        format!(
            "kinds: [task]\n\
             verbs:\n\
             \x20 - name: probe\n\
             \x20   applies-to: [task]\n\
             \x20   from: a\n\
             \x20   to: b\n\
             \x20   base: {base}\n\
             \x20   effect: {effect}{wt}\n"
        )
    }

    /// Section 2: the 2 x 4 `base x effect` space, partitioned 5 legal / 3 not.
    #[test]
    fn the_base_by_effect_space_is_partitioned_exactly() {
        let legal = [
            ("home", "advance"),     // W-Declare-Home
            ("home", "create"),      // W-Cut
            ("home", "ticket-only"), // W-Retire
            ("own", "advance"),      // W-Declare-Own
            ("own", "merge"),        // W-Integrate
        ];

        let mut seen = 0;
        for base in ["home", "own"] {
            for effect in ["advance", "create", "merge", "ticket-only"] {
                let accepted = Schema::parse(&verb(base, effect, None)).is_ok();
                let expected = legal.contains(&(base, effect));
                assert_eq!(
                    accepted, expected,
                    "base '{base}' x effect '{effect}': schema.rs and \
                     semantics.md section 2 disagree"
                );
                seen += 1;
            }
        }
        assert_eq!(seen, 8, "the product space is 2 x 4");
    }

    /// Section 2.1: `worktree: create` needs a ref that survives the step.
    /// `own x merge` satisfies "there is a ref" and then releases it, which is
    /// the case an enumeration finds and prose does not.
    #[test]
    fn worktree_create_requires_a_ref_that_outlives_the_effect() {
        for (base, effect, ok) in [
            ("home", "create", true),       // W-Cut establishes it
            ("own", "advance", true),       // W-Declare-Own leaves it in place
            ("home", "advance", false),     // nothing to attach to
            ("home", "ticket-only", false), // releases it
            ("own", "merge", false),        // validates it, THEN releases it
        ] {
            let accepted = Schema::parse(&verb(base, effect, Some("create"))).is_ok();
            assert_eq!(
                accepted, ok,
                "base '{base}' x effect '{effect}' x worktree 'create'"
            );
        }
    }

    /// `worktree: remove` carries no side condition, so it is legal wherever
    /// the base/effect pair is. Pinned so that adding one is a deliberate act.
    #[test]
    fn worktree_remove_carries_no_side_condition() {
        for (base, effect) in [
            ("home", "advance"),
            ("home", "create"),
            ("home", "ticket-only"),
            ("own", "advance"),
            ("own", "merge"),
        ] {
            assert!(
                Schema::parse(&verb(base, effect, Some("remove"))).is_ok(),
                "base '{base}' x effect '{effect}' x worktree 'remove'"
            );
        }
    }

    /// Section 2: every legal cell is inhabited by a real verb, so the
    /// language has no dead corners. Checked against the reference schema.
    #[test]
    fn every_legal_cell_is_inhabited_by_the_reference_schema() {
        let schema = Schema::parse(include_str!("../../.plan/schema.yml")).unwrap();
        let mut found: Vec<(Base, Effect)> = Vec::new();
        for v in &schema.verbs {
            if !found.contains(&(v.base, v.effect)) {
                found.push((v.base, v.effect));
            }
        }
        for cell in [
            (Base::Home, Effect::Advance),
            (Base::Home, Effect::Create),
            (Base::Home, Effect::TicketOnly),
            (Base::Own, Effect::Advance),
            (Base::Own, Effect::Merge),
        ] {
            assert!(
                found.contains(&cell),
                "{cell:?} is legal but no reference verb inhabits it"
            );
        }
    }
}
