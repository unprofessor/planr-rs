//! Workflow loading for the 0.4 typed-graph model.
//!
//! The workflow is data: kinds are a containment spine, verbs are declarations
//! with a base/content/effect shape. Nothing here is pinned to a published
//! URL yet -- the in-tree workflow is deliberately unadvertised while the model
//! is still being experimented with.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

/// What a ticket's slug may look like, mirroring `$defs/slug` in the published
/// schema -- and pinned against that document by a test below, because this is
/// the third copy of a rule and the other two have drifted before.
///
/// The rule the pattern stands in for: **a slug must be exactly what comes
/// back out of `Planr-Ticket`.** The slug is the ticket's identity in the
/// event log AND its filename, and git's trailer reader trims, so a slug of
/// `"foo "` writes `Planr-Ticket: foo` -- one identity for two files, and a
/// verb run against one of them silently moves the other. Nothing in the
/// pattern survives trimming or splits a trailer, which is what makes the
/// round trip hold; the property itself is asserted in `tests/next-identity`
/// over every accepted shape.
///
/// Deliberately NOT 0.3's stricter `^[a-z0-9]+(-[a-z0-9]+)*$`: 0.4's contract
/// is the published document, and rejecting a slug that document calls valid
/// would be a new drift rather than a shared rule.
pub const SLUG_PATTERN: &str = "^[a-z0-9][a-z0-9_-]*$";

/// The other half of the slug contract: the pattern says what survives a
/// trailer, this says what survives a filesystem. Kept here beside the pattern
/// so one test can pin both against the published document.
pub const SLUG_MAX: usize = 96;

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
struct WorkflowFile {
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
pub struct Workflow {
    pub kinds: Vec<Kind>,
    pub verbs: Vec<Verb>,
    pub worktrees: String,
    pub templates: BTreeMap<String, Template>,
}

impl Workflow {
    pub fn load(plan_dir: &Path) -> Result<Workflow, String> {
        let path = plan_dir.join("workflow.yml");
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        Workflow::parse(&text)
    }

    pub fn parse(text: &str) -> Result<Workflow, String> {
        let file: WorkflowFile =
            serde_yaml::from_str(text).map_err(|e| format!("workflow is not valid: {e}"))?;

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

        let workflow = Workflow {
            kinds,
            verbs: file.verbs,
            worktrees: file.worktrees,
            templates: file.templates,
        };
        workflow.validate()?;
        Ok(workflow)
    }

