# planr v2.0 — schema-driven, typed-graph backlog

> **Status:** design exploration (pre-v1.0). Nothing here is committed to code.
> This captures the reasoning and decisions from the v2 brainstorm so they
> survive the conversation. Open questions are flagged inline and collected at
> the end.

## 1. Why change anything

v1 hard-wires two things that turn out to be project-specific:

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
  read time (as v1 already does for the board). Any index is a *disposable,
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
- **Default schema reproduces v1 byte-for-byte**, so existing backlogs Just
  Work and the default doubles as a regression test of the generalization.
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
  task:  { parents: [story], unit: true }
```

- The list form is **sugar** that desugars to adjacency internally, so the
  engine always speaks adjacency. This buys the future extensible case (e.g. a
  `maintenance` kind with `parents: [epic, story]`) as a config change, not a
  rewrite.
- Exactly one kind is the **`unit`** — the executable leaf that gets a
  worktree/branch/review/merge. In the list form it's the last element,
  implicitly. Containers above it roll up.
- Decomposition stays a **tree** (single `parent` per node) at runtime. KISS/
  YAGNI: we are *not* building multi-parent decomposition now, but the
  adjacency representation leaves the door open.

### 3.2 Axes and edges — the typed graph, disciplined

Relationships are typed edges. Only two carry gating semantics; the rest are
grouping or informational:

| edge / axis        | direction        | semantics                              |
|--------------------|------------------|----------------------------------------|
| `parent`           | child → parent   | **rollup** (container done ⇐ children done) |
| `depends_on`       | dependent → dep  | **gate** (blocks claim until deps done); a DAG (cycle-checked) |
| grouping axes (e.g. `milestone`) | member → group | **group only**, no gate; derived views |
| `link` (`[[..]]`)  | undirected-ish   | **info**, cosmetic                     |

The key insight: the decomposition tree could never express a *second*
grouping (milestones group epics/stories that already have a parent). A typed
**group axis** expresses it as an orthogonal edge, deleting the
milestone-as-directory hack. Design shape: **one decomposition tree + N
orthogonal grouping axes + the dependency DAG + info links.**

**Storage-side = ownership.** Every edge is a frontmatter field on the node
that *declares* it, pointing outward, and which side stores it encodes who owns
the relationship — which gives correct behavior under mutation for free:

- `parent` on the **child** → the child owns its membership; abandon/archive the
  child and the edge travels with it, and the parent's rollup is re-derived from
  whoever still points at it. This is v1's "no agent ever edits a parent to
  record child state," generalized.
- `depends_on` on the **dependent** → the dependent owns its ordering; abandon a
  *dependency* and the edge is untouched, the ordering stays modeled, the dep
  just becomes `terminal` (non-blocking). Nothing to erase.
- a grouping axis (`milestone`) on the **member** → same mechanic as `parent`.

A **milestone is both a kind and an edge**: a node of kind `milestone` (outside
the decomposition spine — no `parent`, not a `unit`) that holds the release
record and its own lifecycle (`planned → in_progress → released`), plus the
`milestone` group edge that members point at. Its completion is opt-in-gated by
a `release` verb requiring `{members: terminal}` (§3.6) — non-blocking as a
grouping, gated only when explicitly released.

So all four edge types are **one mechanism**: a directional field on the owning
node, forward+inverse adjacency derived at read time, differentiated only by a
semantics tag (`rollup` / `gate` / `group` / `info`). Declaring a new axis is a
name + a tag; there is no "axis subsystem." (Precision: since `abandon` keeps
the file, an abandoned child's `parent` edge persists — it stays in the child
set but as `terminal`, satisfying the gate rather than leaving the set; it only
truly vanishes on `archive`. Same end state — the owner controls participation.)

```yaml
edges:
  parent:     rollup     # on child
  depends_on: gate       # on dependent
  milestone:  group      # on member; orthogonal, no gate
  link:       info       # cosmetic
```

### 3.3 Lifecycle — a state machine (not a DAG)

The lifecycle has a rework cycle (`review → changes-requested → in_progress →
review`), so it is a **state machine**, not a DAG. (The DAG is `depends_on`,
a separate graph — don't conflate the two.)

```yaml
lifecycle:
  states: [todo, in_progress, review, approved, done, blocked, abandoned]
