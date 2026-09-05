# planr 0.4 -- semantics of the verb language

A companion to [the design](typed-graph-design.md), which says what the model
is *for*. This says what it *means*.

The point is the forcing function. Writing the rules down makes the case
analysis exhaustive and the assumptions nameable, and that is where the value
is -- not in mechanization. Nothing here is machine-checked, and it does not
need to be. It does need to stay honest: `src/next/schema.rs` enforces the
well-formedness rules, and a test enumerates the whole product space against
the table in section 2, so a rule added here without a rule added there fails
the build.

There is deliberately **no grammar** in the BNF sense. The surface syntax is
YAML and is given; an EBNF would describe a serialization format, not the
language. What is underspecified is meaning.

## 0. Notation

A schema declares a set of kinds `K` and a set of verbs `V`. For a verb `v`:

| | |
| --- | --- |
| `base(v)` | `home` or `own` -- which ref the new tree is built from |
| `effect(v)` | `advance`, `create`, `merge`, or `ticket-only` -- the one ref movement |
| `wt(v)` | `create`, `remove`, or absent -- the workspace action |
| `from(v)`, `to(v)` | optional state names |
| `req(v)` | the guard: `self`, `sections`, `neighbors` |
| `content(v)` | a list of tree transforms |

For a ticket `t`: `own(t)` is the ref `plan/<kind>/<slug>`; `home` is trunk.

## 1. Abstract syntax

```
  verb    ::= require x content x base x effect x worktree x (from?, to?)
  base    ::= home | own
  effect  ::= advance | create | merge | ticket-only
  wt      ::= create | remove | (absent)
```

## 2. Well-formedness

The judgment is `|- v wf`. The rules are stated **positively**: a verb is
well-formed exactly when it matches one of five shapes.

That framing is the substance of this section, not a presentational choice.
The implementation originally stated the same constraints as four
*prohibitions*, and prohibitions leave gaps by construction -- you cannot tell
by looking whether the list is complete. Five positive rules close the world:
anything not matching is ill-formed, and the partition below is then evident
rather than hoped for.

```
   base(v) = home    effect(v) = advance
   ------------------------------------- [W-Declare-Home]
                |- v wf

   base(v) = home    effect(v) = create
   ------------------------------------- [W-Cut]
                |- v wf

   base(v) = home    effect(v) = ticket-only
   ----------------------------------------- [W-Retire]
                |- v wf

   base(v) = own     effect(v) = advance
   ------------------------------------- [W-Declare-Own]
                |- v wf

   base(v) = own     effect(v) = merge
   ------------------------------------- [W-Integrate]
                |- v wf
```

The `base x effect` space is 2 x 4 = 8, partitioned 5 / 3:

| | `advance` | `create` | `merge` | `ticket-only` |
| --- | --- | --- | --- | --- |
| **`home`** | W-Declare-Home | W-Cut | *ill-formed* | W-Retire |
| **`own`** | W-Declare-Own | *ill-formed* | W-Integrate | *ill-formed* |

Every legal cell is inhabited in the reference schema: `close(epic)`, `claim`,
`abandon`, `submit`, `close(task)`. The language has no dead corners.

### 2.1 The workspace side condition

`wt(v) = create` attaches a worktree to `own(t)`, so `own(t)` must exist
**after the step**, not merely before it:

```
   wt(v) = create    |- v wf by W-Cut or W-Declare-Own
   ------------------------------------------------- [W-Worktree]
                       |- v wf
```

W-Cut establishes `own(t)`; W-Declare-Own requires it and leaves it in place.
The other three shapes each fail for a different reason, and the third is the
one an enumeration finds and prose does not:

- **W-Declare-Home** never establishes `own(t)`, so there may be nothing to
  attach to.
- **W-Retire** releases `own(t)`.
- **W-Integrate** releases `own(t)` -- *after* validating that it exists.

That last case is a precondition that is true when checked and false when used.
Stated as a prohibition on `base` it is invisible, because `base(v) = own` is
satisfied; stated as a requirement on the post-state it is immediate. Left
unfixed it does not error -- it produces a worktree pointing at a deleted
branch and reports success.

