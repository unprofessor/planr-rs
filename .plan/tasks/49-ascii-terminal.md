---
id: ascii-terminal
aliases: [ascii-terminal]
kind: task
parent: graph-visualize
title: ASCII terminal graph view
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [visualization, terminal, ascii]
depends_on: [graph-types, transitive-queries]
---

## Goal

Render the `TicketGraph` to the terminal as an indented tree (for hierarchy)
and compact adjacency lists (for depends_on/wiki-links). No external tools
required — pure ASCII/Unicode output.

## Context

For quick terminal glances without piping to GraphViz or a browser, the
ASCII view provides two modes:

1. **Tree view** — hierarchy rendered as an indented tree with box-drawing
   characters (`├──`, `└──`, `│`). Each node shows `slug [status]`. This is
   the primary terminal view.

2. **Depends-on view** — for a given slug, show its transitive dependencies
   as a compact indented list:

   ```
   http-connect-proxy [todo]
   └── parse-foundation [done]
       └── ts-project-setup [done]
   ```

3. **Wiki-link view** — for a given slug, show its direct and transitive
   wiki-link connections.

```rust
pub fn render_ascii_tree(graph: &TicketGraph, root: Option<&str>) -> String;
pub fn render_ascii_deps(graph: &TicketGraph, slug: &str) -> String;
pub fn render_ascii_links(graph: &TicketGraph, slug: &str) -> String;
```

## Acceptance

- [ ] `render_ascii_tree()` renders full hierarchy as indented tree with
  box-drawing characters
- [ ] `render_ascii_tree(Some(root))` renders subtree starting at root
- [ ] `render_ascii_deps()` renders transitive dependency chain as compact
  list
- [ ] `render_ascii_links()` renders inward and outward wiki-links for a slug
- [ ] Output uses Unicode box-drawing (`├── └── │`) with ASCII fallback
  suggestion
- [ ] Output is visually parsable at a glance (indentation depth visible)
- [ ] Empty/nonexistent root produces a message, not a panic
- [ ] Unit tests: single root, deep tree, flat structure, dependency chain,
  no children, filtered output patterns

## Notes

- 2026-08-08 created
- Depends on [[graph-types]] and [[transitive-queries]] (for dependency view)
- Pure string formatting, no external dependencies