```

`approved` is a real state between `review` and `done` (R3): the review outcome
is modeled as a transition, not a parsed free-text field, so the close gate is a
pure `self: {status: approved}` check and the board can distinguish "in review"
from "approved, awaiting merge".

Transitions are declared *on the verbs* that drive them (§3.4), not as a
free-floating table. The state machine is **declared for derivation** (board
coloring, "what's ready", lint) and **enforced only structurally** (via each
verb's `require`).

**`terminal` is derived, not declared.** A terminal state is one with no
outgoing transition in the verb-declared graph — computed (and cached) at
runtime, not listed by the user (a declared list can silently drift from the
actual graph — a footgun). Here that derives `terminal = {done, abandoned}`.
Note `archived` is deliberately **not** a lifecycle state — archival is a
storage relocation (live tree → git history, §5), orthogonal to status; an
archived ticket keeps whatever terminal status it had.

### 3.4 `verbs` — lifecycle mutations as composable recipes

A **verb** is a named lifecycle mutation. `board`/`lint`/`graph` are *not*
verbs — they are fixed read tooling (the tool's spine). Verbs are a **list**,
each entry selected by `(name, applies-to)`:

```yaml
verbs:
  - name: claim
    applies-to: [task]
    require: { neighbors: { depends_on: done } }   # deps must have SUCCEEDED, not merely be terminal
    do: [ new-worktree, branch, transition: { to: in_progress } ]

  - name: review
    applies-to: [task]
    do: [ brief ]                       # read-only helper (prints the review brief)

  - name: submit                        # worker: "ready for review"
    applies-to: [task]
    require: { sections: [validation] } # a ## Validation section exists (structural)
    do: [ transition: { to: review } ]

  - name: approve                       # reviewer: outcome is a STATE, not a field
    applies-to: [task]
    do: [ annotate: { section: Review, body: $message },
          transition: { from: review, to: approved } ]

  - name: request-changes               # reviewer: bounce back to the worker
    applies-to: [task]
    do: [ annotate: { section: Review, body: $message },
          transition: { from: review, to: in_progress } ]

  - name: close                         # unit variant: merges a branch
    applies-to: [task]
    require: { self: { status: approved } }
    do: [ transition: { from: approved, to: done }, merge, cleanup ]

  - name: close                         # container variant: trunk-local gate, no merge
    applies-to: [epic, story]
    require: { neighbors: { children: terminal } }   # abandoned child is a resolved decision
    do: [ transition: { to: done } ]

  - name: abandon
    applies-to: [task, story, epic]
    do: [ annotate: { section: Abandoned, body: $message }, transition: { to: abandoned } ]

  - name: archive                       # relocate to history; NOT a status change
    applies-to: [task, story, epic]
    require: { self: { status: terminal } }
    do: [ archive ]

  # --- edge mutation (leader backlog restructuring, R1/R2) ---
  - name: reparent
    applies-to: [story, task]
    do: [ edge: { set: { parent: $target } } ]

  - name: add-dep
    applies-to: [task]
    do: [ edge: { add: { depends_on: $target } } ]

  - name: drop-dep                      # resolves the abandoned-dependency decision (R2)
    applies-to: [task]
    do: [ edge: { remove: { depends_on: $target } } ]

  - name: assign                        # (un)assign a milestone
    applies-to: [epic, story]
    do: [ edge: { set: { milestone: $target } } ]

  - name: qa                            # example project extension
    applies-to: [task]
    require: { self: { status: approved } }
    do: [ transition: { from: approved, to: qa }, hook: "./ci.sh" ]
