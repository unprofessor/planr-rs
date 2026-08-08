---
id: topological-sort
aliases: [topological-sort]
kind: task
parent: graph-query-traverse
title: Topological sort and critical path
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [graph, topological, critical-path]
depends_on: [transitive-queries]
---

## Goal

Implement topological ordering of the depends_on DAG and critical path
computation, giving users a recommended implementation order and the
longest dependency chain.

## Context

For a set of tickets with `depends_on` edges, a topological sort gives a
valid implementation order (every ticket appears after its dependencies).
The critical path is the longest path through the DAG weighted by node
status — tickets on the critical path are the ones that most affect the
overall timeline.

```rust
impl TicketGraph {
    /// Topological order of all tickets (or a subgraph) using Kahn's algorithm.
    /// Returns None if the subgraph has a cycle.
    pub fn topological_sort(
        &self,
        filter: Option<&[String]>,  // restrict to these slugs
    ) -> Option<Vec<String>>;

    /// Critical path through the depends_on DAG. Edge weight = 1; node
    /// weight = 0 (unweighted) or configurable.
    pub fn critical_path(
        &self,
        subgraph: Option<&[String]>,
    ) -> Vec<String>;

    /// Detect cycles in the depends_on subgraph (beyond what lint already
    /// reports).
    pub fn has_cycle(&self) -> bool;

    /// The longest dependency chain length (max depth).
    pub fn longest_chain(&self) -> usize;
}
```

- Kahn's algorithm for topological sort (O(V+E))
- Critical path via longest-path in a DAG (topological order + relaxation)
- Cycle detection as a side-effect of Kahn's (nodes remaining → cycle)
- Optionally restrict to a subset of tickets (e.g., all tasks in one story)

## Acceptance

- [ ] `topological_sort()` returns a valid topological order respecting all
  depends_on edges
- [ ] `topological_sort()` returns `None` when the subgraph has a cycle
- [ ] `critical_path()` returns the longest path through the depends_on DAG
- [ ] `critical_path()` on a subgraph returns the longest chain within it
- [ ] `has_cycle()` returns true iff depends_on contains a cycle
- [ ] `longest_chain()` returns max depth of depends_on subgraph
- [ ] Unit tests: linear chain, fan-out, fan-in, diamond, cycle,
  disconnected, single node, empty graph

## Notes

- 2026-08-08 created
- Depends on [[transitive-queries]] for the transitive dependency view
- Cycle detection here complements `lint.rs`; the graph module exposes
  the algorithmic result directly for `planr graph topo`
