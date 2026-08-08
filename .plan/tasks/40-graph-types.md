---
id: graph-types
aliases: [graph-types]
kind: task
parent: graph-data-model
title: Core TicketGraph data structures
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [graph, data-structures]
depends_on: []
---

## Goal

Define the `TicketGraph` struct and supporting types that represent the full
relationship graph between tickets. This is the data model all subsequent
graph tasks build on.

## Context

A new module `src/graph.rs` defines:

- **`EdgeKind`** enum: `Hierarchy`, `DependsOn`, `WikiLink`
- **`GraphEdge`** struct: `{ from: String, to: String, kind: EdgeKind }`
- **`TicketGraph`** struct with three adjacency maps:
  - `hierarchy: HashMap<String, Vec<String>>` — parent → children (directed,
    tree-shaped)
  - `depends_on: HashMap<String, Vec<String>>` — ticket → its dependencies
    (directed, acyclic)
  - `wiki_links: HashMap<String, Vec<String>>` — ticket → tickets it
    references (directed, but treated as undirected for backlink queries)
  - `tickets: HashMap<String, ParsedTicket>` — id → ticket for metadata
    lookups
- **`NeighborFilter`** enum: `All`, `Only(EdgeKind)`, `Except(EdgeKind)`
- **`Path`** struct: `{ nodes: Vec<String>, total_weight: usize }`

All maps index by ticket slug (string). The graph is intentionally a
separate simple structure rather than wrapping a generic petgraph — it keeps
the dependency footprint zero and the code understandable.

## Acceptance

- [ ] `TicketGraph` stores three typed adjacency maps (hierarchy, depends_on,
  wiki_links) plus a ticket metadata map
- [ ] `EdgeKind` distinguishes all three relationship types
- [ ] `NeighborFilter` supports filtering by edge kind for all query methods
- [ ] `Path` carries ordered node list and optional weight
- [ ] Graph stores `N` tickets with `O(E)` memory for all edge types
- [ ] No external graph libraries required (pure std::collections)
- [ ] Unit tests: empty graph, graph with all three edge types, edge
  deduplication (same slug pair via two kinds)

## Notes

- 2026-08-08 created
- File: `src/graph.rs` (new module, register in main.rs)
- No I/O — pure data structure
- Adjacency is stored as `HashMap<String, Vec<String>>` — each entry maps
  a source slug to a list of target slugs. Reverse lookups (backlinks,
  dependents, parent) are provided by the backlink-index task.
