---
id: path-finding
aliases: [path-finding]
kind: task
parent: graph-query-traverse
title: Path finding and reachability
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [graph, path, bfs]
depends_on: [neighbor-queries]
---

## Goal

Implement shortest-path and all-simple-paths algorithms on the `TicketGraph`,
letting users find how two tickets are connected across any combination of
edge types.

## Context

Path finding answers questions like "why is task A blocked on task D?"
(shortest path through depends_on + hierarchy edges) or "how are these two
unrelated tickets connected?" (path across wiki-link edges).

```rust
impl TicketGraph {
    /// Shortest path between two slugs (BFS, unweighted).
    pub fn shortest_path(
        &self,
        from: &str,
        to: &str,
        filter: NeighborFilter,
    ) -> Option<Path>;

    /// All simple paths up to max_length between two slugs (DFS with
    /// visited set).
    pub fn all_simple_paths(
        &self,
        from: &str,
        to: &str,
        max_length: usize,
        filter: NeighborFilter,
    ) -> Vec<Path>;

    /// Is there any path between two slugs?
    pub fn reachable(&self, from: &str, to: &str, filter: NeighborFilter) -> bool;

    /// All tickets reachable from slug through given edge kinds.
    pub fn reachable_set(&self, from: &str, filter: NeighborFilter) -> Vec<String>;
}
```

- BFS for shortest path (all edges have equal weight)
- DFS with visited set for all simple paths (no node repeated in a path)
- `max_length` caps all-simple-paths to avoid combinatorial explosion
- `EdgeKind` filtering scopes the search (e.g., depends_on only, or all)

## Acceptance

- [ ] `shortest_path()` returns the minimal-hop path between two slugs, or
  `None` if unreachable
- [ ] `all_simple_paths()` returns every non-cyclic path up to `max_length`
- [ ] `reachable()` is a boolean predicate (O(V+E) worst case)
- [ ] `reachable_set()` returns all nodes reachable from start
- [ ] `NeighborFilter` scopes search (e.g., depends_on only, excluding
  wiki-links)
- [ ] Same-slug query returns a zero-length path (reachable to itself)
- [ ] Path order is deterministic for same inputs
- [ ] Unit tests: direct edge, linear chain, diamond (multiple equal-length
  paths), disconnected, self-path, filtered vs unfiltered

## Notes

- 2026-08-08 created
- Depends on [[neighbor-queries]] for one-hop access
- Graph is small enough that BFS/DFS are efficient without heuristics
