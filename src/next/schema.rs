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
            if verb.worktree == Some(WorktreeAction::Create) && verb.effect != Effect::Create {
                return Err(format!(
                    "verb '{}': worktree 'create' requires effect 'create'",
                    verb.name
                ));
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