```

Every state change flows through a verb — including the ones v1 did as **manual
file edits** (the worker hand-setting `status: review`, the reviewer hand-writing
the verdict). In v2 those become `submit` / `approve` / `request-changes`, which
is strictly better: they get atomic commits and structural guards too. `approve`
*transitions* the ticket to the `approved` state; `close`'s
`require: {self: {status: approved}}` reads that state — a pure graph fact, so
the verbs interlock without parsing a free-text verdict (R3).

Decisions baked in:

- **`do`** is a single ordered list of primitives. Order is load-bearing:
  `claim` runs git actions *before* the state flip (establish the branch, then
  commit the flip on it); `close` runs the flip *before* `merge` (flip to
  `done` on the branch, then merge that flip). A fixed field-order model can't
  express both, so ordering lives in the sequence.
- **`require`** is a **separate key**, not a `do` item — it is a precondition,
  evaluated before any side effect. Its vocabulary is structural predicates
  only: `{field: value}` on self (`status: approved`) and aggregates over
  edge-neighbors (`children: done`). No semantic judgment (the pin).
- **`applies-to`** selects which kinds a verb definition serves. The same
  `name` may appear multiple times with **disjoint** `applies-to` — this is how
  `close` overloads (merge on units, gate-only on containers). The CLI surface
  stays `planr close <slug>`; the engine resolves `(name, kind-of-slug)` to one
  definition. Overlapping `applies-to` for one name is a lint error.
- **`from`** on a transition is optional, but a from-less transition originates
  from **any non-terminal state** (never from a terminal one — terminal states
  are absorbing, which is what keeps `terminal` derivable, R4). Rework verbs that
  must originate from a specific state (`approve`/`request-changes` from
  `review`, `close` from `approved`) declare `from` explicitly; otherwise a
  `request-changes` could fire from `todo` (the rework-guard footgun, R4).
  Prefer a structural `require`
  over `from` where a graph fact already implies the state.
- **Per-kind capability is derivable**: an epic offers no `claim` because no
  `claim` verb applies to it. The board can list available actions per ticket
  for free.

### 3.5 Primitives — the fixed vocabulary `do` composes from

The binary owns a small, git-aware primitive set; verbs are recipes over it.
The set is where the enforce/don't-enforce line is frozen (there is no
primitive that evaluates quality). Ten primitives in four flavors:

- **Content** (edit the ticket file; staged and committed as the verb's single
  atomic commit): `transition`, `annotate`, `edge`. Each guards its own
  invariant — that is why they are typed, not a single generic `set`.
- **Git** (manipulate refs/worktrees/history): `new-worktree`, `branch`,
  `merge`, `cleanup`, `archive`.
- **Output** (read-only, produce text): `brief`.
- **Escape hatch**: `hook` (run a project script) — opt-in, rare, kept off the
  main path so a stateless agent can still read what a verb does.

The three content primitives are the only ones that mutate ticket state; all
are pure file edits the engine stages and commits atomically.

**`transition`** writes the `status` frontmatter field, validated against the
state machine (a transition not declared by any verb is refused). The **review
outcome is a state, not an attribute** (R3): `approve` transitions
`review → approved`, so `close`'s gate is the *pure* `self: {status: approved}`
— no free-text verdict field, no git-order dependency.

**`annotate`** writes a **named section with a templated body** (structured
section+body form). Reviewer prose still goes in a `## Review` section, but it
is *commentary*, not a gate:

```yaml
- annotate: { section: "Abandoned", body: "$message" }
```

**`edge`** writes a **forward edge field on the owning node** (R1) —
`set` (single-valued: `parent`, `milestone`), `add`/`remove` (multi-valued:
`depends_on`, `link`):

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

Frontmatter stays minimal (id, kind, status, parent, depends_on, axes). No
generic attribute-writing primitive exists: everything that was tempted toward
one is either a state (`transition`) or an edge (`edge`).

**Commit boundary.** A verb produces exactly **one commit** (its net content
mutation), so `commit` is the verb boundary, not a primitive. Bumping
`updated:` is an automatic side effect of that commit, not a primitive either.
`merge`/`cleanup`/`archive` are git operations layered on top. This gives free
unwind-on-failure: a verb that fails before its commit leaves nothing behind;
the only partial-state case is `merge`/`cleanup` failing *after*, which is the
existing rebase-guidance path.

### 3.6 `require` — the gate predicate vocabulary