    fn validate(&self) -> Result<(), String> {
        if self.kinds.is_empty() {
            return Err("workflow declares no kinds".to_string());
        }
        let known: Vec<&str> = self.kinds.iter().map(|k| k.name.as_str()).collect();
        for verb in &self.verbs {
            // Genesis owns this name. Creation writes `Planr-Verb: new`, and a
            // bounded state read stops its backwards walk at that record --
            // the floor for a ticket that has never transitioned. A verb of
            // the same name puts a second floor into every walk, and it fails
            // silently rather than loudly: `verb::run` reads the before and
            // after states through that same walk, so even a stateless verb
            // would report itself as a transition to the initial state. The
            // published schema rejects it too, and the constant lives with the
            // walk that depends on it.
            if verb.name == super::events::GENESIS {
                return Err(format!(
                    "verb '{}' is a reserved name: creation is fixed tooling rather than a verb, and it writes 'Planr-Verb: {}' -- the record a bounded state read stops at. A verb of that name would end every walk at itself. Rename it",
                    verb.name,
                    super::events::GENESIS
                ));
            }
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

    /// The document itself, read at compile time so that editing it forces a
    /// rebuild of these tests.
    const SEMANTICS: &str = include_str!("../../docs/semantics.md");

    /// Split one markdown table row into its cells, stripped of the emphasis
    /// and code markers the prose uses.
    fn cells(line: &str) -> Vec<String> {
        line.trim()
            .trim_start_matches('|')
            .trim_end_matches('|')
            .split('|')
            .map(|c| c.trim().trim_matches(|ch| ch == '*' || ch == '`').trim())
            .map(String::from)
            .collect()
    }

    /// Section 2's `base x effect` grid, read out of the document: the header
    /// row supplies the effect names, and each body row a base name.
    fn base_by_effect_table() -> Vec<(String, String, bool)> {
        let lines: Vec<&str> = SEMANTICS.lines().collect();
        // Match on the header's exact cell structure, not on the words it
        // contains: section 0's notation table names all four effects in
        // running prose and is otherwise a plausible match.
        let header = lines
            .iter()
            .position(|l| {
                l.starts_with('|') && cells(l)[1..] == ["advance", "create", "merge", "ticket-only"]
            })
            .expect("semantics.md section 2 has no base x effect table");

        let effects = &cells(lines[header])[1..];
        let mut out = Vec::new();
        for line in lines[header + 2..]
            .iter()
            .take_while(|l| l.starts_with('|'))
        {
            let row = cells(line);
            let base = row[0].clone();
            for (effect, cell) in effects.iter().zip(&row[1..]) {
                out.push((base.clone(), effect.clone(), !cell.contains("ill-formed")));
            }
        }
        out
    }

    /// Section 2.1's worktree table: base, effect, and a verdict cell that
    /// begins with either `permitted` or `rejected`.
    fn worktree_table() -> Vec<(String, String, bool)> {
        let lines: Vec<&str> = SEMANTICS.lines().collect();
        let header = lines
            .iter()
            .position(|l| l.starts_with('|') && cells(l) == ["base", "effect", "worktree: create"])
            .expect("semantics.md section 2.1 has no worktree table");

        lines[header + 2..]
            .iter()
            .take_while(|l| l.starts_with('|'))
            .map(|l| {
                let row = cells(l);
                let permitted = match row[2].split_whitespace().next() {
                    Some("permitted") => true,
                    Some("rejected") => false,
                    other => panic!("unreadable verdict {other:?} in the worktree table"),
                };
                (row[0].clone(), row[1].clone(), permitted)
            })
            .collect()
    }

    /// Section 2: the 2 x 4 `base x effect` space, partitioned 5 legal / 3 not.
    ///
    /// Driven from the table in the document rather than from a copy of it.
    /// A copy would agree with the document because the same hand wrote both,
    /// which is the self-consistency trap round 4 recorded: a suite that
    /// checks only its own fixtures proves nothing about the thing it cites.
    #[test]
    fn the_base_by_effect_space_matches_the_document() {
        let table = base_by_effect_table();
        assert_eq!(table.len(), 8, "the product space is 2 x 4");
        assert_eq!(
            table.iter().filter(|(_, _, legal)| *legal).count(),
            5,
            "section 2 claims the space partitions 5 legal / 3 ill-formed"
        );

        for (base, effect, legal) in table {
            let accepted = Workflow::parse(&verb(&base, &effect, None)).is_ok();
            assert_eq!(
                accepted, legal,
                "base '{base}' x effect '{effect}': workflow.rs and \
                 semantics.md section 2 disagree"
            );
        }
    }

    /// Section 2.1: `worktree: create` needs a ref that survives the step.
    /// `own x merge` satisfies "there is a ref" and then releases it, which is
    /// the case an enumeration finds and prose does not.
    #[test]
    fn worktree_create_matches_the_document() {
        let table = worktree_table();
        assert!(
            table.len() >= 5,
            "section 2.1 should cover every legal base/effect pair"
        );
        for (base, effect, permitted) in table {
            let accepted = Workflow::parse(&verb(&base, &effect, Some("create"))).is_ok();
            assert_eq!(
                accepted, permitted,
                "base '{base}' x effect '{effect}' x worktree 'create': \
                 workflow.rs and semantics.md section 2.1 disagree"
            );
        }
    }

    /// Section 2.1 says `worktree: remove` carries no side condition, so it is
    /// legal wherever the base/effect pair is. Pinned so that adding one is a
    /// deliberate act rather than a side effect.
    #[test]
    fn worktree_remove_carries_no_side_condition() {
        for (base, effect, legal) in base_by_effect_table() {
            if !legal {
                continue;
            }
            assert!(
                Workflow::parse(&verb(&base, &effect, Some("remove"))).is_ok(),
                "base '{base}' x effect '{effect}' x worktree 'remove'"
            );
        }
    }

    /// Section 2: every legal cell is inhabited by a real verb, so the
    /// language has no dead corners. Checked against the reference workflow.
    #[test]
    fn every_legal_cell_is_inhabited_by_the_reference_schema() {
        let workflow = Workflow::parse(include_str!("../../.plan/workflow.yml")).unwrap();
        let mut found: Vec<(Base, Effect)> = Vec::new();
        for v in &workflow.verbs {
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

    /// Section 2.2, W-Reserved. The published schema rejects this too, via
    /// `tests/fixtures/workflow/root/invalid/verb-named-new.yml` -- both sides
    /// are pinned because three-way drift between the reference workflow, this
    /// file, and the published document is how the earlier renames got lost.
    #[test]
    fn the_genesis_name_is_not_available_to_a_verb() {
        let with_name = |name: &str| {
            format!(
                "kinds: [task]\n\
                 verbs:\n\
                 \x20 - name: {name}\n\
                 \x20   applies-to: [task]\n\
                 \x20   to: todo\n"
            )
        };
        let err = Workflow::parse(&with_name(crate::next::events::GENESIS))
            .expect_err("a verb named 'new' must not load");
        assert!(err.contains("reserved"), "unhelpful refusal: {err}");
        // Only the name is disqualifying -- the same verb otherwise loads.
        assert!(Workflow::parse(&with_name("spawn")).is_ok());
    }

    /// The slug rule is written down twice -- here and in the published
    /// document -- so it is read out of the document rather than restated.
    /// `tests/workflow.rs` validates fixtures against the document and would not
    /// notice the ENGINE drifting away from it, which is the direction that
    /// lets planr accept a slug its own workflow calls invalid.
    #[test]
    fn the_slug_pattern_matches_the_published_schema() {
        const PUBLISHED: &str = include_str!("../../schemas/planr/v1/1.0.0/planr.schema.json");
        let doc: serde_json::Value = serde_json::from_str(PUBLISHED).unwrap();
        let published = doc["$defs"]["slug"]["pattern"]
            .as_str()
            .expect("the published schema has no $defs/slug pattern");
        assert_eq!(
            published, SLUG_PATTERN,
            "the engine and the published schema disagree about what a slug is"
        );

        let max = doc["$defs"]["slug"]["maxLength"]
            .as_u64()
            .expect("the published schema has no $defs/slug maxLength");
        assert_eq!(
            max as usize, SLUG_MAX,
            "the engine and the published schema disagree about how long a slug may be"
        );
    }
}
