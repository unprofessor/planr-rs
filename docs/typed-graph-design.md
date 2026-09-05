# planr — the schema-driven, typed-graph model

> **Version names.** **0.3** means planr as it ships today (0.3.1 at the time
> of writing). **0.4** means the model described here -- 0.4.0 under the
> crate's pre-1.0 semver policy, since this is a breaking change; whether it
> actually ships as 0.4.0 or is promoted to 1.0.0 is a product call that
> changes nothing in this document. The **`v1` in the schema URL is neither**:
> it versions the schema *language*, and moves only when the key set or a
> primitive's meaning changes, never with a planr release.
>
> **Status:** design exploration (pre-1.0). The schema language is pinned:
> `schemas/planr/v1/1.0.0/planr.schema.json` is published at
> `https://schemas.columnzero.com/planr/v1/1.0.0/planr.schema.json` and
> validated in CI against a fixture corpus. A throwaway engine spike lives
> behind the `next` cargo feature (`src/next/**`); it is reference material for
> what the model costs in practice, not a foundation.
>
> This document says what the model is **for**.
> [`semantics.md`](semantics.md) says what it **means** -- the well-formedness
> rules for a verb, the transition relation for `do`, and the algebra that
> makes state a fold. Read it before changing `validate()` or the effect
> vocabulary; the rules there are enforced in `src/next/schema.rs` and pinned
> by a test that enumerates the whole `base × effect × worktree` space.
>
> This captures the reasoning and decisions from the 0.4 brainstorm so they
> survive the conversation. Open questions are flagged inline and collected in
> §8; review and working rounds are recorded in §9, §9a and §9b. **Round 3
> (§9b) changed the model** -- ticket state is now folded from commit events
> rather than stored in frontmatter -- so read §3.3 before §3.4.

## 1. Why change anything

0.3 hard-wires two things that turn out to be project-specific:

1. **The hierarchy** is a closed `Epic → Story → Task` enum (`Kind` in
   `ticket.rs`), with a fixed directory mapping (`kind_to_subdir`) and fixed
   parent rules. Only `Task` is a *unit of execution* (worktree, branch,
   claim, review, merge); `Story`/`Epic` are pure **rollup gates**
   ("all children done").
2. **The workflow** (`todo → in_progress → review → done`, plus
   `blocked`/`abandoned`) is a fixed state machine whose transitions are driven
   by hard-coded operations (`claim`, `review`, `close`). Most of the *policy*
   actually lives in the skill prose, not the binary — the binary enforces only
   a couple of structural checks (`close task` requires `status: review` + an
   `approved` verdict before merging).

The backlog itself shows the rigidity chafing: epic 07 bolts *milestones* on as
a fourth scope via **directory placement** because there was no room in the
schema for a second grouping axis; epic 06 builds a general graph over all edge
types. We're already patching around the fixed shape.

**Goal:** make the hierarchy and workflow *data* (a small schema) instead of
code, and unify relationships into a typed graph — without turning planr into a
policy engine or a database.

## 2. Principles (the guardrails)

These are the constraints that keep "configurable" from becoming "unbounded":

- **Files are the source of truth.** One markdown file per live ticket.
  Observable (`git diff`, `grep`, PR review, human/Obsidian edit), mergeable,
  in-repo. This is load-bearing and non-negotiable — it's why `branch = claim`
  works and why a relational DB is rejected (§6).
- **Derive, don't store.** All groupings/rollups are computed by scanning at
  read time (as 0.3 already does for the board). Any index is a *disposable,
  rebuildable cache*, never authoritative.
- **Enforce structure, never semantics.** The tool may check that a ticket is in
  the `approved` state (structural); it must never judge whether the acceptance
  criteria are *actually* met (semantic — that's the reviewer agent's
  natural-language judgment). This line is frozen into the `require` predicate
  vocabulary, so it holds by construction.
- **Few fixed rules, shared by stateless agents.** planr's value is that
  independently-spawned, fresh-context leader/worker/reviewer agents coordinate
  because the rules are few and fixed. Every configurable axis is an axis where
  two agents can silently diverge unless both load and read the same schema
  identically. So: keep the schema tiny, ship **named presets** for the common
  cases, and make bespoke schemas the exception.
- **Compatibility is outcome-equivalence, not byte-identity** (round-2). The
  default schema runs existing 0.3 backlogs and produces the same *gate/merge
  outcomes* — but it is not byte-for-byte 0.3 (verdict-as-state alone guarantees
  that, §3.3), and "byte-for-byte" is not a goal. The regression test compares
  outcomes on migrated backlogs. See §7 for the (non-byte-identical) presets.
