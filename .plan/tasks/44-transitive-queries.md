---
id: transitive-queries
aliases: [transitive-queries]
kind: task
parent: graph-query-traverse
title: Transitive closure queries
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [graph, transitive, traversal]
depends_on: [neighbor-queries]
---

## Goal

Implement BFS/DFS-based transitive closure queries: given a starting slug,
find all tickets reachable following edges of a given kind (or all kinds).

## Context

Transitive queries answer "what does this depend on, directly or indirectly?"
and "what depends on this, directly or indirectly?". They traverse the graph
following edges until no new nodes are found (or up to a configurable depth
limit).

```rust
impl TicketGraph {
    /// All tickets reachable following depends_on edges forward.
    pub fn transitive_dependencies(&self, slug: &str, filter: NeighborFilter) -> Vec<String>;

    /// All tickets that can reach this slug via depends_on edges (reverse).
    pub fn transitive_dependents(&self, slug: &str, filter: NeighborFilter) -> Vec<String>;

    /// All descendants in the hierarchy tree.
    pub fn descendants(&self, slug: &str) -> Vec<String>;

    /// All ancestors in the hierarchy chain (parent, grandparent, ...).
    pub fn ancestors(&self, slug: &str) -> Vec<String>;

    /// Transitive outward wiki-links (links from links from ...).
    pub fn transitive_links_from(&self, slug: &str, depth: Option<usize>) -> Vec<String>;

    /// Transitive inward wiki-links (backlinks of backlinks).
    pub fn transitive_links_to(&self, slug: &str, depth: Option<usize>) -> Vec<String>;
}
```

Traversal uses BFS for shortest-path-first exploration and DFS for
exhaustive exploration. A `depth` parameter caps traversal depth (default:
no limit). Cycles are handled by a visited set — no infinite loops.

## Acceptance

- [ ] `transitive_dependencies()` returns closure of depends_on edges
  (A → B, B → C => A returns [B, C])
- [ ] `transitive_dependents()` returns reverse closure
- [ ] `descendants()` returns all tickets recursively under a parent
- [ ] `ancestors()` returns parent chain up to epic root
- [ ] `transitive_links_from/to()` respect depth limit
- [ ] Cycle-safe: visited set prevents re-visiting nodes
- [ ] `NeighborFilter` scopes traversal to specific edge kinds
- [ ] Unit tests: linear chain, fan-out, fan-in, diamond, cycle guarded,
  disconnected components, root-to-leaf

## Notes

- 2026-08-08 created
- Depends on [[neighbor-queries]] for the one-hop interface
- Transitive depends_on queries are the basis for the topological sort
  and critical path tasks
