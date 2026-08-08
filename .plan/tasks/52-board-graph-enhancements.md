---
id: board-graph-enhancements
aliases: [board-graph-enhancements]
kind: task
parent: graph-cli
title: Graph-aware board enhancements
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [board, graph, enhancements]
depends_on: [stats-metrics, transitive-queries, graph-milestone-scope, milestone-board-views]
---

## Goal

Enhance `planr board` with optional graph-derived information: dependency
chain summaries, per-story completion percentages via graph traversal, and
blocked-cluster annotations.

## Context

When `planr board --graph` is used, the standard board output is augmented
with:

1. **Longest chain indicator** — appended to the summary section:

   ```
   longest dependency chain: 4  (ts-project-setup → parse-core → ...)
   ```

2. **Story/epic completion** — replaces the simple child-count with graph
   traversal: "all descendants done" instead of "all children done". This
   correctly handles stories whose tasks are nested under intermediate
   groupings.

3. **Blocked clusters** — when blocked tasks share a common un-met
   dependency, group them:

   ```
   blocked cluster: a, b, c ← d [todo]
   ```

4. **Wiki-link density** — for each story, show how many cross-references
   exist between its tasks (high density → strongly coupled).

These are opt-in with `--graph`; the default board output is unchanged.
Graph-derived views must respect the selected milestone scope and the
current/unplanned/completed board rules from the milestone board view task.

## Acceptance

- [ ] `planr board --graph` includes longest chain in summary
- [ ] Story/epic completion computed via descendant traversal (not just
  direct children)
- [ ] Blocked clusters shown when 2+ tasks blocked by same dependency
- [ ] Wiki-link density shown per story
- [ ] `planr board` without `--graph` is unchanged
- [ ] All existing board tests pass unchanged
- [ ] Empty backlog with `--graph` produces valid (brief) output

## Notes

- 2026-08-08 created
- Depends on `stats-metrics` and `transitive-queries`
- Modifies `src/board.rs` and `src/main.rs` to accept the `--graph` flag