- **CLI completeness.** Every intended operation by every expected actor
  (leader/worker/reviewer) is achievable as a verb. Hand-editing files stays
  *permitted* (it's git — can't and shouldn't be prevented) but is the fallback,
  never a required step. Reason it's load-bearing: a step that *requires*
  hand-editing can't be structurally guarded and forces every stateless agent to
  learn the file format, not just the verb set. `lint` is the safety net for
  out-of-band edits — it promotes `require` predicates to standing invariants
  (§3.6, double duty).

## 3. The model

### 3.1 `kinds` — the decomposition tree

The value's **type selects the semantics** (no second keyword for one idea):

```yaml
# list form — a simple totally-ordered hierarchy; last element is the unit
kinds: [epic, story, task]
```

```yaml
# object form — adjacency; needed for per-kind attributes or multi-parent kinds
kinds:
  epic:  { parents: [] }
  story: { parents: [epic] }
  task:  { parents: [story] }
```

- The list form is **sugar** that desugars to adjacency internally, so the
  engine always speaks adjacency. This buys the future extensible case (e.g. a
  `maintenance` kind with `parents: [epic, story]`) as a config change, not a
  rewrite.
- Exactly one kind is the **unit** — the one that gets a worktree, a branch,
  review, and a merge. It is **derived, not declared**: the unit is the kind
  whose verbs declare `worktree: create` (§3.4). There is no `unit: true` key;
  the same reasoning that makes the lifecycle emergent makes this emergent too,
  and it closes old open question 7.
- **The unit is a declared *cut* across the tree, not necessarily its leaf.**
  Kinds above the cut roll up; the kind at the cut is the execution boundary;
  kinds *below* it are **sub-unit structure** carried inside the unit's branch.
  So a project can make `story` the unit while `task` survives beneath it with
  its own small lifecycle — keeping per-task acceptance criteria and graph
  visibility while paying for one worker and one reviewer per story instead of
  per task. This decouples two things 0.3 conflated by accident: the execution
  boundary and the bottom of the decomposition tree.
- Decomposition stays a **tree** (single `parent` per node) at runtime. KISS/
  YAGNI: we are *not* building multi-parent decomposition now, but the
  adjacency representation leaves the door open.

**Non-spine kinds live in `groups` (R6).** `kinds` is the *containment spine*, so
a kind that isn't part of decomposition (a milestone) doesn't belong in it —
which is exactly why `parents: []` there would collide with an epic-as-root. A
grouping kind is declared separately and additively, so the common case stays a
clean list:

```yaml
kinds: [epic, story, task]           # containment spine (unchanged)
groups:
  milestone: { inverse: members }    # a non-spine kind + its group edge, one declaration
```

`groups.milestone` materializes **both** the milestone *node-kind* (with its own
derived lifecycle, §3.3) **and** the `milestone` *group edge* on members (forward
field on the member; inverse role `members`, which also feeds R9's inverse-role
registry). This is the concrete form of §3.2's "a milestone is both a kind and an
edge." Each kind's lifecycle — spine or group — is *derived from the verbs that
apply to it* (§3.3), so nothing per-kind is declared beyond the kind itself.

### 3.2 Edges and groups — the typed graph, disciplined

Relationships are typed edges. Only two carry gating semantics; the rest are
grouping or informational:

| edge               | direction        | semantics                              |
|--------------------|------------------|----------------------------------------|
| `parent`           | child → parent   | **rollup** (container done ⇐ children done) |
| `depends_on`       | dependent → dep  | **gate** (blocks claim until deps done); a DAG (cycle-checked) |
| group edges (e.g. `milestone`) | member → group | **group only**, no gate; derived views |
| `link` (`[[..]]`)  | undirected-ish   | **info**, cosmetic; **body prose, not a frontmatter edge** |

`parent`, `depends_on`, and group edges are **frontmatter fields** written by the
`edge` primitive (§3.5). `link` is different: it is a `[[slug]]` reference in the
**body**, authored inline like any prose (via an `annotate` body or ordinary
editing) and *derived* by parsing — so it needs no verb and no `edge` op, and its
absence from the `edge` primitive is not a CLI-completeness gap (round-2).

The key insight: the decomposition tree could never express a *second*
grouping (milestones group epics/stories that already have a parent). A typed
**group edge** expresses it as an orthogonal relationship, deleting the
milestone-as-directory hack. Design shape: **one decomposition tree + N
orthogonal groups + the dependency DAG + info links.** (A group edge's anchor
kind is declared in `groups`, §3.1.)

**Storage-side = ownership.** Every edge is a frontmatter field on the node
that *declares* it, pointing outward, and which side stores it encodes who owns
the relationship — which gives correct behavior under mutation for free:

- `parent` on the **child** → the child owns its membership; abandon/archive the
  child and the edge travels with it, and the parent's rollup is re-derived from
  whoever still points at it. This is 0.3's "no agent ever edits a parent to
  record child state," generalized.
- `depends_on` on the **dependent** → the dependent owns its ordering; abandon a
  *dependency* and the edge is untouched, the ordering stays modeled, the dep
  just becomes `terminal` (non-blocking). Nothing to erase.
- a group edge (`milestone`) on the **member** → same mechanic as `parent`.

A **milestone is both a kind and an edge**: a node of kind `milestone` (outside
the decomposition spine — no `parent`, not a `unit`) that holds the release
record and its own lifecycle (`planned → in_progress → released`), plus the
`milestone` group edge that members point at. Its completion is opt-in-gated by
a `release` verb requiring `{members: terminal}` (§3.6) — non-blocking as a
grouping, gated only when explicitly released.

So all four edge types are **one mechanism**: a directional field on the owning
node, forward+inverse adjacency derived at read time, differentiated only by a
semantics tag (`rollup` / `gate` / `group` / `info`). Declaring a new group is a
name + a tag; there is no "group subsystem." (Precision: since `abandon` keeps
the file, an abandoned child's `parent` edge persists — it stays in the child
set but as `terminal`, satisfying the gate rather than leaving the set; it only
truly vanishes on `archive`. Same end state — the owner controls participation.)

```yaml
edges:                                   # `parent` and `depends_on` are built in;
  parent:     { cardinality: one,  semantics: rollup }   # on child;     inverse: children
  depends_on: { cardinality: many, semantics: gate }     # on dependent; inverse: dependents
  milestone:  { cardinality: one,  semantics: group }    # on member;    inverse: members
                                                         # (declared in `groups`, §3.1)
  # link: semantics `info` -- body [[..]] prose, NOT a frontmatter edge, so it
  # is never declared here and the `edge` primitive has no domain over it.
```

Cardinality is declared alongside the tag because it is what selects the legal
`edge` operations (`set` on single-valued, `add`/`remove` on multi-valued, §3.5)
and how the field is written to disk (a block list for multi-valued, §4).

**Inverse-role names (R9).** `neighbors` (§3.6) traverses an edge in either
direction, so each edge registers a **forward name** (the frontmatter field, on
the owner) and an **inverse-role name** for reading the other way: `parent ↔
children`, `milestone ↔ members`, `depends_on ↔ dependents`. Built-ins are
fixed; a group edge declares its inverse in `groups` (`{inverse: members}`,
§3.1). `neighbors` may name either direction; `lint` validates the name against
this registry (so `neighbors: {children: terminal}` resolves, and a typo does
not).

### 3.3 Lifecycle — a state machine (not a DAG)

The lifecycle has a rework cycle (`review → changes-requested → in_progress →
review`), so it is a **state machine**, not a DAG. (The DAG is `depends_on`,
a separate graph — don't conflate the two.)

**State is not stored — it is folded from events (round-3).** There is no
`status` field in a ticket's frontmatter. Each verb invocation is a commit
carrying a `Planr-Verb` trailer (§3.5), and a ticket's state is
`fold(schema.verbs, events(ticket))` — the `to` of its most recent declaration,
seeded by the derived initial state. What made this the right call is
**configurability**: a stored status is data a schema edit can silently
invalidate, with no migration story, and the checking that would catch the drift
degrades exactly where custom schemas make it most likely. Under a fold the
schema stops being a validator over stored data and becomes the *interpreter* of
history — rename a state, add `qa`, reorder the machine, and history is
reinterpreted rather than left stale.

Note the state machine survives this; only its *storage* dies. `request-changes`
returns a ticket to `in_progress`, which is not its own name, so a verb-to-state
mapping is still needed — that mapping is `from`/`to`, promoted from arguments
of a primitive to attributes of the verb (§3.4).

**Two layers, and only one of them needs the schema.** Some of the lifecycle is
readable from repository structure alone:

| layer | facts | needs the schema? |
|---|---|---|
| **structural spine** | present / archived (file in tree); open / closed (a close-class declaration); in-progress (a `plan/<kind>/<slug>` ref exists); done / abandoned (closed *with* a merge / *without* one) | **no** |
| **refinements** | `review`, `approved`, `qa`, … — all subdivisions of `open` | yes |

The derivable three — untouched, claimed, absorbed — are exactly git's own
ontology for a branch, which is why `claim` and `close` kept wanting to *be* the
transition. What isn't derivable are the **speech acts**: `review` is "I, the
worker, assert this is complete"; `approved` is "I, the reviewer, judge it
acceptable"; `abandoned` is "I, the leader, decide this shouldn't happen." Git
represents work, not judgments about work, so those live as declarations. The
payoff of the layering is that an agent with git and no schema can still read
the spine; only the refinements require the fold.

**The lifecycle is not authored — it is entirely emergent (R6).** There is no
`lifecycle` block in the schema. The state set is *derived* as the union of every
verb's `from`/`to`; **each kind's sub-machine** is derived from the transitions
of verbs whose `applies-to` includes that kind; the **initial state** is derived
as the state that appears as some verb's `from` but never as any verb's `to`
(zero or several such states is a lint error); and `terminal` derives as below. So
`task`, `epic`, and `milestone` each get a *different* lifecycle for free — a
`task` runs `todo→in_progress→review→approved→done`, an `epic` only
`todo→done` (no verb gives it the review states), a `milestone`
`planned→in_progress→released` (§3.4). Sharing a label like `in_progress` across
kinds is harmless — `self: {status: …}` reads one ticket's status; the per-kind
sub-machine governs legality.

Because nothing is declared, **validation is by derivation, not redundancy**:
- **`lint` flags anomalies** — the from-less-sole-exit *freeze* (error, see
  guardrail below); an **unreachable** state (no incoming transition and not a
  template initial → typo or dead); a **reachable state with no exit** that is
  newly terminal (surfaced so you notice — this catches e.g. a `qa` verb whose
  target state has no exit verb).
- **`planr lifecycle [kind]`** (fixed read tooling, like `board`/`graph`)
  *renders* each kind's derived sub-machine as text + optional mermaid, so you
  match it against intent by inspection — always-correct, never drift-prone.

`approved` is a real state between `review` and `done` (R3): the review outcome
is modeled as a transition, not a parsed free-text field, so the close gate is a
pure `self: {status: approved}` check and the board can distinguish "in review"
from "approved, awaiting merge".

**`blocked` is not a state — it's derived (or commentary).** Two meanings,
neither a status: *(1) blocked-by-dependency* is the exact negation of `claim`'s
gate — `blocked(T) = ∃ d ∈ depends_on(T): status(d) ≠ done` — so it is a derived
`board`/`lint` view, never stored (storing it would denormalize the graph and
drift; and it automatically flags the R2 abandoned-dep decision, since an
abandoned dep also fails `≠ done`). *(2) blocked-by-external-reason* ("waiting on
vendor") is orthogonal to the lifecycle — it changes no legal transition — so it
is commentary (`annotate` a note), or a project-specific schema extension, never
a core status. A fixed-target transition also can't express "unblock to
wherever you were," which is the tell it was never a state.

There is a **third** meaning those two miss, and it needs a verb rather than a
state: *(3) the worker discovered the plan is wrong and is handing the ticket
back.* Unlike (2) it is not orthogonal to the lifecycle — work stops and the
supervisor picks it up. But the "unblock to wherever you were" objection
dissolves here, because handing back **is** relinquishment: the target is
exactly `todo`. So it needs no new state, only a verb symmetric to `claim` —
`yield` (§3.4), which annotates a `## Blocked` section and returns the ticket to
`todo`. See §4.1 for why this matters: it is one of the four things an actor can
do on discovering an unforeseen problem, and the design's job is to make the
other three cheap enough that "keep going and hope" is never the path of least
resistance.

Transitions live *on the verbs* that drive them (§3.4), never as a free-floating
table; the machine is **enforced only structurally** (via each verb's
`require`).

**`terminal` is derived, not declared.** A terminal state is one with no
outgoing transition in the verb graph — computed (and cached) at runtime. Note
`archived` is deliberately **not** a lifecycle
state — archival is a storage relocation (live tree → git history, §5),
orthogonal to status; an archived ticket keeps whatever terminal status it had.

The derivation looks circular — `terminal(S)` means "no transition leaves `S`,"
yet a `from`-less transition (e.g. `abandon`) appears to leave *every* state
(R4). It resolves by **stratification**: `from`-less transitions are *consumers*
of the terminal set, never *contributors* to it.

1. Derive terminal from **explicit-`from` transitions only** (ignore `from`-less
   verbs): `terminal = { S : no verb declares from: S }`.
2. *Then* expand each `from`-less transition to "originates from every state
   ∉ terminal."

Step 2 never feeds step 1, so it is non-circular; it is sound because the
**absorbing rule** *defines* a `from`-less transition not to originate in a
terminal state — so it only ever fires from states that already had an explicit
exit (already non-terminal) and can never flip a terminal state to non-terminal.
Worked example: with explicit `claim`/`submit`/`approve`/`request-changes`/
`close`/`qa` and `from`-less `abandon`, step 1 sees no explicit
`from: done|abandoned` → those are terminal (computed without ever looking at
`abandon`); step 2 lets `abandon` fire from the non-terminals (todo,
in_progress, review, approved) but not from `done`. So `terminal =
{done, abandoned}`.

**Lint guardrail:** a `from`-less verb may not be a state's *sole* exit — else
step 1 marks the state terminal, step 2 refuses to apply the verb, and the state
silently freezes. So every intended-non-terminal state must have ≥1
explicit-`from` transition; any state with no explicit outgoing transition is
*declared* terminal, and if that is wrong you add an explicit-`from` verb.

### 3.4 `verbs` — one declaration, one commit

A **verb** is a named lifecycle mutation. `board`/`lint`/`graph`/`brief` are
*not* verbs — they are fixed read tooling (the tool's spine). Verbs are a
**list**, each entry selected by `(name, applies-to)`.

Every verb has the same shape, and it is a shape with no ordering in it:

```
require  ->  build a tree from BASE  ->  move the TARGET ref
```

`require` gates. The content steps, if any, transform the base's tree into one
new commit. `effect` says which ref moves. Because content steps are pure tree
transformations and ref movement is a separate phase, **there is nothing to
sequence** — which is what dissolved two rounds of argument about whether
`claim` and `close` flip before or after their git work. The earlier model put
tree edits and ref moves in one ordered `do` list, and that conflation was the
bug, not the ordering:

```yaml
verbs:
  - name: claim
    applies-to: [task]
    from: todo
    to: in_progress
    require: { neighbors: { depends_on: done } }   # deps must have SUCCEEDED, not merely be terminal
    base: home                        # build the declaration against the integration ref...
    effect: create                    # ...then cut plan/task/<slug> at that commit
    worktree: create                  # declaring this is what makes `task` the unit

  - name: submit                      # worker: "ready for review"
    applies-to: [task]
    from: in_progress
    to: review
    require: { sections: [validation] }   # a ## Validation section exists (structural)
    base: own                             # no content -- the commit message is the whole payload

  - name: approve                     # reviewer: the outcome is a STATE, not a field
    applies-to: [task]
    from: review
    to: approved
    base: own
    content: [ annotate: { section: Review, body: $message } ]

  - name: request-changes             # reviewer: bounce back to the worker
    applies-to: [task]
    from: review
    to: in_progress
    base: own
    content: [ annotate: { section: Review, body: $message } ]

  - name: yield                       # "I found something; this needs re-planning"
    applies-to: [task]
    from: in_progress
    to: todo
    base: own
    content: [ annotate: { section: Blocked, body: $message } ]

  - name: close                       # unit variant: integrates a branch
    applies-to: [task]
    from: approved
    to: done
    require: { self: { status: approved } }   # folded from the branch's own events (§4.1)
    base: own                                 # build the declaration on the branch tip...
    effect: merge                             # ...then merge it into home, as one ref move
    worktree: remove

  - name: close                       # container variant: gate only, no merge
    applies-to: [epic, story]
    from: todo
    to: done
    require: { neighbors: { children: terminal } }   # an abandoned child is a resolved decision
    base: home

  - name: abandon                     # from-less: the "any non-terminal state" verb
    applies-to: [task, story, epic, milestone]
    to: abandoned
    content: [ annotate: { section: Abandoned, body: $message } ]

  - name: archive                     # relocate to history; NOT a state change
    applies-to: [task, story, epic, milestone]
    require: { self: { status: terminal } }
    content: [ remove ]

  # --- edge mutation (leader backlog restructuring, R1/R2) ---
  - name: reparent
    applies-to: [story, task]
    content: [ edge: { set: { parent: $target } } ]      # no `to` -- the state is unchanged

  - name: add-dep
    applies-to: [task]
    content: [ edge: { add: { depends_on: $target } } ]

  - name: drop-dep                    # resolves the abandoned-dependency decision (R2)
    applies-to: [task]
    content: [ edge: { remove: { depends_on: $target } } ]

  - name: assign                      # (un)assign a milestone
    applies-to: [epic, story]
    content: [ edge: { set: { milestone: $target } } ]

  # --- milestone lifecycle (a non-spine `groups` kind, R6) ---
  # `planned` is DERIVED as the initial state: it is a `from` that is never a `to`.
  - name: start
    applies-to: [milestone]
    from: planned
    to: in_progress

  - name: release
    applies-to: [milestone]
    from: in_progress
    to: released
    require: { neighbors: { members: terminal } }

  - name: qa                          # example project extension
    applies-to: [task]
    from: approved
    to: qa
    hook: { run: "./ci.sh" }
```

Every state change flows through a verb — including the ones 0.3 did as **manual
file edits** (the worker hand-setting `status: review`, the reviewer hand-writing
the verdict). In 0.4 those become `submit` / `approve` / `request-changes`, which
is strictly better: they get atomic commits and structural guards too. `approve`
*declares* the ticket `approved`; `close`'s `require: {self: {status: approved}}`
reads that state as folded from the branch's own events — a pure graph fact, so
the verbs interlock without parsing a free-text verdict (R3).

**Several verbs have no content at all.** `claim` cuts a ref; `submit` and unit
`close` change no bytes. Their commit *message* is the entire payload. So a verb
is not "one content mutation" but **one declaration, optionally carrying
content**, and empty commits become first-class and meaningful rather than a
smell. It is also what makes the fold total: every state change has exactly one
commit to point at, even the ones that touch nothing. The one caution is that an
empty declaration with no precondition is precisely the shape that lets an actor
plough on and hope, so `submit` keeps its `sections: [validation]` gate — it
reads content it does not write.

Decisions baked in:

- **`base` and `effect` replace the old ordered `do` list (round-3).** `base` is
  the ref the verb's commit is built on — `home` (the ticket's integration ref:
  trunk, or the nearest ancestor holding an open integration branch) or `own`
  (`plan/<kind>/<slug>`). `effect` is the single ref movement: `advance` (the
  base moves), `create` (cut the ticket's ref at the new commit), `merge`
  (integrate into `home`), `delete` (drop the ref without integrating). Trunk is
  a *value* resolved by walking up `parent` edges, never a constant baked into a
  primitive — which is what leaves container-integration branches (§9b) a later
  schema choice rather than a later rewrite.
- **Three ref-algebra invariants** follow and are checked by the published schema
  itself: `effect: create` requires `base: home` (you can only cut a ref from the
  ref the ticket already lives on); `effect: merge` requires `base: own` (only a
  ticket's own ref integrates into its home); and `worktree: create` requires
  `effect: create` (a worktree belongs to a ref, so making one without cutting
  that ref is incoherent).
- **`require`** is a **separate key**, not a content step — it is a precondition,
  evaluated before any side effect. Its vocabulary is structural predicates
  only: `{field: value}` on self (`status: approved`) and aggregates over
  edge-neighbors (`children: done`). No semantic judgment (the pin).
- **`applies-to`** selects which kinds a verb definition serves. The same
  `name` may appear multiple times with **disjoint** `applies-to` — this is how
  `close` overloads (merge on units, gate-only on containers). The CLI surface
  stays `planr close <slug>`; the engine resolves `(name, kind-of-slug)` to one
  definition. Overlapping `applies-to` for one name is a lint error.
- **`from`/`to` are attributes of the verb (round-3).** They used to be
  arguments to a `transition` primitive. But `from` is *structural* — it is what
  makes the per-kind sub-machine well-formed and `terminal` derivable — and
  structural facts about a verb belong on the verb. With state no longer stored,
  `to` is not an instruction to write anything; it is what the fold yields. A
  verb that changes no state (an edge mutation) simply omits both.
- **`from` and `require` are orthogonal, and `from` is the norm (R4/round-2).**
  `from` declares a transition's *source state* — structural, and what makes the
  per-kind sub-machine well-formed and `terminal` derivable (§3.3). **Every
  progression verb that is the sole exit of its source state must declare
  `from`** — that's `claim` (`todo`), `submit` (`in_progress`), container-`close`
  (`todo`), `approve`/`request-changes` (`review`), `close` (`approved`), the
  milestone verbs. `from`-less is *not* the ergonomic default; it is reserved for
  the **"any non-terminal state"** verb — `abandon` — which is never a sole exit,
  so it can't freeze a state. (This corrects an earlier "prefer `require` over
  `from`" framing: `require` adds a *precondition*, it never substitutes for the
  structural `from`.)
- **Per-kind capability is derivable**: an epic offers no `claim` because no
  `claim` verb applies to it. The board can list available actions per ticket
  for free.

### 3.5 Primitives — content transforms and ref effects

The binary owns a small, fixed vocabulary; verbs compose it. The set is where the
enforce/don't-enforce line is frozen — there is no primitive that evaluates
quality. Round-3 roughly halved it by separating two things the old
ten-primitive list conflated: **content primitives produce a tree; ref effects
move a pointer.** Ten loosely-typed primitives in an ordered list became three
content transforms plus four ref effects, and ordering stopped existing as a
concept because there was nothing left to order.

**Three content transforms.** Pure `(tree) → tree` functions. They know nothing
of HEAD, refs, or worktrees; the verb's `base` and `effect` decide where the
resulting commit is built and which ref moves. All of a verb's content steps are
staged into its single commit. Each guards its own invariant — that is why they
are typed rather than one generic `set`.

**`annotate`** writes a **named section with a templated body**:

```yaml
- annotate: { section: "Abandoned", body: "$message" }
```

Reviewer prose still goes in a `## Review` section, but it is *commentary*, not a
gate. Use `annotate` for material a future reader must act on — review findings
persist and get worked from. Commentary that nothing downstream acts on belongs
in the commit message instead; an abandon reason is read once and the file is
about to leave the tree.

**`edge`** writes a **forward edge field on the owning node** (R1) —
`set` (single-valued: `parent`, `milestone`), `add`/`remove` (multi-valued:
`depends_on`). `link` is *not* in `edge`'s domain — it is body `[[..]]` prose,
authored inline (§3.2), not a frontmatter edge:

```yaml
- edge: { set:    { parent: $target } }
- edge: { add:    { depends_on: $target } }
- edge: { remove: { depends_on: $target } }
```

It enforces edge validity as a precondition — the same checks `lint` runs
(double duty): **target exists**, **adjacency legal** (a new `parent`'s kind is
allowed by the `kinds` graph), **acyclic** (`parent`/`depends_on` must not form
a cycle), **cardinality** (`set` on single-valued, `add`/`remove` on
multi-valued; cardinality is declared per edge in the `edges` schema). You only
ever write the *forward* field from the *owner* — inverse roles (`children`,
`members`) stay read-only derived and are never written, which is why edge
mutation sidesteps the R9 naming question entirely.

**`remove`** deletes the ticket file. This *is* archival (§5): a tree change,
not a git operation and not a state change. Classifying it here is the honest
call — `git rm` was never in the same category as `merge`.

**Four ref effects**, exactly one per verb, each with a defined failure mode
(R11):

- **`advance`** — the commit lands on `base` and the base ref moves. The default,
  and what every edge-mutation verb does.
- **`create`** — cut `plan/<kind>/<slug>` at the new commit. Because the commit
  is built *before* the ref exists, the branch springs into existence already
  carrying the claim; there is no instant at which the ref exists but the ticket
  is unclaimed. `git branch` is an atomic create-or-fail, so two agents claiming
  one ticket concurrently resolve by **ref CAS** — the loser gets "branch already
  exists" and its commit is garbage-collected. No lock is needed for `claim` at
  all.
- **`merge`** — integrate the new commit into `home`. On conflict, **abort and
  print rebase guidance** (0.3 behaviour); the target is left untouched, and the
  built commit was never referenced, so a re-run after the human rebase simply
  rebuilds it. The ticket's own ref is deleted afterwards by default, since the
  commits are reachable from `home` either way; `retain-ref: true` keeps it for
  anything outside planr that still needs it, such as a code-forge review record.
- **`delete`** — drop the ticket's own ref without integrating it. Idempotent: a
  missing ref is a no-op, not an error.

**`worktree`** (`create` / `remove`) is a **key on the verb, not an effect** — a
worktree is ephemeral local workspace that never enters history, and treating it
as a peer of `merge` is what made the unit look like a declared property rather
than a derived one. It is idempotent in both directions, so a re-run after a
partial failure completes. Its path comes from the `worktrees` template (§4).

**`hook`** remains the escape hatch (§3.9), in its own slot rather than a list
position — opt-in, rare, kept off the main path so a stateless agent can still
read what a verb does.

**What left the set.** `transition` was promoted to the verb's `from`/`to`
(§3.4). `archive` became the `remove` content transform. `new-worktree` and
`branch`/`cleanup` collapsed into the `worktree` key and the ref effects.
`brief` left entirely: it is read tooling like `board` and `lint`, and it was
only ever in the primitive list because the list was the only place to put
things. `commit` was never a primitive — it is the verb boundary.

**Unwind-on-failure is now true rather than aspirational.** The old model claimed
"a verb that fails before its commit leaves nothing behind," but `claim` created
a worktree and a branch *before* its commit, so a failure in between stranded
both — and re-running hit create-or-abort, making recovery manual. Under
build-then-move, everything before the ref operation is unreferenced object
construction, which git garbage-collects. The one residual partial state is a
`worktree: create` that fails after the ref was cut; `worktree` idempotency
covers the re-run.

**The event log lives in commit trailers.** A verb's commit carries exactly two:

```
Planr-Verb: approve
Planr-Ticket: verb-runner
```

`Planr-Verb` is irreducible — `approve` and `request-changes` produce identical
content shapes, so only the declaration distinguishes them. `Planr-Ticket` is
derivable from changed paths in the common case, but recorded explicitly so the
event chain survives renames and so an archival commit, which deletes the path
outright, stays attributable. The commit subject is conventionally
`plan: <verb> <slug>`, asserting no state — 0.3's `plan: claim <slug>
(in_progress)` parenthetical is gone, because the state is computed rather than
declared.

**There is deliberately no schema trailer.** An earlier draft proposed recording
a schema identifier per event so a later schema edit could not silently
reinterpret history. It is unnecessary: `.plan/schema.yml` is tracked in the same
history, so every event commit's tree already carries the schema in force when
that event was declared — `git show <event>:.plan/schema.yml`, with
`git log -- .plan/schema.yml` giving the timeline so the lookup only repeats when
the schema actually changed. Per-event granularity, zero bytes, and it cannot
drift from the thing it describes. It also gets the two-lane semantics right for
free: a task branch's events are interpreted under the schema as of the branch
point — what the worker could actually see — so a leader's mid-flight schema edit
on trunk does not retroactively rewrite what the worker meant.

**The schema language itself is versioned by URL**, not by hash:

```yaml
# .plan/schema.yml
$schema: https://schemas.columnzero.com/planr/v1/planr.schema.json
```

The registry distinguishes two URL forms, and the distinction matters: a
**canonical** URL carries the full version and never moves, while an **alias**
carries only the major line and moves forward with each release. The document's
own `$id` is canonical, because `$id` is a permanent identity and an alias would
name a different document after the next release. A *project* cites the alias, so
compatible releases do not churn every repo. A validator fetching the alias
therefore gets a document whose `$id` differs from the retrieval URL — intended
registry behaviour, since the embedded `$id` establishes the base URI.

A content hash would be *too* precise — a typo fix in a comment would read as a
different schema. The URL is a stable, dereferenceable identity with room for
compatible evolution; the registry entry carries the sha256 separately, which is
the right split, since identity wants stability across cosmetic edits and
integrity wants byte-exactness. The document is a JSON Schema 2020-12 file whose
root validates `.plan/schema.yml`, with `#ticket` and `#commit` anchors for
frontmatter and the trailer block, so it is a *validator* and not merely a label.
It ships in-tree at `schemas/planr/v1/1.0.0/planr.schema.json`: **planr never
dereferences it at runtime**, so the tool works offline, air-gapped, and in CI.
An unrecognized major version is a clear "this backlog declares planr schema v2;
this binary understands v1" error, never a download. The version tracks the
*language*, not the binary — planr 0.4 to 0.5 does not move the URL unless the
key set or a primitive's meaning changed.

### 3.6 `require` — the gate predicate vocabulary

A predicate is conceptually a **pure, referentially-transparent boolean check**
over the graph — so its result is derivable and cacheable, and every predicate
is a **built-in** computed straight from the graph (never a spawned process).
`require` has **no custom-hook escape hatch**: impure or bespoke gating belongs
in the verb's `hook` slot (which vetoes by exit code all the same, §3.9). Note
that `self: {status: …}` reads the *folded* state (§3.3), not a stored field —
the predicate is a graph fact either way, which is exactly why the fold changed
nothing here. Keeping `require`
purely graph-derived is what lets it do double duty as a `lint` invariant
without ever executing anything.

**Every `require` key is a reserved operator** (a documented, closed set);
schema-defined names (edges, fields, section names) appear only as *arguments*.
This removes the ambiguity of bare field names — a reader always knows a
`require` key is a planr predicate to look up, never a possibly-mistyped field.
Three operators:

```yaml
require:
  self:      { status: approved }       # attribute(s) of THIS ticket equal / in a set
  neighbors: { depends_on: done }       # ∀ direct neighbors along an edge are in a state/set
  sections:  [ validation ]             # these body sections must exist
```

Entries combine by **implicit AND**. Operators are validated by `lint` (an
unknown operator is a lint error — that is what makes "reserved" real); so are
arguments (an edge/section/field the schema doesn't define). `sections` checks
**existence, not content** (a worker satisfies it with an empty `## Validation`)
— deliberate: content quality is the reviewer's semantic call, not the tool's
(the enforce-structure-never-semantics pin). Coverage proof:

| verb | require |
|---|---|
| `claim` (task) | `neighbors: {depends_on: done}` |
| `submit` (task) | `sections: [validation]` |
| `close` (task) | `self: {status: approved}` |
| `close` (container) | `neighbors: {children: terminal}` |
| `release` (milestone) | `neighbors: {members: terminal}` |
| `archive` | `self: {status: terminal}` |
| `qa` | `self: {status: approved}` |
| `approve`, `request-changes`, `yield`, `abandon`, edge verbs | none |

**Double duty — shared operators, plus a quantifier (R5).** The same operators
power `lint` as *standing invariants*, but a lint invariant is a statement over
*all* tickets, so it needs a **guard** to select the population and the ∀ a
single-ticket verb precondition doesn't. A lint rule is a `when`/`must` pair,
**both built from the exact same operators**:

```yaml
invariants:
  - when: { self: { status: review } }   # guard: selects the population
    must: { sections: [validation] }      # assertion: held ∀ selected ticket
```

So the honest claim is *shared vocabulary*, not identical grammar: the operators
build both guard and assertion; `lint` adds `∀ ticket where when(t): must(t)`. A
verb precondition is the **degenerate case** — guard = the one ticket the verb
targets (narrowed by `applies-to`/`from`), assertion = `require`.

Many invariants are also **derivable** from verb requires: if the only verb
entering a state `S` requires a *durable* predicate (sections/edges), then every
`S` ticket satisfies it — so `lint` can surface the `submit → review` invariant
above for free and catch anyone who hand-edited around the verb. `lint` likewise
surfaces the abandoned-dependency decision (below) as a standing check.

**Deliberate exclusions** (each safe because no gate needs it):

- **No transitive closure.** Gates check *direct* neighbors only. The DAG
  invariant is maintained edge-by-edge (B can't be `done` unless B's own deps
  were resolved when B was claimed), so a direct `depends_on` check transitively
  implies the rest. Transitive queries are a *view* concern, never a `require`.
- **Only ∀** — no ∃, no counts, no "is a leaf" (`unit` is kind-based).
- **No disjunction / no nesting** — map is AND; OR is handled by two verb defs
  with disjoint `applies-to`, or a `hook`.
- **No negation** — set membership covers positive cases; "can't leave a
  terminal state" is enforced by the transition layer (terminal states have no
  outgoing edges), so `abandon` needs no `require`.
- **Tags govern *automatic* behavior, not what a verb may gate on.** The
  semantics tag makes containment auto-roll-up and makes group edges auto-block
  *nothing*; but a verb may add an **explicit** `require` over **any** edge
  (still neighbor-∀ over one direct edge — no new machinery). So a milestone
  stays non-blocking automatically, yet a `release` verb can opt into
  `require: {members: terminal}` (§3.2). No edge allow-list.
- **Values are literals or the derived set** `terminal` (§3.3) — never
  expressions. Anything relational (`child.assignee == self.assignee`) is a
  `hook`, not `require`.

**Needs vs. decomposition — the neighbor gates are asymmetric.** An abandoned
neighbor is treated differently depending on *why* the edge exists:

- **`depends_on` is a *needs* relationship** → gate on `done`, not `terminal`.
  Only successful completion satisfies a need; an abandoned dependency does
  **not** — it **blocks** the dependent, surfacing a required decision (the edge
  was wrong and should be dropped, or the dependent must itself be abandoned /
  redesigned). Auto-unblocking would erase a decision that has to be made
  per-dependent. `lint` surfaces "depends on an abandoned ticket" as a standing
  invariant.
- **`parent`/`children` and milestone `members` are *decomposition/grouping*
  relationships** → gate on `terminal`. Abandoning a child *is* a resolved
  scoping decision, so it doesn't block the parent's close.

(This corrects an earlier draft that used `terminal` for both. 0.3 gated
container-close on children `== done`, which lets an abandoned child block its
parent forever — that half was right to change; deps were not.)

### 3.7 Templating

Primitive string arguments (`annotate` bodies, commit trailers, `hook` args)
flow through **simple `$var` substitution over a fixed context** — *no*
expressions or logic (KISS, and so a stateless agent can read a verb without
evaluating a DSL). The variable namespace is bounded and documented; starting
set: `$message` (CLI-supplied), `$target` (the other end of an edge mutation),
`$slug`, `$kind`, `$title`, `$date`, `$actor`, `$branch`.

**Availability is context-dependent.** Not every var is defined for every verb:
`$branch` is undefined during `new`/creation (the branch is the claim, which
happens later), so template bodies must not reference it. A verb referencing a
var not in *its* context is a `lint` error — the same reserved-name discipline
as operators.

> **Open:** finalize the `$var` namespace and each verb's available subset.

### 3.8 Creation: `new` is fixed tooling + a `templates` schema key

Creation is *genesis*, not a lifecycle mutation — it has no prior node and no
from-transition — so `new` stays **fixed tooling** (like `board`/`lint`), not a
verb, and no `scaffold` primitive is needed. Per-kind starter content is
declared in a dedicated schema key (working name **`templates`**, echoing 0.3's
`templates/` dir; `init` was considered but risks colliding with
workspace-initialization semantics):

```yaml
templates:                            # one entry per kind — spine kinds AND groups
  epic:      { body: "## Goal\n\n## Context\n\n## Stories\n" }
  story:     { body: "## Goal\n\n## Context\n\n## Tasks\n" }
  task:      { body: "## Goal\n\n## Acceptance\n\n## Validation\n" }
  milestone: { body: "## Goal\n\n## Exit criteria\n" }
```

**No `status` key (round-3).** An earlier draft made the template the sole source
of a kind's initial state, on the grounds that the derived lifecycle couldn't
infer it. It can: the initial state is **the state that appears as some verb's
`from` but is never any verb's `to`** — `todo` for a task, `planned` for a
milestone. Zero or several such states for a kind is a `lint` error, in the same
family as the from-less-sole-exit freeze (§3.3). With state folded from events
rather than stored, a ticket with no events *is* in its initial state, so there
was never anything to write down.

Templates are therefore pure scaffolding, and a kind without one simply gets an
empty body; `lint` still flags the omission as probably unintended.

`planr new <kind> <slug> <title>` reads `templates.<kind>` to scaffold, with the
same `$var` substitution applied to the body.

### 3.9 Hook contract

Hooks live **only in a verb's `hook` slot** — there is no hook in `require`
(§3.6). A
hook is a subprocess whose exit code is a one-bit signal (`0` = proceed) and
which *may* cause external effects (CI, API, build). It is the sole extension
point for impure or bespoke gating: put the check in the verb's `hook` slot and let its
exit code veto the verb.

**Threat model: defend against mistakes, not adversaries.** A hook script lives
*in the repo* — same trust boundary as the source the worker compiles and the
tests the reviewer runs. Anyone who can write a hook can already write the build
script or the code under test, so sandboxing against a *malicious* hook guards a
door in a wall that isn't there. The real risk is a *buggy* hook that
accidentally corrupts the graph — a far weaker adversary.

**Integrity via git tamper-evidence: detect, don't prevent.** The verb engine
snapshots git state, runs the hook, and verifies the hook left no unexpected
modification to **tracked** `.plan/` files; if the tree came back dirty in a way
the verb didn't author, it aborts and reports. (The derived index is
**git-ignored** and lives outside the tracked set — §5.1 — so a legitimate index
rebuild during a hook does not trip the check; only tracked ticket/schema files
are watched.) Git makes any write *visible*, and the engine refuses to build on
an unauthored change — a *practical* read-only guarantee without the
(theoretically impossible) *prevention* one. No bwrap, no container,
no shimmed `planr` required; git is already the substrate. An **opt-in sandbox**
(`hook: {run: "./ci.sh", sandbox: true}` → bwrap with ro-mounted `.plan/` where
available) is defense-in-depth, never required. The scripting-engine route (an
embedded sandboxed interpreter) is rejected: large cost for a guarantee
git-detection gives more cheaply, and it destroys the escape hatch's whole value
— that a hook is *the user's own script in their own language*, observable and
same-trust as the repo.

`hook` is otherwise **an external validator with a one-bit veto and optional
text output, never a graph mutator:**

- **Read-only graph access, through the same door as everyone else.** No SDK, no
  in-process graph handle. A hook receives context via **environment**
  (`$var` namespace exported as `PLANR_SLUG`, `PLANR_KIND`, `PLANR_BRANCH`,
  `PLANR_STATUS`, `PLANR_TRUNK`, `PLANR_DIR`, `PLANR_MESSAGE`) with **cwd = the
  ticket's worktree**, and it queries by shelling to `planr` read commands
  (`board`, `graph`, future `query`). Graph access stays in the fixed read
  layer; hooks get no privileged path.
- **Non-zero exit = the verb fails.** A hook's *only* channel back into graph
  state is its exit code — one bit, proceed or veto. Because the verb's content
  mutation lands as a single commit at the verb boundary, a hook that exits
  non-zero aborts *before* that commit, so the transition never persists (clean
  content rollback). A hook runs after `require` and before the
  ref effect, so a veto aborts before anything irreversible.
- **A hook may not mutate the graph.** It may inspect, cause *external* effects
  (CI, API calls, builds), and veto — but it cannot edit tickets or drive
  transitions. State changes flow only through primitives, preserving the
  atomic-commit model and structural enforcement.
- **Producing data for the ticket** (a coverage number, a build hash) is
  recovered without breaking the rule: capture the hook's stdout and feed a
  following `annotate` — `[hook: "./cov.sh", annotate: {section: Coverage, body:
  $output}]`. The hook produces text; a *primitive* persists it.

A stateless agent reading `hook: "./ci.sh"` knows the *shape* of the effect
(runs that script, exit gates the verb) without reading it; the script is
in-repo (observable, same trust boundary as the code being built).

## 4. Filesystem layout

- **Flat `tickets/<slug>.md`.** Slug is the identity; **no numeric prefix**
  (removes an arbitrary layer of indirection *and* removes the `planr new`
  prefix-allocation `flock` — one of the two operations that needed
  serialization; only trunk merge in `close` still serializes).
- **Branch refs are `plan/<kind>/<slug>`** (round-3), not flat `plan/<slug>`.
  Slugs contain no `/` and refs are always three segments, so git's
  directory/file ref conflict can never arise. It makes `board` *cheaper*: the
  ref itself names the kind, so enumerating `plan/story/*` needs no blob reads.
  The invariant it introduces: **kind is immutable for a claimed ticket**, since
  the ref is derived from it. No kind-change verb exists; if one is ever added
  it must refuse on a claimed ticket.
- **Worktrees live at `.plan/worktrees/$kind/$slug`** by default, and the path is
  a configurable template (`worktrees:`, §3.4). 0.3 already had both the in-repo
  default and the override — an earlier 0.4 draft froze `../wt-<slug>` into the
  primitive and dropped the knob, which was a regression, not a simplification.
  In-repo is the better default because a sibling path sits *outside* the
  sandbox boundary agent harnesses commonly enforce, and because two clones
  sharing a parent directory both want `../wt-foo`. It must be gitignored; the
  cost is that recursive indexers see N checkouts, which is the honest reason to
  point the key elsewhere.
- **All structure lives in frontmatter** (kind, title, `parent`, group edges,
  `depends_on`) — and *only* that. `status` is folded from events (§3.3); `id`
  went when the slug became the identity; `created`/`updated` are the commit
  timestamps of a ticket's first and last events. What remains is exactly what
  git cannot derive: what kind this is, what it is called, and what it is
  connected to. The published `#ticket` schema rejects all four removed fields by
  name, so a half-migrated backlog fails loudly. The directory no longer encodes
  the kind. Every
  grouping/view is derived at read time. **The multi-valued edge `depends_on` is
  stored as a block list (one target per line)**, not inline `[a, b]` — so
  concurrent `add-dep` of *different* targets land on different lines and git
  auto-merges (see optimistic concurrency below). (`link` is *not* a frontmatter
  edge — see §3.2.)
- **Optimistic concurrency, git as the detector.** Edge mutation (`reparent`
  etc., §3.5) edits a ticket's frontmatter on trunk; if that ticket is also
  claimed, its file lives on a branch too. No lock and no version field: **git
  is the optimistic-concurrency mechanism** — the merge detects any clash. With
  verb↔commit correspondence + field-level granularity, most cases *auto-merge*
  (a `reparent` touches the `parent:` line, a `submit` the `status:` line —
  different lines). A true conflict needs two edits to the *same* field (two
  concurrent `reparent`s of one ticket), which is rare (hours-to-days apart, not
  milliseconds) and one line — resolved by the existing rebase-on-`close` path.
- **Consequences we like:**
  - Milestone membership is a frontmatter field, not a location →
    "move an epic into a milestone" is a one-field edit, conflict-free on any
    branch. Epic 07's "filesystem-only placement operations" story evaporates.
  - `branch = claim`, files-merge-with-code, and PR-observability are all
    preserved (still one md file per ticket).
- **Cost:** you lose `ls stories/` browsability. Recover it via `planr
  board`/`graph` views (same cheap derive-at-read-time), grep, and optionally a
  *generated, git-ignored* symlink view tree (`views/by-milestone/v1/…`).

### 4.1 Refs, actors, and the two lanes (R7)

Every verb lives in one of **two lanes**, and lane membership is **derived from
which ref the verb writes** rather than enumerated by name — an enumerated list
breaks the moment the unit moves up a level or a container opens an integration
branch.

- **Branch lane — per-unit, parallel, lock-free.** Verbs whose `base` is `own`:
  `submit`, `approve`, `request-changes`, `yield`, plus `abandon` / edge-edits
  *of a claimed ticket*. Run by the worker or reviewer in the unit's worktree;
  commit to `plan/<kind>/<slug>`; touch only **the unit's ticket file and its
  sub-unit descendants**. N units = N branches = zero contention. This is where
  all parallelism lives — no serialization ever.
- **Integration lane — structure and integration, serialized.** Verbs whose
  `base` is `home`: `new`, container `close`, `archive`, `release`, and
  `abandon` / edge-edits *of an unclaimed ticket*. `home` is trunk today; if a
  container ever opens an integration branch (§9b) the lane generalizes with no
  rewording, which is why it is no longer called the "trunk lane".

The widened branch-lane invariant — the unit's file *and its sub-unit
descendants* — costs nothing, because the whole sub-unit subtree is exclusively
owned by that one branch. Exclusive ownership was always the property doing the
work; "one ticket file" was just the special case where the cut sat at the leaf.

**`claim` spans the lanes and needs no lock.** It builds its declaration against
`home` and creates `plan/<kind>/<slug>` at that commit. Because the commit exists
before the ref does, the branch springs into existence already carrying the
claim, and `git branch`'s atomic create-or-fail resolves two concurrent claims of
one ticket by ref CAS (§3.5). 0.3 needed a lock here partly for prefix allocation,
which the flat layout deleted; this removes the rest of the reason.

**`close` bridges the lanes** and dissolves the apparent chicken-and-egg: it
folds the task's state *from its branch* (`git show plan/task/<slug>:...` plus
that branch's `Planr-Verb` trailers — 0.3's board already reads branches this
way), gates on `approved`, **builds the `done` declaration on the branch tip,
then merges that into `home`** — so the terminal state rides into trunk *with*
the work, as one integration, never a trailing trunk-only edit. The declaration
is built as a commit on the branch tip and merged in; it never checks out the
branch or moves its ref, so the one-branch-one-worktree rule never binds. The
worktree and ref are then released.

Why flip-then-merge rather than merge-then-flip, precisely — because the earlier
justification was the weakest of the available arguments and kept failing to
stick:

- **Revert atomicity (the real reason).** `git revert -m 1 <merge>` undoes the
  work *and* the `done` in one operation, leaving a ticket that correctly needs
  integrating again. Under merge-then-flip, reverting the merge leaves a ticket
  marked `done` with no work behind it, and consistency takes two reverts. The
  declaration belongs on the side of the DAG that gets reverted with the work.
- **One commit per close in trunk's first-parent history.** Both orders add two
  commits to reachable history, but merge-then-flip puts a bookkeeping commit in
  the first-parent line, which is the history humans read.
- **The fast-forward case stays clean.** If trunk hasn't moved, flip-then-merge
  fast-forwards straight to the declaration — no merge commit at all. The other
  order fast-forwards and then adds a commit on top.
- **Not atomicity, strictly.** Built with plumbing, *both* orders can be a single
  ref move, so atomicity alone does not decide it. But merge-then-flip's natural
  porcelain implementation is two ref moves on a shared ref, with an observable
  inconsistent state in between; flip-then-merge has no such implementation.
  Atomic by construction beats atomic if you are careful.

Two rules make the whole thing airtight:

- **Actor rule — authorship is open, integration has one writer.** The old
  wording ("only the leader mutates trunk") conflated two things. Any actor may
  *author* any ticket change on their own branch; what is reserved is
  *integrating* it. A plan change a worker makes rides the same close/merge path
  as their code, and the leader's merge is where it gets reviewed. Single-writer
  to the integration ref survives verbatim; it was never about authorship.
- **Authority rule** — once claimed, a unit's subtree is authoritative *on its
  branch* until merge. An integration-lane structural edit to a *claimed* ticket
  (`reparent`) commits on `home` and reconciles via the optimistic field-level
  merge (§4) — a different line (`parent:`) than the branch's events, so it
  auto-merges. A same-field clash (leader `abandon`s while the worker `submit`s)
  is the rare genuine conflict, and it *should* surface to a human — it means two
  actors disagree about the ticket's fate.

**Ownership is graded, not binary.** The question "may a worker create tickets?"
is the wrong shape; the real constraint is that concurrent actors respect
responsibility domains, and the design supplies two defences rather than a
prohibition — branches make logical collisions *detectable*, worktrees make
physical ones *impossible*. So:

- edits **inside** the claimed subtree are contention-free by construction and
  need no signal at all;
- edits **outside** it are permitted but *detected* — git catches the collision
  at merge, and the close brief lists "this branch also modified: X, Y" so the
  leader reviews the plan change deliberately instead of finding it in a diff.

That grading is what makes the four responses to an unforeseen discovery all
cheap. An actor who finds a problem mid-flight can:

1. **keep going and hope** — the only objectively wrong choice, and the only one
   with no verb behind it;
2. **adapt within the task** — entirely inside the owned subtree, including
   creating sub-unit tickets: `new` is branch-lane when the new ticket is a
   sub-unit descendant of a claimed unit, integration-lane otherwise;
3. **abort and raise to the supervisor** — `yield` (§3.3/§3.4), which needs no
   new state because handing back *is* relinquishment;
4. **modify the surrounding plan** — author the edit on their own branch under
   the actor rule above, detected at merge.

planr cannot forbid (1) — it is git — but it can make (2), (3) and (4) each a
single verb, so that hoping is never the path of least resistance.

Net: parallel workers never contend, parallel reviewers never contend, same-slug
claims resolve by ref CAS, and only the inherently-sequential integration steps
serialize. The flat-file rewrite preserves 0.3's coordination guarantee and
tightens it — two fewer locks than 0.3, and one fewer than the round-2 design.

## 5. Archival — bounded working tree, lossless recovery

The problem: a mature project accumulates tens of thousands of tickets; a flat
`tickets/` (or any in-tree structure, including a manifest file) grows without
bound and slows every scan/clone.

**Resolution — git history *is* the archive:**

- **Retire** = remove the file from the working tree (the `remove` content
  transform, §3.5) in a commit whose **trailers carry the metadata** --
  `Planr-Verb: archive` and `Planr-Ticket: <slug>`, the same two trailers every
  other verb writes. Kind and terminal state are read from the pre-deletion
  blob and the ticket's event chain, so no archive-specific trailer vocabulary
  is needed. The full record is preserved in history; the working tree stops
  carrying it.
- **No manifest file.** A separate manifest merely trades a boundless tree for
  a boundless file. The commit message *is* the manifest entry, riding along in
  metadata git already stores and grows unavoidably.
- **Working tree is bounded by *active* work**, not lifetime volume. 30k
  lifetime tickets, 200 live → you scan 200.
- **Recover** = read the trailer/sha, `git restore` the file back into
  `tickets/`. A lossless git operation.
- **Search over cold records** = a **derived index** built by walking
  `git log --diff-filter=D -- tickets/`, reading each retirement's trailer +
  blob. Deleting the index loses nothing (rebuild from the log), so it stays
  derived/disposable. Metadata search hits the trailers; full-text search hits
  the archived blobs.

So: **history = durable archive, commit trailers = metadata, index = rebuildable
search surface.** Nothing unbounded in the working tree; nothing authoritative
outside git; more git-native than a manifest.

`archive` is just a configurable verb (`content: [remove]`, gated on
`self: {status: terminal}`), not a special subsystem — and it does *not* change
state (§3.3): an archived ticket keeps whatever terminal state it folded to;
archival only relocates the file to history. That the file is gone is itself
derivable, which is why `archived` is a sub-status of `closed` on the structural
spine rather than a lifecycle state.

### 5.1 The derived index

Referenced throughout (archival search, query speed, the DB alternative), so
defined once here: **the index is a disposable, git-ignored cache derived from
the source of truth — never authoritative.** Live tickets are derived by
scanning `tickets/` (as 0.3's board already does); cold tickets by walking
`git log --diff-filter=D -- tickets/`. Delete it and it rebuilds; it is never
committed and never merges.

**Two different costs, honestly (R10).** The *live* scan is O(active work) —
bounded and cheap; for most projects no persisted index is needed at all (build
in memory per invocation). The *cold-archive* rebuild is O(lifetime history),
**not** O(live) — walking every retirement commit. That's why the archive index
is the one place a **persisted** cache earns its keep: because history is
append-only, the index is maintained **incrementally** (each retirement appends
one entry → steady-state O(new retirements)); the full O(history) walk happens
only on a cold rebuild (cache lost / first build). Even persisted it stays
disposable — losing it costs a one-time rebuild, never data (open question #8
tracks the persistence trigger). This preserves both "derive, don't store" and
fast queries without a database.

**The fold raises the index's stakes without changing its status (round-3).**
With state computed from a ticket's event chain rather than read off a line, the
in-memory graph stops being a convenience and becomes the only fast read path.
It remains disposable and rebuildable from history — §2's "never authoritative"
survives intact — but rebuild cost stops being trivial, so it should be sized
before the engine is committed to. Mitigating factor: the events for a live
ticket are bounded by its own short history, and the schema timeline is one
`git log -- .plan/schema.yml` walk shared across every ticket.

## 6. Why not a relational DB

A DB is categorically incompatible with the load-bearing bets:

- **`branch = claim` dies.** Two task branches never conflict in `.plan/`
  because each touches one file — a *filesystem* property. A shared DB file
  conflicts on every write and can't live on a per-task branch that merges with
  code.
- **Observability dies.** A ticket file is diffable/greppable/PR-reviewable; a
  sqlite blob is opaque and unmergeable in git.

The DB's only real advantage (fast queries) is recoverable via the disposable
derived index (§5.1), without paying the DB's costs.

**Nor a separate ticket history (round-3).** A tempting middle road stores
tickets in their own commit history — an orphan ref in the manner of `git-bug`
or `git-appraise` — coupled to trunk by pointers, which would keep plan churn
out of `git log`. It is rejected for a reason that only became visible after the
close ordering was settled: closing a task would become **two ref updates**,
one to integrate the code and one to advance the plan pointer, and no plumbing
trick makes those a single CAS. That is the merge-then-flip inconsistent
intermediate state, permanent and by construction. It also breaks §2's
non-negotiable — tickets would no longer be in the working tree, so `cat`, grep
and Obsidian stop working, and a PR stops showing the work and its ticket
together. The motivating problem is a *view* problem with a view-level fix:
plan commits touch only `.plan/` paths, so `git log -- src/` is already clean,
and `git log --first-parent` already shows one merge per unit.

## 7. Backward compatibility & migration

**Compatibility is outcome-equivalence, not byte-identity (R8/round-2).**
"Byte-for-byte 0.3" is impossible by construction: 0.3 has no `approved` state and
gates `close` by parsing free-text `verdict:` from a `## Review` body
(`extract_last_review_verdict` in `close_cmd.rs`) — the exact semantic
content-parse the 0.4 `require` vocabulary deliberately *cannot* do (§3.6). Since
verdict-as-state (R3) is core to 0.4, reproducing 0.3's file/history exactly is a
non-goal. So there is **one recommended preset**, and compatibility means the
same *gate/merge outcomes* on a migrated backlog:

- **default (recommended)** — `kinds: [epic, story, task]`, the 0.4 verbs and
  derived lifecycle. Relative to 0.3 it carries two intentional fixes: container
  `close` gates on `terminal` (an abandoned child no longer deadlocks its parent)
  and the manual review steps (`status: review`, hand-written verdict) become the
  `submit`/`approve`/`request-changes` verbs with an `approved` state.
- A more conservative variant (container-close on `done`, review left as manual
  edits) is expressible as a schema, but is *not* byte-for-byte 0.3 either — the
  verdict mechanism still differs. A project that genuinely needs 0.3's exact
  section-parse can put it in a `hook` (the escape hatch), accepting that this
  steps outside the schema-as-data model.

The **regression test** compares outcomes (which tickets gate/merge, given the
same inputs) between 0.3 and the 0.4 default on migrated backlogs — not byte
identity. Migration from `epics/ stories/ tasks/` dirs to flat `tickets/` is
mechanical (move files, drop numeric prefix, kind already in frontmatter); a
`planr migrate` command does it.

**Migration must also seed the event chain (round-3).** Frontmatter loses
`status`, `id`, `created` and `updated`, but a 0.3 ticket carries no `Planr-Verb`
history, so a naive fold would compute every migrated ticket as sitting in its
initial state. `planr migrate` therefore writes **one seed event per ticket** —
a commit whose trailers declare the state the ticket was in at migration — and
the fold treats that as the chain's origin. Pre-migration history stays readable
as prose but is not interpreted as events. This is the one place where the fold
needs something written down rather than derived, and it is a one-time cost at a
known boundary.

## 8. Open questions

1. ~~Primitive set completeness~~ — **resolved** (§3.5). Three content
   transforms (`annotate`/`edge`/`remove`) plus four ref effects
   (`advance`/`create`/`merge`/`delete`), with `worktree` and `hook` as verb
   keys; `commit` is the verb boundary.
2. ~~`new`/scaffold~~ — **resolved.** `new` is fixed tooling; per-kind
   `templates` schema key (§3.8); no `scaffold` primitive.
3. ~~`require` predicate vocabulary~~ — **resolved** (§3.6). Three reserved
   operators (`self`, `neighbors`, `sections`), implicit-AND, all graph-derived
   (no hook in `require`); double-duty with `lint`; surfaced the needs-vs-
   decomposition asymmetry (`depends_on` gates on `done`, rollup on `terminal`).
4. **Templating `$var` namespace** — finalize the fixed variable set and each
   verb's available subset (§3.7). Starting set now includes `$target`.
5. ~~Axes schema surface~~ — **resolved by dissolution** (§3.2). Storage-side =
   ownership; all edges are one mechanism differentiated by a semantics tag; a
   new axis is a name + tag. Hook contract nailed in §3.9.
6. ~~Schema location & loading~~ — **resolved** (§3.5). The schema is
   `.plan/schema.yml`, tracked in the same history as the events, declaring its
   language by URL (`$schema: https://schemas.columnzero.com/planr/v1/planr.schema.json`).
   Every agent reads the same schema because they read the same *commit*; the
   published document is a JSON Schema 2020-12 validator shipped in-tree and
   never dereferenced at runtime. Which presets ship is folded into §7.
7. ~~`unit` = strictly the terminal kind, or any childless node?~~ —
   **resolved by a third answer** (§3.1). Neither: the unit is a declared *cut*
   across the tree, and it is derived rather than declared — the kind whose
   verbs say `worktree: create`. Kinds below the cut are sub-unit structure.
8. **Index persistence** — pure in-memory rebuild per invocation vs a persisted
   git-ignored cache; when does scan cost justify persistence? Sharper now that
   the fold makes the index the only fast read path (§5.1) — the rebuild cost
   wants measuring before the engine is committed to.
9. **Filesystem legibility** — is `board`/`graph` + generated symlink views
   enough to replace `ls`-by-kind for humans? Slightly sharper under the fold: a
   ticket file no longer states its own state, so `board` carries more of the
   legibility burden than it did.
10. ~~**`yield` and re-claim**~~ — **resolved** in round 4
    ([§9c](#9c-round-4--what-the-spike-found-2026-09-01)). `yield` keeps the
    ref so the work and the handback note survive the supervisor's decision;
    `resume` re-dispatches it and `abandon` releases it. `claim` was left
    create-or-fail rather than made reattaching, so its compare-and-swap
    survives — and every other ref move became a CAS too.
11. **Container integration branches** — **resolved by choosing not to have
    them.** Children merge to trunk; a container's ref carries only its own
    declarations, and its close is gated on `children: terminal`. Executed in
    `tests/next-scenarios.rs`. The contract change that would keep a true
    integration branch cheap is still in place (`base: home` resolves by walking
    the parent chain), so adopting one later stays a schema change rather than a
    contract change.
12. **Migration seed events** — the exact trailer shape `planr migrate` writes to
    seed a 0.3 ticket's event chain (§7), and whether a seed is one commit per
    ticket or one commit for the whole backlog.

## 9. Fresh-eyes review findings (round 1, 2026-08-21)

An independent fresh-context review (no prior design context) cross-checked the
doc against the 0.3 source. Assessment column is *this author's* triage, not the
reviewer's. Findings drive the next rework pass; nothing below is fixed in the
prose above yet except where noted.

### Load-bearing

- **R1 — No primitive/verb mutates an *edge*. ✅ RESOLVED.** Added the typed
  **`edge` primitive** (`set`/`add`/`remove`, per-edge cardinality, validity
  guards: target-exists / adjacency-legal / acyclic) and leader verbs
  `reparent` / `add-dep` / `drop-dep` / `assign` (§3.4, §3.5). Only forward
  fields are written, from the owner. CLI-completeness restored.
- **R2 — Abandoned-dependency decision has no CLI resolution. ✅ RESOLVED** by
  R1: `drop-dep` removes the edge; `abandon` abandons the dependent.
- **R3 — Verdict-in-a-section-body breaks require purity. ✅ RESOLVED — better
  than the proposed fix.** The review outcome is modeled as a **state**
  (`approved`), not a frontmatter attribute: `approve` transitions
  `review → approved`; `close` gates on the pure `self: {status: approved}`. No
  verdict field, no append-log, no git-order dependency — and no
  attribute-writing primitive needed. Reviewer prose stays as *commentary* in
  `## Review`.
- **R4 — `terminal` vs optional `from`. ✅ RESOLVED.** From-less transitions
  originate from any **non-terminal** state (terminal states are absorbing);
  rework verbs (`approve`/`request-changes`/`close`) declare `from` explicitly
  (§3.4). `terminal = {done, abandoned}` derives cleanly; the rework-guard
  footgun (reviewer's #12) closed.
- **R5 — `require`↔`lint` grammar. ✅ RESOLVED (§3.6).** A lint invariant is a
  `when`/`must` pair, both built from the same operators; `lint` adds
  `∀ ticket where when(t): must(t)`. A verb precondition is the degenerate
  single-ticket case. Claim corrected from "identical grammar" to "shared
  vocabulary + a quantifier"; many invariants also derive from verb requires.
- **R6 — Per-kind lifecycle / non-spine kinds. ✅ RESOLVED (§3.1, §3.3).**
  Non-spine kinds go in a separate **`groups`** key (§3.1), which materializes a
  grouping node-kind + its group edge, dissolving the `parents: []`/epic
  collision. Per-kind lifecycle is **not declared — it's derived** from the
  verbs that `apply-to` each kind (like `terminal`), with the initial state from
  `templates.<kind>.status`. The authored `lifecycle` block disappears entirely;
  `lint` flags anomalies and `planr lifecycle [kind]` renders the derived
  machine (which also settles the `qa`-state nit).
- **R7 — Ref/actor model. ✅ RESOLVED (§4.1).** Two lanes: a **branch lane**
  (`claim`/`submit`/`approve`/`request-changes` + claimed-task edits) — per-task,
  parallel, lock-free; and a **trunk lane** (`new`/`close`/`archive`/`release` +
  unclaimed-ticket edits) — leader-only, serialized. `close` bridges by reading
  the `approved` state from the branch, then merging (no chicken-and-egg).
  Actor rule (only the leader writes trunk) + authority rule (a claimed task's
  file is branch-authoritative until merge). Also settles the deferred
  concurrency pressure-test.

### Real but smaller — all resolved

- **R8 — "Byte-for-byte 0.3" was false. ✅ RESOLVED (§7)** — *superseded by B2
  (round 2), which went further:* "byte-for-byte" is dropped entirely (0.3's
  verdict-parse isn't expressible in `require`); compatibility is
  outcome-equivalence, one recommended preset. See §9a/B2.
- **R9 — `neighbors` inverse-role names. ✅ RESOLVED (§3.2).** Each edge
  registers a forward name and an inverse-role name (`parent↔children`,
  `milestone↔members`, `depends_on↔dependents`); group edges declare their
  inverse in `groups`; `lint` validates `neighbors` args against the registry.
- **R10 — Cold-index cost. ✅ RESOLVED (§5.1).** Now stated honestly: live scan
  is O(active), cold-archive rebuild is O(history); the archive index is
  maintained incrementally (steady-state O(new)), and it is the one place a
  persisted (still-disposable) cache is justified.
- **R11 — Primitive contracts. ✅ RESOLVED (§3.5).** `merge` (conflict → abort +
  rebase guidance), `cleanup` (idempotent), `archive` (`git rm`, gated terminal),
  `new-worktree`/`branch` (create-or-abort) now have defined semantics and
  failure modes; only `merge` can leave partial state.

### Pushed back on

- **R12 — Rollup-under-archival (reviewer's #13): rejected.** `archive` requires
  `self: {status: terminal}`, so an incomplete child can't be archived, and
  archiving an already-terminal child doesn't change the parent's closeability.
  Not a real problem.

### Nits — resolved

- ~~`qa` transitions `to: qa`~~ — moot under R6 (states derived; `lint` flags a
  reachable-but-terminal state, §3.3).
- ~~`$branch` undefined at `new`~~ — §3.7: template-var availability is
  context-dependent; referencing an out-of-context var is a `lint` error.
- ~~`sections` checks existence not content~~ — §3.6: stated, and it's the
  structural pin working as intended (content quality is the reviewer's call).
- ~~Tamper-check vs. index location~~ — §3.9: the check watches only *tracked*
  `.plan/` files; the git-ignored index is outside that set.
- **Naming (residual):** `link` ("undirected-ish, cosmetic") is still vague vs
  `depends_on` — worth a one-line "when to use which" in a future edit. `neighbors`
  kept (its inverse-role args, §3.2, make the direction explicit).

### Validated as sound (no change)

Storage-side = ownership (§3.2); milestone-as-group-edge removing the epic-07
directory hack; needs-vs-decomposition asymmetry (§3.6, confirmed against 0.3
`close_cmd.rs`); per-kind available actions from `applies-to` (§3.4);
git-history-as-archive with commit trailers (§5).

### Status (round 1)

All round-1 findings (R1–R11) resolved. But a second pass then found two
blockers the *rework itself* introduced — see below.

## 9a. Fresh-eyes review findings (round 2, 2026-08-24)

A second cold review checked whether the round-1 resolutions held and whether the
heavy rework introduced new contradictions. Two blockers (both in the rework
seams) + smaller items; all now fixed.

- **B1 — terminal-derivation contradicted the default verbs. ✅ FIXED (§3.3/§3.4).**
  `claim`/`submit`/container-`close` were written `from`-less but are the *sole
  exits* of `todo`/`in_progress`/`todo` — which the §3.3 derivation marks
  terminal and the lint guardrail freezes, so the default schema failed its own
  lint and the §3.3 worked example silently mislabeled those verbs "explicit."
  Fix: **progression verbs declare explicit `from`; `from`-less is reserved for
  the "any non-terminal" verb (`abandon`).** Corrects the earlier "prefer
  `require` over `from`" framing — `from` is structural (source state), `require`
  is an orthogonal precondition; the former is the norm.
- **B2 — "byte-for-byte 0.3" was self-contradictory and unbuildable. ✅ FIXED
  (§2/§7).** §2 claimed the default is byte-for-byte while §7 said it's a
  superset; and `0.3-strict` couldn't reproduce 0.3's free-text `verdict:` parse
  within the `require` vocabulary. Fix: **drop "byte-for-byte" entirely** —
  compatibility is *outcome-equivalence* on migrated backlogs; one recommended
  preset; 0.3's exact section-parse is only reachable via a `do` hook.
- **Smaller (all fixed):** primitive count 9→**ten** (§8); added the **`story`
  template** and stated every kind needs one (§3.7); `abandon`/`archive` now
  `applies-to` **`milestone`** so milestones can be retired (§3.4); **`link`** is
  clarified as body `[[..]]` prose, *not* a frontmatter edge / not in `edge`'s
  domain — so its absence from the verb set is not a CLI-completeness gap
  (§3.2/§3.5); **`close`** flips `approved → done` on the branch tip, *then*
  merges that into trunk — the terminal state rides in with the work as one
  integration, not a trailing trunk-only edit (§3.4/§4.1).
- **Validated by round 2 (no change):** storage-side ownership + the tightened
  R2 argument (a claimed task's deps are `done`, and `done` is terminal/
  un-abandonable, so claimed tasks never face an abandoned dep); verdict-as-state;
  the edge primitive; two-lane concurrency; git-history archival; the
  structural-only `require` pin.

### Status

Both review rounds' findings resolved. Remaining: one residual naming note
(`link` usage guidance) and the standing open questions (§8). Ready to move from
design into implementation planning.

## 9b. Round 3 — the derived-state pivot (2026-08-28)

Round 3 was not a fresh-eyes review but a working session that pushed on one
question — *what does the `status` field actually buy us?* — until the model
changed underneath it. Recorded here because the reasoning is not recoverable
from the resulting text.

### The pivot

**State is folded from events, not stored** (§3.3). The argument that carried it
was not elegance but **configurability**: a stored status is data a schema edit
can silently invalidate, with no migration story, and the redundancy-plus-lint
scheme that was supposed to catch the drift degrades exactly where custom
schemas make drift most likely. Under a fold the schema becomes the *interpreter*
of history rather than a validator over stored data.

The line that decides what can be derived: **structure encodes where the work
is; declarations encode what someone has decided about it.** `todo` /
`in_progress` / `done` are git's own branch ontology and derive for free — as do
`done` vs `abandoned` (closed *with* a merge vs *without* one) and `archived`
(the file left the tree). `review`, `approved` and `abandoned` are speech acts
and must be declared. Hence the two-layer model, whose real payoff is that the
structural spine is readable **without the schema at all**.

Consequences that fell out, each recorded in place:

- `transition` stops being a primitive; `from`/`to` are verb attributes (§3.4).
- Ten primitives become three content transforms plus four ref effects; the
  ordered `do` list disappears because tree-building and ref-moving are separate
  phases (§3.5). Both earlier ordering arguments — `claim`'s and `close`'s — were
  arguments about a conflation, not about order.
- Frontmatter loses `status`, `id`, `created` and `updated` (§4).
- The initial state is derived, so `templates` loses its `status` key (§3.8).
- `archive` becomes the `remove` content transform (§5).
- Migration must seed an event chain (§7).

### What was rejected, and why it is worth remembering

- **Structural encoding all the way down** — inventing refs for `review` and
  `approved`. Refs do not merge, carry no prose, and *die*: `cleanup` deletes
  them and archival removes the file, so a terminal state encoded in a ref
  evaporates. The durable record of a close is the **merge commit**, not the ref.
- **Status as a commit trailer** rather than a file field. It is content, so it
  merges and survives, but it is not greppable in the working tree, not
  hand-editable, and needs a history walk to read. A relocation, not a
  simplification.
- **A separate ticket history** (§6) — rejected because closing would become two
  ref updates that no plumbing makes atomic.
- **A per-event schema trailer** (§3.5) — unnecessary, because the schema is
  tracked in the same history the events live in.

### Absorbed from the skill-side handoff

`docs/v2-handoff-execution-topology.md` asked for two things, on the grounds that
2N agents per epic exhausts usage limits. They were less entangled than the
handoff presented:

- **Request B — the unit above a leaf: accepted** (§3.1), and it is the request
  that actually delivers the cost reduction. One worker and one reviewer per
  *story* is 2x-5x fewer agents, and it needs nothing from Request A.
- **Request A — container integration branches: deferred**, but its *contract*
  change is adopted: `branch`/`merge` resolve base and target by walking the
  parent chain, with trunk as the base case rather than a constant (§3.4). 0.3
  left this variable and 0.4 had re-fixed it; restoring it makes A a later schema
  choice rather than a later rewrite. Two things A must answer before it lands.
  First, the derivation is half-specified: "a container is an integration point
  iff its `close` includes `merge`" says where the branch *ends*, not who *cuts*
  it — the container also needs a verb carrying `effect: create`, which makes it
  a unit minus the worktree and collapses the unit/container distinction into
  "which git effects appear in your verbs". Second, A trades away trunk-based
  development: a story branched from an epic branch cut days earlier accumulates
  integration risk against real trunk until the epic merges, and the refresh
  direction is mechanically harder than the flip — merging trunk *down* into an
  open epic branch is a real three-way merge, worktree-free only while it does
  not conflict.
- **Request C — no 0.3 changes**: nothing asked, nothing done.

### Two corrections to the design as written

- **Worktree location.** 0.3 already defaults to `.plan/worktrees/` and already
  accepts an override (`claim.rs`); the round-2 text had frozen `../wt-<slug>`
  into the primitive contract. A regression, now reverted and made a template
  (§4).
- **`close`'s justification.** Flip-then-merge was already correct, but was
  defended by lineage and tamper-evidence — an argument that does not survive
  pressure, since the leader authors the declaration either way. Replaced with
  revert atomicity and first-parent history (§4.1).

### Status (round 3)

The model is materially different from round 2 and materially simpler: fewer
primitives, fewer stored fields, no ordering, one fewer lock. The published
schema at `schemas/planr/v1/1.0.0/planr.schema.json` now pins it — 29 fixtures encode
what the language must accept and reject, including the legacy `do` list, and
they run in CI. Writing that schema caught a real gap in this design
in the process, which is the argument for pinning a contract in something
executable rather than in prose alone.

Still open: everything in §8, with 10 through 12 new this round.


## 9c. Round 4 — what the spike found (2026-09-01)

Round 4 was a throwaway vertical slice: `planr next`, behind the `next` cargo
feature, implementing schema loading, event enumeration, the fold, and a verb
runner over the five-verb loop for a single `task` kind. 0.3 was untouched
throughout. The point was to find out which parts of round 3 survive contact
with git, and the answer is that four of them did not.

Everything below was demonstrated, not reasoned about. Where a claim is about
cost, it was measured; `benches/board_scaling.rs` reproduces the measurement.

### Four findings that change the design

**1. Enumeration must go through trailers, never paths.** §3.4 makes empty
declarations first-class — "*Several verbs have no content at all. `claim` cuts
a ref; `submit` and unit `close` change no bytes. Their commit message is the
entire payload.*" An empty commit touches no path, so the obvious cheap read,
`git log -- .plan/tickets/<slug>.md`, **silently skips exactly those
declarations** and the fold reports a submitted ticket as still `in_progress`.
Demonstrated in a scratch repo before any engine code was written. §3.5 gives
every event a `Planr-Ticket` trailer so it is *attributable*; attributable is
not the same as *findable*, and the design never said how events are
enumerated.

**2. Ordering must come from the commit graph, never from which ref read the
event.** The first implementation walked trunk and the ticket's branch
separately and concatenated, reasoning that a branch's events are newer than
the trunk events it descends from. That is false as soon as trunk moves after
the branch is cut — which is precisely what §4.1 already permits: "*An
integration-lane structural edit to a claimed ticket (`reparent`) commits on
`home` and reconciles via the optimistic field-level merge (§4) — a different
line (`parent:`) than the branch's events, so it auto-merges.*" That reasoning
is about reconciling *files*, and it quietly stops applying once state is a
fold: there
is no field to merge, there is an ordering to establish. The observable
failure: a worker `yield`s, the supervisor then `abandon`s on trunk, and the
concatenation ordered the later abandon *before* the earlier yield — so an
abandoned ticket read back as `todo`, and therefore as reclaimable.

**3. The initial state cannot always be derived.** §3.8 states the rule — "*the
initial state is **the state that appears as some verb's `from` but is never
any verb's `to`*** *— `todo` for a task, `planned` for a milestone*" — which
assumes an acyclic state graph. §3.3 asserts the opposite four sections
earlier: "*The lifecycle has a rework cycle … so it is a **state machine**, not
a DAG.*" The contradiction was invisible until `yield` closed a loop back to
the entry state, making `todo` both a `from` and a `to`. The deeper cause is
that **creation is not a verb** — §3.8 keeps `new` as fixed tooling precisely
because "*it has no prior node and no from-transition*", so the entry state is
not an edge in the verb graph at all. An acyclic graph lets you recover it by
accident from the source node; a cycle removes the accident. Resolved as
derive-or-declare: derived when unambiguous, `templates.<kind>.initial`
required otherwise, with the fix stated in the error.

**4. `delete` is superseded by `ticket-only`.** Releasing a ref and destroying
work looked like one act because `git branch -D` does both. They separate: a
`merge -s ours` recording the branch as a second parent, with the ticket file
taken from the branch, puts the worker's `## Blocked` rationale on trunk beside
the `## Abandoned` note, leaves the work reachable in history but absent from
the tree, and lets the ref be released destroying nothing. Once separated, only
the first half belongs to planr — destroying history is git's job, and an
operator who truly needs a branch gone deletes the ref first, leaving nothing to
preserve. So the effect vocabulary is **`advance | create | merge |
ticket-only`**, and there is deliberately no destructive effect: a destructive
flag in `--help` is an attractive nuisance when the callers are agents.

### Scaling, analysed and then measured

Terms: **C** total commits, unbounded; **T** live tickets, bounded by archival;
**E** events per ticket, small.

| operation | as first written | now |
|---|---|---|
| `state <slug>` | O(C) | O(C) |
| `board` | **O(T · C)** | **O(C + ΣE)** |

Archival bounds **T**, the live tree — not **C**. That is enough: with `board`
doing one walk and bucketing by `Planr-Ticket`, planr is O(C), the same order as
git's own log. Measured on synthetic backlogs, per-ticket cost was 12.6 → 17.7 ms
rising with history under the per-ticket walk, and a flat ~2.8 ms under the
single walk. Per-ticket cost is the diagnostic; total time cannot distinguish
removing a factor from shaving a constant.

Two conclusions worth carrying forward:

- **Empty declarations and the index are the same decision.** Trailer scanning
  must load and parse every commit object. Path-limited scanning gets git's
  changed-path Bloom filters and skips most object loads entirely — same
  asymptotics, a large constant. planr cannot use them *only* because empty
  declarations touch no path. So the question is not whether an empty `submit`
  is elegant; it is whether planr owns an index or borrows git's.
- **The archive commit is a natural memoization point.** `archive` requires
  `self: {status: terminal}`, so an archived ticket's state can never need
  recomputing. Recording it there lets a gate on an archived dependency resolve
  in O(1) without re-folding a ticket whose file no longer exists — which makes
  enforcing no-dangling-pointers unnecessary, and that is worth avoiding, since
  enforcement would make archival a closure operation over unrelated tickets.

Still unmeasured, and the reason `board` is not yet done: with the T multiplier
gone, **the bottleneck moves from history walking to process spawning** — one
`git show` per ticket to read its kind. That is a tree read, so archival bounds
it, but it now dominates.

### Implementation cautions

- **A template must not scaffold a section a verb gates on.** `submit` requires
  `## Validation`; the task template created it; the gate was therefore
  satisfied at birth and checked nothing, forever. §3.6 is right that `sections`
  checks "*existence, not content*" — that is only a gate if something other
  than the tool writes the section.
- **`claim` needs no lock.** `git update-ref <ref> <sha> ""` is a
  compare-and-swap: the empty old-value means "must not exist", so two
  concurrent claims resolve by ref CAS and the loser gets a clear failure.
- **A latent 0.3 bug the new ordering exposes.** `git.rs`'s `worktree_add`
  omits the branch argument when the branch already exists, so
  `git worktree add <path>` invents a branch named after the path. 0.3 never
  reaches that path because it creates branch and worktree together; the 0.4
  `claim` creates the ref first *by design*, and hits it every time.
- **A ref-releasing effect must land its own declaration first.** The original
  `delete` deleted the ref without advancing the base, orphaning the verb's own
  commit along with the work it was recording — abandoning destroyed the
  evidence that it happened.
- **The structural spine's "in-progress" fact is wrong.** §3.3 lists
  "*in-progress (a `plan/<kind>/<slug>` ref exists)*" as schema-free. After a
  `yield` the ref exists and the state is `todo`. The correct structural fact is
  **has work in flight**, which is a different and more useful thing: it is
  exactly what a supervisor weighs when deciding whether to replan or abandon.

### A gap the spike opened, and how it closed

`yield` keeps the ticket's ref, so the partial work and the note explaining the
handback both survive for the supervisor to weigh. But `claim` declares
`effect: create`, which is create-or-fail, so **a yielded ticket cannot be
re-claimed**:

```
$ planr next do claim t1
cannot create branch 'plan/task/t1': ... reference already exists
```

That foreclosed the *more likely* of the supervisor's two options — replanning
and re-dispatching is the normal outcome, and abandoning the exception — so
only the exception worked, and the failure surfaced as a raw git error rather
than a planr-level explanation.

**Resolved by a `resume` verb, not by extending `claim`.** Making `claim`
create-or-reattach would destroy its atomicity: the empty old-value in
`update-ref <ref> <sha> ""` is what makes two concurrent claims resolve in the
kernel, and reattachment removes that guard, leaving only a folded `from: todo`
check that two claimants can both read as true. A distinct verb is also what
the model forces, since one name with one kind is ambiguous dispatch — and
that is arguably correct, because re-dispatching after a replan is a different
act by a different actor than claiming for the first time.

The precondition needed no new operator: **`base: own` on a ticket in its
initial state is the definition of "yielded"**, because a fresh ticket has no
ref and a claimed one is not `todo`.

Two mechanism changes fell out of building it:

- **The worktree rule was too strict.** It required `worktree: create` to imply
  `effect: create`, on the reasoning that a worktree belongs to a ref the verb
  cut. A worktree needs a ref to *exist*, not to be *created*, and `base: own`
  guarantees that too. The rule is now `effect: create` **or** `base: own`, in
  both the engine and the published schema.
- **Every ref move is now a compare-and-swap.** `git update-ref` takes an
  expected-old-value for any move, not only creation, so passing the sha the
  verb read costs one argument and makes `submit` and `approve` as safe as
  `claim`. The atomicity had looked like a property of claiming; it is a
  property of moving a ref.

Worktree creation also became idempotent, which the design already claimed of
it in both directions but the engine only did for removal. It matters here:
`yield` leaves the worktree standing, so a `resume` by the same worker finds it
present while a `resume` elsewhere has to make one.

### Method findings

- **A contract suite that validates only its own fixtures is self-consistent,
  not correct.** `tests/schema.rs` validated fixtures, which are authored to
  match the published schema and so can never disagree with it. Nothing
  validated `.plan/schema.yml`, the file the tool actually loads — and three
  keys drifted across two renames with every test green. Fixed by validating the
  reference schema itself; mutation-tested by restoring the old effect enum.
- **A benchmark reporting only total time cannot detect the regression it
  exists to catch.** Hence `benches/board_scaling.rs` prints per-ticket cost and
  states in its own output what shape to expect.
- **An experiment that grows two variables together cannot separate them.** The
  first measurement of the container gate grew the child count and the history
  together, reported a flat per-child cost, and read as healthy. Holding the
  children fixed and growing only the unrelated history showed the gate is
  O(N · C). A confounded experiment does not return a weak signal; it returns a
  confident wrong one.

### Follow-on probes: topology, bounds, and git's index (2026-09-05)

The round-4 spike proved the verb machinery on a flat backlog. These probes run
the two shapes the rework exists to justify, and then pull on the two threads
they exposed. Everything below is measured; the scenarios live in
`tests/next-scenarios.rs`.

**Both topologies execute, and the sub-unit one needs no language change.** A
container gated on `children: terminal` closes only once every child is
terminal, and `terminal` has to span `done` and `abandoned` alike. Above it, the
more important result: a *unit above the leaf* -- three tasks under one story
costing **one** worktree and **one** branch. The unit is derived from `worktree:
create`, so a kind whose verbs declare no worktree is already expressible.
"A task is a ticket" and "a task is a checklist line in the story body"
are therefore two schemas, not two designs, and both are available at once.

**`archive` could never fire.** It is from-less *and* declares `require: { self:
{ status: terminal } }`, but the absorbing rule for from-less verbs refuses
terminal tickets outright. The two rules contradict, so the verb was
unreachable. An explicit status precondition now wins over the implicit rule.
This matters well beyond one verb: the scaling story in
[Scaling, analysed and then measured](#scaling-analysed-and-then-measured)
turns on archival bounding **T** while **C** grows, and that argument rested on
a verb that had never once run. A step that bounds the system, which nobody
takes, is not a step.

**Folding an archived ticket needs the kind, which lives in the file `archive`
deletes.** Enumeration survives archival, because trailers are in commit
messages -- but interpretation does not, because the kind selects the
sub-machine. State falls back to the last commit that still had the file, and
only on a genuine miss: a file that is present but invalid must report *that*,
or a ticket carrying a stored `status` is reported as never having existed.

**The container gate is O(N · C).** One full history walk per child. Holding the
child count fixed and growing *unrelated* history separates the two costs:

| children | commits | `close` |
| --- | --- | --- |
| 20 | 42 | 190ms |
| 20 | 542 | 316ms |
| 20 | 2042 | 545ms |

Left unfixed in the spike deliberately; the fix is the one already applied twice
elsewhere -- one shared walk, plus the batched blob read.

#### Bound the walk on the event, not on the file

`fold_state` is last-write-wins over `to`: it neither validates transitions nor
accumulates. So a ticket's state is determined entirely by **the most recent
event whose verb declares a `to`**. Reading state never needs a ticket's
history; it needs its latest transition. That makes the right terminator an
event property rather than a structural one:

| bound | sound? | scans |
| --- | --- | --- |
| the parent's creation | **no** -- misses pre-reparent transitions | commits since the parent existed |
| the ticket's own creation | yes | commits since the ticket existed |
| **first event with a `to`, walking backwards** | yes | **commits since the ticket last moved** |

A parent anchor is sound for *discovering* an edge and unsound for *folding*
one. `reparent` is the case that separates them, and it separates them the
opposite way round from the intuition: the edge is established by the reparent
commit, which necessarily postdates the new parent, so the edge is always in
range. The child's own transitions are not. Measured on a task claimed under one
epic and then reparented under a newer one, a parent-anchored fold sees a single
event and reports `todo` for a ticket that is `in_progress`.

The event bound needs the own-creation anchor as its **floor**, not its primary
rule: `new` is not a schema verb, so it carries no `to` and will not terminate
the walk. A ticket that has never transitioned needs its creation commit to stop
at, and then folds to `initial`.

What the anchor buys depends entirely on where unrelated commits sit. Over 2022
commits with ten children:

| unrelated history sits | commits after the epic's creation | saved |
| --- | --- | --- |
| before the epic exists | 20 of 2022 | 100% |
| during the epic's life | 2020 of 2022 | 1% |

Anchoring each ticket at its own creation inverts that, because children are the
short-lived things: in the interleaved layout the epic scans 2020 commits while
its children scan 19, 11, and 1. **The event bound is on recency, not
lifetime** -- a five-year-old epic that transitioned yesterday costs one commit
to read, and what is expensive is a ticket that has sat idle, which is the
opposite of what a creation anchor punishes.

One caveat this concentrates rather than creates: last-write-wins means trusting
that the first state-changing event found walking backwards really is the
latest -- the same `--date-order` union assumption that the trunk-versus-branch
ordering bug exposed. Today a misordering is one wrong event among many; under a
terminating walk it is the whole answer.

#### Git's index is usable exactly where paths are touched

Empty declarations forfeit git's changed-path Bloom filters, but only for the
one question that is not path-shaped. The design already splits along the right
seam:

| operation | touches a path? | uses git's index? |
| --- | --- | --- |
| find a ticket's creation (the anchor) | yes -- `new` writes the file | **yes** |
| find archived tickets (`--diff-filter=D`) | yes -- `archive` deletes it | **yes** |
| enumerate a ticket's events | no -- declarations may be empty | no; trailers |

Measured over 2022 commits, an anchor lookup costs 17ms with no commit-graph,
12ms with one, and 5ms with `--changed-paths`. The trailer walk sits at 16ms and
is unmovable. The cold-ticket scan in
[The derived index](#51-the-derived-index) already relies on this. So the
empty-declaration decision costs the index only where it was never going to
help, and the alternative that would recover it -- making every declaration
touch a path -- buys the index back at the price of a transcript accumulating in
the ticket file and textual conflicts between declarations that today never
conflict.

#### Archive versus close is a schema choice

Folding removal into `close` is a pure schema edit: `archive` is only
`content: [remove]`, so moving that onto `close` needs no code change. Probed
directly, the container gate stays correct, because a child drops out of
enumeration exactly when it would have passed the gate anyway.

The two stay separate for now, to keep the upgrade path from 0.3 open. Recording
it as a property of the new system rather than a pending decision: **the
friction of a two-step retirement is a schema default, not a design
commitment.** An author who wants retirement to be automatic puts
`content: [remove]` on `close`; an author who wants a window in which finished
work is still readable on disk keeps the verbs apart. Neither needs the tool to
change, which is the same shape as the sub-unit question above.

The one thing that must **not** be made symmetric: removal on `abandon`.
Both states are terminal, so symmetry is tempting, and it hollows out every
container gate -- if abandoned tickets vanish too, every child drops out of
enumeration and `children: <anything>` passes vacuously. The asymmetry is
principled: `ticket-only` exists to deposit the worker's rationale on trunk, and
erasing the file in the same commit defeats its only purpose.

The cost of coupling, if it is ever made the default, is
[open question #9](#8-open-questions): a just-closed ticket is the one carrying
`## Validation` and `## Review`, so the moment it is most worth reading is the
moment it leaves the filesystem.

### Status (round 4)

The model survives, with four corrections. Nothing found here challenges the
round-3 pivot itself: state folded from events held up under every scenario
tried, including the one that broke the enumeration. What broke was consistently
the *mechanism* around it — how events are found, how they are ordered, where
the initial state comes from, and what happens to a ref when a ticket ends.

The spike is `src/next/**` on `planr-next`, behind the `next` feature, with
sixteen end-to-end tests across `next-e2e.rs` and `next-scenarios.rs`. It is
reference material, not a foundation: the valuable residue is `plumbing.rs` —
build-tree-then-move-ref, the ref CAS, the ticket-only merge — and the throwaway
part is the shortcut that survives, a `state_at` that re-walks history inside a
loop, which is what makes the container gate O(N · C).
