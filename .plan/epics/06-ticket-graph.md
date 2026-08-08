---
id: ticket-graph
aliases: [ticket-graph]
kind: epic
title: "Ticket relationship graph: parse, query, traverse, and visualize all relationship types"
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [graph, relationships, visualization, query]
---

## Goal

Build a `TicketGraph` module that captures the full relationship graph across
all ticket types in the backlog — hierarchy edges (parent/child), ordering
edges (depends_on), and soft reference edges (wiki-links) — and exposes a rich
API for querying, traversing, and visualizing the graph. The module integrates
into the CLI as a `planr graph` subcommand and feeds into board/lint
enhancements.

## Context

The current planr already parses wiki-links (`extract_wiki_links` in parse.rs),
captures them in `ParsedTicket.links`, and lint warns on broken ones. But
there is no unified graph that combines all three edge types (hierarchy,
depends_on, wiki-links) into a single traversable structure. Queries like
"what depends on this transitively", "find paths between two tickets", or
"visualize the full dependency DAG" require ad-hoc scripting.

This epic builds a dedicated `graph.rs` module with:

- Adjacency maps for all three edge types
- Backlink index (reverse wiki-links — currently only available via
  `grep -rn '\[\[slug\]' .plan/`)
- Query methods: neighbors, transitive closure, path finding, topological sort
- Visualization output: Mermaid.js, GraphViz DOT, ASCII terminal tree
- A `planr graph` CLI subcommand
- Graph-aware lint and board enhancements

The existing [[backlinks-script]] task under [[utility-scripts]] is a simpler
shell-layer backlinks finder. This epic subsumes and supersedes that scope
with a proper Rust graph module, but the CLI integration ensures the same
ad-hoc discovery remains accessible.

## Stories

- [[graph-data-model]] — Core data structures and graph construction pipeline
- [[graph-query-traverse]] — Query and traversal API
- [[graph-visualize]] — Visualization and export (Mermaid, DOT, ASCII, metrics)
- [[graph-cli]] — Subcommand integration and graph-aware enhancements

## Out of scope

- Persistent graph storage (graph is always derived from current backlog)
- Graph mutation (no edges to add/remove — only ead-only queries)
- Interactive/web-based graph rendering (terminal output and file export only)
- Full Obsidian vault integration beyond what wiki-links already provide

## Notes

- 2026-08-08 created