A predicate is conceptually a **pure, referentially-transparent boolean check**
over the graph — so its result is derivable and cacheable, and every predicate
is a **built-in** computed straight from the graph (never a spawned process).
`require` has **no custom-hook escape hatch**: impure or bespoke gating belongs
in a `do` hook (which vetoes by exit code all the same, §3.9). Keeping `require`
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
arguments (an edge/section/field the schema doesn't define). Coverage proof:

| verb | require |
|---|---|
| `claim` (task) | `neighbors: {depends_on: done}` |
| `submit` (task) | `sections: [validation]` |
| `close` (task) | `self: {status: approved}` |
| `close` (container) | `neighbors: {children: terminal}` |
| `release` (milestone) | `neighbors: {members: terminal}` |
| `archive` | `self: {status: terminal}` |
| `qa` | `self: {status: approved}` |
| `review`, `approve`, `request-changes`, `abandon`, edge verbs | none |

**Double duty:** these same operators power `lint` as *standing invariants*, not
just verb *preconditions*. `submit` gates `→review` on `sections: [validation]`;
`lint` checks the whole-graph version ("no `review`-status task lacks a
Validation section"), catching anyone who hand-edited around the verb (§ CLI
completeness, principles). One vocabulary, both jobs — enforce at transition
time, verify across the backlog. `lint` also surfaces the abandoned-dependency
decision (below) as a standing check.

**Deliberate exclusions** (each safe because no gate needs it):

- **No transitive closure.** Gates check *direct* neighbors only. The DAG
  invariant is maintained edge-by-edge (B can't be `done` unless B's own deps
  were resolved when B was claimed), so a direct `depends_on` check transitively
  implies the rest. Transitive queries are a *view* concern, never a `require`.
- **Only ∀** — no ∃, no counts, no "is a leaf" (`unit` is kind-based).
- **No disjunction / no nesting** — map is AND; OR is handled by two verb defs
  with disjoint `applies-to`, or a `do` hook.
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
  expressions. Anything relational (`child.assignee == self.assignee`) is a `do`
  hook, not `require`.

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

(This corrects an earlier draft that used `terminal` for both. v1 gated
container-close on children `== done`, which lets an abandoned child block its
parent forever — that half was right to change; deps were not.)

### 3.7 Templating

Primitive string arguments (`annotate` bodies, commit trailers, `hook` args)
flow through **simple `$var` substitution over a fixed context** — *no*
expressions or logic (KISS, and so a stateless agent can read a verb without
evaluating a DSL). The variable namespace is bounded and documented; starting
set: `$message` (CLI-supplied), `$slug`, `$kind`, `$title`, `$date`, `$actor`,
`$branch`.

> **Open:** finalize the `$var` namespace.

### 3.8 Creation: `new` is fixed tooling + a `templates` schema key

Creation is *genesis*, not a lifecycle mutation — it has no prior node and no
from-transition — so `new` stays **fixed tooling** (like `board`/`lint`), not a
verb, and no `scaffold` primitive is needed. Per-kind starter content is
declared in a dedicated schema key (working name **`templates`**, echoing v1's
`templates/` dir; `init` was considered but risks colliding with
workspace-initialization semantics):

```yaml
templates:
  task:  { status: todo, body: "## Goal\n\n## Acceptance\n\n## Notes\n" }
  epic:  { status: todo, body: "## Goal\n\n## Context\n\n## Stories\n" }
```

`planr new <kind> <slug> <title>` reads `templates.<kind>` to scaffold, with the
same `$var` substitution applied to the body.

### 3.9 Hook contract

Hooks live **only in a verb's `do`** — there is no hook in `require` (§3.6). A
hook is a subprocess whose exit code is a one-bit signal (`0` = proceed) and
which *may* cause external effects (CI, API, build). It is the sole extension
point for impure or bespoke gating: put the check in a `do` hook and let its
exit code veto the verb.

**Threat model: defend against mistakes, not adversaries.** A hook script lives
*in the repo* — same trust boundary as the source the worker compiles and the
tests the reviewer runs. Anyone who can write a hook can already write the build
script or the code under test, so sandboxing against a *malicious* hook guards a
door in a wall that isn't there. The real risk is a *buggy* hook that
accidentally corrupts the graph — a far weaker adversary.

**Integrity via git tamper-evidence: detect, don't prevent.** The verb engine
snapshots git state, runs the hook, and verifies the hook left no unexpected
modification to `.plan/`; if the tree came back dirty in a way the verb didn't
author, it aborts and reports. Git makes any write *visible*, and the engine
refuses to build on an unauthored change — a *practical* read-only guarantee
without the (theoretically impossible) *prevention* one. No bwrap, no container,
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
  content rollback). Order hooks *before* irreversible git ops (`merge`) so a
  veto aborts cleanly.
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
- **All structure lives in frontmatter** (kind, `parent`, grouping axes,
  `depends_on`, status). The directory no longer encodes the kind. Every
  grouping/view is derived at read time. **Multi-valued edges (`depends_on`,
  `link`) are stored as block lists (one target per line)**, not inline `[a, b]`
  — so concurrent `add-dep` of *different* targets land on different lines and
  git auto-merges (see optimistic concurrency below).
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

