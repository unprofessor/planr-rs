---
id: neighbor-queries
aliases: [neighbor-queries]
kind: task
parent: graph-query-traverse
title: Direct neighbor queries
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [graph, query, neighbors]
depends_on: [graph-types]
---

## Goal

Provide methods on `TicketGraph` to query a ticket's direct neighbors across
all edge kinds, with filtering. This is the foundation for all higher-level
traversal.

## Context

Neighbor queries return tickets directly connected to a slug via one hop.
Each method returns a result that preserves edge kind information:

```rust
impl TicketGraph {
    /// All neighbors, optionally filtered by edge kind.
    pub fn neighbors(&self, slug: &str, filter: NeighborFilter) -> NeighborResult;

    /// Parent in the hierarchy (singular — a ticket has one parent or none).
    pub fn parent(&self, slug: &str) -> Option<&str>;

    /// Direct children in the hierarchy.
    pub fn children(&self, slug: &str) -> Vec<&str>;

    /// Direct dependencies (depends_on targets).
    pub fn dependencies(&self, slug: &str) -> Vec<&str>;

    /// Direct dependents (tickets that list slug in their depends_on).
    pub fn dependents(&self, slug: &str) -> Vec<&str>;

    /// Outgoing wiki-links.
    pub fn links_from(&self, slug: &str) -> Vec<&str>;

    /// Incoming wiki-links (backlinks).
    pub fn links_to(&self, slug: &str) -> Vec<&str>;
}
```

Supporting types:

```rust
pub struct NeighborResult {
    pub edges: Vec<(String, EdgeKind)>,  // (neighbor_slug, edge_kind)
}

pub enum NeighborFilter { All, Only(EdgeKind), Except(EdgeKind) }
```

## Acceptance

- [ ] `neighbors()` returns all one-hop neighbors with edge kind annotations
- [ ] `parent()` returns the single hierarchy parent or `None`
- [ ] `children()` returns all tickets whose `parent` is the given slug
- [ ] `dependencies()` returns tickets listed in `depends_on`
- [ ] `dependents()` returns tickets that list slug in `depends_on` (reverse
  lookup using the depends_on adjacency)
- [ ] `links_from()` returns outward wiki-links
- [ ] `links_to()` returns inward wiki-links (backlinks)
- [ ] `NeighborFilter` works: `All` returns everything, `Only(k)` filters
  to one kind, `Except(k)` excludes one kind
- [ ] Non-existent slug returns empty/None gracefully (no panic)
- [ ] Unit tests: leaf node, root node, node with all three edge kinds,
  node with no edges, missing slug

## Notes

- 2026-08-08 created
- Depends on [[graph-types]] for the data structures
- Uses the adjacency maps built by [[graph-construction]] and
  [[backlink-index]]
