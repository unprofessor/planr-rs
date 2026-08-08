---
id: dot-output
aliases: [dot-output]
kind: task
parent: graph-visualize
title: GraphViz DOT output
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [visualization, dot, graphviz]
depends_on: [graph-types]
---

## Goal

Render the `TicketGraph` as a GraphViz DOT file, suitable for processing
with `dot`, `neato`, or `xdot` to produce SVG/PNG/PDF renderings.

## Context

DOT is the standard graph description language for GraphViz tools. The
output is a `digraph` with:

- Directed edges by default
- Edge colors and styles per kind:
  - Hierarchy: `[color=blue, style=solid]`
  - Depends_on: `[color=red, style=bold]`
  - Wiki-link: `[color=green, style=dashed]`
- Node attributes: `label="slug\nstatus"`, `shape=box`, `style=rounded`
- Subgraph clusters for each story/epic (if hierarchy is rendered)
- Optional `--filter slug` for subgraph extraction

```rust
pub fn render_dot(graph: &TicketGraph, filter: Option<&str>) -> String;
```

The output can be piped directly:

```bash
planr graph visualize --format dot | dot -Tsvg -o graph.svg
```

## Acceptance

- [ ] Output is valid DOT syntax with `digraph` header
- [ ] Edge colors/styles clearly distinguish the three edge kinds
- [ ] Node labels show slug and status
- [ ] Subgraph clusters for stories/epics (when hierarchy is included)
- [ ] Optional `--filter slug` renders subgraph
- [ ] Empty graph produces valid empty `digraph {}`
- [ ] Self-loops rendered with `[style=dashed, color=gray]`
- [ ] Unit tests: full graph, filtered, empty, DOT header pattern match

## Notes

- 2026-08-08 created
- Depends on [[graph-types]]; can run in parallel with query tasks
- No external dependencies — DOT is a text format
- Reference: <https://graphviz.org/doc/info/lang.html>
