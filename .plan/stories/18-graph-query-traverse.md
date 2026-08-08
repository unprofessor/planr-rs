---
id: graph-query-traverse
aliases: [graph-query-traverse]
kind: story
parent: ticket-graph
title: Query and traversal API
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [graph, query, traversal]
depends_on: [graph-data-model]
---

## Goal

Provide a rich query and traversal API on top of the `TicketGraph` from
[[graph-data-model]]. Callers can ask for neighbors, transitive closure,
paths, topological order, and critical path information across all three
relationship types.

## Context

With the graph data model in place (adjacency maps for hierarchy, depends_on,
and wiki-link edges), this story adds methods that answer questions like:

- "What tickets does slug X depend on transitively?"
- "What tickets transitively depend on slug X?"
- "What's the shortest path between ticket A and ticket B?"
- "Is there a cycle in the depends_on subgraph?" (lint already detects this,
  but the graph module should expose it)
- "What's the topological order of tasks in story Y?"
- "What's the critical path through the dependency graph for epic Z?"

The API follows a builder/fluent pattern where callers can filter by edge
kind: `.through_hierarchy()`, `.through_depends_on()`, `.through_wiki_links()`,
or `.through_all()` (default). This lets callers ask targeted questions
without traversing irrelevant edge types.

## Acceptance

- [ ] [[neighbor-queries]] done: direct neighbors — children, parent,
  dependencies, dependents, outward links, inward backlinks; all filterable
  by edge kind
- [ ] [[transitive-queries]] done: BFS/DFS-based transitive closure for
  forward (depends / children / outward links) and reverse
  (dependents / parent chain / backlinks) traversal
- [ ] [[path-finding]] done: shortest path (BFS unweighted) and all-simple-paths
  (DFS with visited set) between two tickets; optional edge-kind filters
- [ ] [[topological-sort]] done: Kahn's algorithm for depends_on subgraph;
  cycle detection; critical path (longest path through depends_on DAG with
  status-weighted nodes)
- [ ] Every query returns typed results (not raw string lists) — callers
  know which edge each result came from
- [ ] Unit tests cover: empty graph, linear chain, fan-out/fan-in, diamond,
  disconnected components, cycle in subgraph

## Notes

- 2026-08-08 created
- Depends on [[graph-data-model]] — the graph must exist first
- All queries are read-only (no mutation of the underlying graph)
- Traversal uses standard BFS/DFS with visited sets; graph is small enough
  (hundreds of tickets) that recursion depth is not an issue
