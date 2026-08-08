---
id: mermaid-output
aliases: [mermaid-output]
kind: task
parent: graph-visualize
title: Mermaid.js flowchart output
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [visualization, mermaid]
depends_on: [graph-types]
---

## Goal

Render the `TicketGraph` as a Mermaid.js flowchart — the primary output
format for embedding in markdown documents, PR descriptions, and Obsidian.

## Context

Mermaid.js is widely supported in GitHub, GitLab, and Obsidian markdown
rendering. The output is a ` ```mermaid ` code block containing a
`flowchart LR` (left-to-right) graph definition.

Edge styling:

- Hierarchy edges: solid arrow `-->` (parent → child)
- Depends_on edges: thick solid arrow `==>` (task → dependency)
- Wiki-link edges: dashed arrow `-.->` (ticket → linked ticket)
- Self-loops: circular arrow with label

Node labels show ticket ID and status (e.g., `[proxy]` → `proxy [review]`).
Clickable links (`click proxy href "../tasks/01-proxy.md"`) enable navigation
in Obsidian and GitHub.

```rust
pub fn render_mermaid(graph: &TicketGraph, filter: Option<&str>) -> String;
```

When `filter` is provided, only the subgraph reachable from that slug is
rendered. The output includes the `flowchart LR` declaration and a
`classDef` for status-based styling (done=green, blocked=red, etc.).

## Acceptance

- [ ] Output is valid Mermaid.js flowchart syntax with ` ```mermaid ` fence
- [ ] `flowchart LR` orientation with all three edge styles distinguishable
- [ ] Node labels: `slug [status]` (e.g., `http-connect-proxy [review]`)
- [ ] Status-based CSS classes: `classDef done fill:#4caf50`, `classDef
  blocked fill:#f44336`, `classDef review fill:#ff9800`, etc.
- [ ] Clickable nodes (`click slug href "...")` pointing to task files
- [ ] Optional `--filter slug` renders only the subgraph reachable from
  that slug
- [ ] Empty graph produces valid empty Mermaid (`flowchart LR` with no
  nodes)
- [ ] Self-loop wiki-links rendered as a circular edge
- [ ] Unit tests: full graph, filtered subgraph, empty graph, single node,
  cycle in wiki-links only

## Notes

- 2026-08-08 created
- Depends on [[graph-types]]; doesn't need query/traversal module
- No external dependencies — Mermaid is a text format
- Reference: <https://mermaid.js.org/syntax/flowchart.html>