`wt(v) = remove` carries no side condition. Note the redundancy it creates:
`merge` and `ticket-only` already remove the worktree as part of the effect, so
under W-Integrate and W-Retire the declaration says nothing. Two mechanisms for
one concern; see [open questions](#8-open-questions-this-raises).

### 2.2 Derived properties of a kind

Three properties are computed from the verb set rather than declared:

```
  initial(k)  = the s with s in from(V_k) and s not in to(V_k)
                -- or templates.k.initial when that set is empty
  terminal(k) = { s : s in to(V_k) and s not in from(V_k) }
  unit        = the k whose verbs include some v with wt(v) = create
```

`initial(k)` is a *derive-or-declare*: a rework cycle (`yield` returning a task
to `todo`) makes `todo` both a `from` and a `to`, so nothing is derivable and
the template supplies it.

`unit` is currently under-specified in the implementation: it takes the first
kind in the first matching verb's `applies-to`, so a `claim` applying to two
kinds silently picks one. Under this presentation that is an **ambiguity to
reject**, not a value to compute.

## 3. Operational semantics of `do`

A configuration is `(R, G, W)`: refs (name -> sha), the commit graph, and
worktrees. The judgment is

```
   (R, G, W)  --v(t)-->  (R', G', W')
```

for verb `v` applied to ticket `t`, defined when every premise holds:

1. **Resolve the base.** `b = home` if `base(v) = home`, else `own(t)`, which
   must exist in `R`.
2. **Guard, structural.** If `from(v)` is defined, `state(t) = from(v)`.
   Otherwise `state(t)` is not in `terminal(kind(t))` -- unless `req(v)` names
   `self.status`, in which case the explicit precondition governs and the
   absorbing rule is not applied.
3. **Guard, declared.** `req(v)` holds: `self` attributes, `sections`
   existence, and `neighbors` universally over one direct edge.
4. **Build.** `T' = content(v)` applied to the tree of `b`.
5. **Commit.** `c = commit(T', parents = [b], msg)` where `msg` carries
   `Planr-Verb: v` and `Planr-Ticket: t`. This extends `G` only; `c` is
   unreferenced.
6. **Move exactly one ref**, per `effect(v)`, as a compare-and-swap.
7. **Act on the workspace**, per `wt(v)`. Touches `W` only.

### 3.1 Properties

**One ref moves.** Step 6 is the only mutation of `R`. `create` is a CAS
against absence; `advance` a CAS against the observed base; `merge` and
`ticket-only` move `home` and then release `own(t)`.

**Atomicity is per-step, not per-command.** If the CAS in step 6 fails, `R` is
unchanged and the step fails as a whole. `G` is append-only and may retain the
commit from step 5 as garbage. This is the intended trade: an unreferenced
commit is inert, and the alternative -- a lock -- does not survive concurrent
agents in separate worktrees.

**The workspace is never history.** Step 7 cannot fail the step, and nothing in
`R` or `G` depends on it. A missing worktree is a workspace problem.

## 4. Denotational semantics of the fold

State is not stored. It is the meaning of a ticket's event sequence.

Each event `e` denotes an endofunction on state:

```
   [[e]] = const s   when to(verb(e)) = s
         = id        otherwise
```

and the fold is composition, applied to the initial state:

```
   fold(e_1 ... e_n) = ([[e_n]] o ... o [[e_1]]) (initial(kind))
```

**Lemma (absorption).** `const s o f = const s` for every `f`.

**Corollary (last-write-wins).** `fold(e_1 ... e_n) = to(verb(e_k))` where `k`
is the largest index with `to(verb(e_k))` defined, and `initial(kind)` if no
such index exists.

**Corollary (the backwards bound).** A right-to-left scan of the event sequence
may stop at the first event with a defined `to`. Every earlier event is
annihilated by absorption, so the prefix cannot affect the result.

That last corollary is why a state read costs "commits since the ticket last
moved" rather than "commits since the ticket existed" -- see
[the follow-on probes](typed-graph-design.md#follow-on-probes-topology-bounds-and-gits-index).
It is a consequence of the denotation, not a heuristic that happens to work.
`new` carries no `to`, so it does not terminate the scan; a ticket that has
never transitioned needs its creation commit as the floor.

## 5. Where the read and write semantics diverge

`from(v)` is enforced in step 2 of the transition relation and **ignored by the
fold**. The fold reads only `to`.

This is deliberate and worth stating rather than discovering. The consequence
is that the language assigns a meaning to event sequences the runtime would
never produce -- a rewritten history, a merge of two branches that both
declared, or a synthesized migration chain. The fold reports a state for all of
them instead of failing.

The cost lands on migration: a `planr migrate` that seeds a 0.3 ticket's event
chain can emit a sequence no verb sequence could have produced, and nothing
will object. If that is unacceptable, the check belongs in the migrator, not in
the fold -- making the fold total is what makes it robust.

## 6. Assumptions this rests on

Named so that breaking one is a decision rather than an accident.

1. **Transitions are state-independent.** Every verb's effect on state is a
   constant or the identity. A verb whose `to` depended on the current state --
   a retry counter, a conditional transition -- breaks absorption, and the
   backwards bound of section 4 becomes *wrong*, not merely slower. This is the
   most expensive assumption in the document and the least visible in the YAML.
2. **Event order is total, and given by `--date-order` over trunk and the
   `plan/` refs together.** Committer-date skew across machines breaks it.
   Under a terminating backwards scan a misordering is not one wrong event
   among many; it is the whole answer.
3. **A ticket's events all descend from its creation commit**, which is what
   makes that commit a valid floor.
4. **The schema in force is the one in the same history.** There is no schema
   trailer, deliberately: the schema is tracked in the repository it governs.
5. **Trailers survive.** Events are attributable because commit messages are
   immutable; a history rewrite that drops trailers drops events.

## 7. What this does not cover

The textual semantics of the content transforms (`annotate`, `edge`, `remove`);
merge conflict resolution; concurrency beyond the single-ref CAS; and the
filesystem state of worktrees. These are mechanism, and the design document
describes them.

## 8. Open questions this raises

- **The `worktree` axis is not really an axis.** `merge` and `ticket-only`
  remove worktrees inside the effect, while `wt(v) = remove` does it
  explicitly. One concern, two mechanisms. Either the effect should not touch
  the workspace, or `remove` should not be declarable.
- **`unit` should reject ambiguity** rather than resolve it by position.
- **Is `advance` on `home` with no content meaningful?** It is well-formed by
  W-Declare-Home and produces an empty commit whose entire payload is its
  trailers. That is the intended design, but it is also what forfeits git's
  changed-path filters for enumeration.