## 5. Archival — bounded working tree, lossless recovery

The problem: a mature project accumulates tens of thousands of tickets; a flat
`tickets/` (or any in-tree structure, including a manifest file) grows without
bound and slows every scan/clone.

**Resolution — git history *is* the archive:**

- **Retire** = remove the file from the working tree (`git rm`) in a commit
  whose **trailers carry the metadata** (`Planr-Retire: <slug>`,
  `Planr-Kind:`, `Planr-Status:`, tags). The full record is preserved in
  history (the pre-deletion blob); the working tree stops carrying it.
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

`archive` is just a configurable verb (`do: [archive]`, gated on
`self: {status: terminal}`), not a special subsystem — and it does *not* change
status (§3.3): an archived ticket keeps its terminal status; archival only
relocates the file to history.

### 5.1 The derived index

Referenced throughout (archival search, query speed, the DB alternative), so
defined once here: **the index is a disposable, git-ignored cache derived from
the source of truth — never authoritative.** Live tickets are derived by
scanning `tickets/` (as v1's board already does); cold tickets by walking
`git log --diff-filter=D -- tickets/`. Delete it and it rebuilds; it is never
committed and never merges. For most scales no persisted index is needed at all
— the graph is built in memory per invocation; the persisted cache is a
transparent optimization for when scan cost is *measured* to hurt (open
question #8). This is what preserves both "derive, don't store" and fast queries
without a database.

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

## 7. Backward compatibility & migration

- The **default schema** encodes v1: `kinds: [epic, story, task]`, the v1 verbs,
  the v1 lifecycle. Existing backlogs run unchanged. *(See R8: "byte-for-byte" is
  not literally true — the recommended default carries two intentional bug-fixes;
  a separate **strict-v1 preset** is the true byte-for-byte regression baseline.)*
- Migration from `epics/ stories/ tasks/` dirs to flat `tickets/` is mechanical
  (move files, drop numeric prefix, kind already in frontmatter). A `planr
  migrate` verb can do it.

## 8. Open questions

1. ~~Primitive set completeness~~ — **resolved.** Nine primitives (§3.5), closed
   for all known verbs; `annotate` added, `commit` is the verb boundary.
2. ~~`new`/scaffold~~ — **resolved.** `new` is fixed tooling; per-kind
   `templates` schema key (§3.7); no `scaffold` primitive.
3. ~~`require` predicate vocabulary~~ — **resolved** (§3.6). Three reserved
   operators (`self`, `neighbors`, `sections`), implicit-AND, all graph-derived
   (no hook in `require`); double-duty with `lint`; surfaced the needs-vs-
   decomposition asymmetry (`depends_on` gates on `done`, rollup on `terminal`).
4. **Templating `$var` namespace** — finalize the fixed variable set (§3.7).
   *(Default verb set completed via the pressure-test: `submit` / `approve` /
   `request-changes` / `release` added; v1's manual status edits are now verbs.
   Confirm the `{validation: present}` gate on `submit` is wanted.)*
5. ~~Axes schema surface~~ — **resolved by dissolution** (§3.2). Storage-side =
   ownership; all edges are one mechanism differentiated by a semantics tag; a
   new axis is a name + tag. Hook contract nailed in §3.9.
6. **Schema location & loading** — where the schema file lives
   (`.plan/schema.yml`?), how every stateless agent reliably loads and reads the
   *same* schema (the shared-mental-model bet), and which presets ship.
7. **`unit` = strictly the terminal kind, or any childless node?** Leaning
   terminal-only (keeps rollup-vs-work unambiguous).
8. **Index persistence** — pure in-memory rebuild per invocation vs a
   persisted git-ignored cache; when does scan cost justify persistence?
9. **Filesystem legibility** — is `board`/`graph` + generated symlink views
   enough to replace `ls`-by-kind for humans?

## 9. Fresh-eyes review findings (round 1, 2026-08-21)

An independent fresh-context review (no prior design context) cross-checked the
doc against the v1 source. Assessment column is *this author's* triage, not the
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
- **R5 — `require`↔`lint` "one vocabulary" is overstated (§3.6).** The lint form
  ("∀ nodes *where status==review*, has Validation") needs a **guarded
  quantifier** the require grammar excludes. *Agreed. Fix: shared operators +
  a per-state guard wrapper for the invariant form — not "identical grammar."*
- **R6 — Per-kind lifecycle / non-spine kinds unspecified (§3.3, §3.1).**
  Milestone's `planned→in_progress→released` has nowhere in the single global
  `lifecycle.states`, and `kinds` adjacency can't declare a non-spine kind
  (`parents: []` collides with epic-as-root). *Agreed; milestone-as-kind is
  asserted but not expressible — needs per-kind lifecycle + a grouping-kind role.*
- **R7 — Ref/actor model unspecified (§4/§8).** Where the reviewer runs
  `approve`/`request-changes` (branch vs trunk) decides the whole concurrency
  story. *Agreed; open. Presumed v1 model (reviewer verbs commit on the branch,
  `close` reads the `approved` state pre-merge, board sees it via branch-scan) —
  but the doc must say so. This is the deferred concurrency pressure-test.*

### Real but smaller

- **R8 — "Byte-for-byte v1" (§2/§7) is false.** We changed container-close
  (`done`→`terminal`) and turned manual edits into verbs. *Fix: split a
  **strict-v1 preset** (byte-for-byte, the regression baseline) from the
  **recommended default** (with the two intentional bug-fixes).*
- **R9 — `neighbors` argument namespace (§3.6).** `children`/`members` are
  *inverse-role* names absent from the `edges` map (which declares `parent`).
  *Fix: register inverse-role names.*
- **R10 — Cold-search index is O(history), not O(live) (§5.1).** Undersold.
  *Partly agreed; bounded — history is append-only, so the index is
  incrementally maintained (steady-state O(new)); the O(history) walk is a rare
  cold rebuild. This is the one place a persisted cache is genuinely justified.*
- **R11 — Primitive contracts thin (§3.5).** `merge`/`cleanup`/`archive` lack
  precise semantics / partial-failure behavior; `release` isn't decomposed.
  *Agreed as a tightening (not a design hole), except the R1 half — the set was
  not closed.*

### Pushed back on

- **R12 — Rollup-under-archival (reviewer's #13): rejected.** `archive` requires
  `self: {status: terminal}`, so an incomplete child can't be archived, and
  archiving an already-terminal child doesn't change the parent's closeability.
  Not a real problem.

### Nits (cheap fixes)

- `qa` example transitions `to: qa`, not in `lifecycle.states` (§3.4/§3.3) — a
  project adding a verb must add its state.
- `$branch` is undefined at `new`/creation time (§3.7) — template-var
  availability is context-dependent.
- `sections: [validation]` checks *existence*, not content (§3.6) — consistent
  with the structural pin, weaker than the prose implies; state it.
- Tamper-check (§3.9) vs. index location (§5.1) — the index must live outside the
  tamper-checked `.plan/` path (or be excluded) or a legit rebuild trips it.
- Naming: `neighbors` reads bidirectional but means one edge/one direction;
  `link` ("undirected-ish, cosmetic") is too vague for an agent to place vs
  `depends_on`.

### Validated as sound (no change)

Storage-side = ownership (§3.2); milestone-as-group-edge removing the epic-07
directory hack; needs-vs-decomposition asymmetry (§3.6, confirmed against v1
`close_cmd.rs`); per-kind available actions from `applies-to` (§3.4);
git-history-as-archive with commit trailers (§5). The design's *spine* holds; the
*mutation/enforcement actuators* (R1, R3, R7) are the unfinished part.
