---
id: graph-visualize
aliases: [graph-visualize]
kind: story
parent: ticket-graph
title: Visualization and export
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [graph, visualization, mermaid, dot]
depends_on: [graph-data-model]
---

## Goal

Render the `TicketGraph` into multiple output formats: Mermaid.js flowcharts,
GraphViz DOT files, ASCII terminal trees, and summary metrics. This lets
users visualize the issue graph in their editor, in PR descriptions, or
directly in the terminal.

## Context

Graph visualization is the primary way humans make sense of complex
dependency structures. From the CLI, users need:

- **Mermaid.js** — embeddable in GitHub/GitLab markdown, PR descriptions,
  and Obsidian. The canonical output format.
- **DOT** — opens in GraphViz tools (dot, xdot, etc.) for interactive
  exploration and SVG/PNG export.
- **ASCII tree** — quick terminal view without any external tooling.
  Indented tree view for hierarchy; compact graph for dependencies.
- **Metrics** — summary statistics: number of nodes, edges per type,
  longest chain, connected components, degree distribution.

Each visualizer is a pure function: `TicketGraph → String`. The caller
decides output format and filters (subgraph for a specific story/epic,
only depends_on edges, only hierarchy, etc.).

## Acceptance

- [ ] [[mermaid-output]] done: renders full graph or subgraph as Mermaid.js
  flowchart with styled edges (directed for hierarchy/depends_on, dashed for
  wiki-links)
- [ ] [[dot-output]] done: renders full graph or subgraph as DOT format
  with colored/dashed edge styles per type
- [ ] [[ascii-terminal]] done: renders hierarchy as indented tree,
  depends_on subgraph as compact adjacency list
- [ ] [[stats-metrics]] done: prints node count, edge counts per type,
  min/max/mean degree, longest chain length, connected components count,
  cycle count
- [ ] All outputs accept `--filter slug` to limit to a subgraph
  (reachable from or related to a ticket)
- [ ] Mermaid output wraps nodes in clickable links (`click NodeUrl`) when
  tickets have known file paths
- [ ] Unit tests verify key output patterns (Mermaid header, DOT digraph
  wrapper, tree indentation)

## Notes

- 2026-08-08 created
- Depends on [[graph-data-model]] for the graph; can run in parallel
  with [[graph-query-traverse]]
- No external dependencies: DOT and Mermaid are text formats; ASCII tree
  is terminal-safe Unicode/ANSI
- Mermaid output targets the `flowchart LR` orientation for horizontal
  layout; ticket IDs become node labels, titles shown in tooltip/click
