---
id: graph-data-model
aliases: [graph-data-model]
kind: story
parent: ticket-graph
title: Graph data model and construction pipeline
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [graph, data-model, construction]
depends_on: []
---

## Goal

Build the core `TicketGraph` data structures and the pipeline that constructs
the graph from the backlog. This is the foundation that all other stories
in epic [[ticket-graph]] build on.

## Context

The graph captures three relationship types between tickets:

1. **Hierarchy edges** — derived from the `parent:` frontmatter field. A
   story's parent is its epic; a task's parent is its story. These create
   a tree: epics → stories → tasks.
2. **Ordering edges** — derived from `depends_on:` frontmatter. These express
   "must be done before" constraints between any two tickets (cross-kind
   edges are valid). Together with hierarchy edges, these form a DAG.
3. **Reference edges** — derived from `[[wiki-links]]` in the body text.
   These are soft, non-blocking relationships: "references", "related to",
   "discovered while working on". They add undirected edges on top of the
   directed hierarchy/ordering edges.

The graph module (`src/graph.rs`) is a pure data structure with no I/O.
Construction takes `Vec<ParsedTicket>` and produces an adjacency-indexed
graph that downstream consumers (queries, traversal, visualization) operate
on without re-parsing.

## Acceptance

- [ ] [[graph-types]] done: `TicketGraph` struct with typed adjacency maps
  for each edge category
- [ ] [[graph-construction]] done: pipeline reads `Vec<ParsedTicket>` and
  populates all adjacency maps
- [ ] [[backlink-index]] done: reverse wiki-link index maps slug → slugs
  that reference it
- [ ] Edge categories are distinguishable (caller can ask "is this a
  hierarchy edge?", "is this a depends_on edge?", "is this a wiki-link?")
- [ ] Graph construction is idempotent and order-independent
- [ ] Unit tests cover: empty backlog, single ticket, full backlog with
  mixed edge types
- [ ] `depends_on: []` (or field absent) produces no ordering edges for
  that ticket
- [ ] `parent: null` (or field absent) produces no hierarchy edge for
  that ticket
- [ ] Self-referencing wiki-links are included but tagged as self-loops

## Notes

- 2026-08-08 created
- Builds directly on the existing `ParsedTicket` struct from `ticket.rs`
- No I/O — construction takes already-parsed tickets
- The graph is always derived (read-only); no mutation API in this story
