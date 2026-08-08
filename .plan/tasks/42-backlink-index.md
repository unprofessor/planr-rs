---
id: backlink-index
aliases: [backlink-index]
kind: task
parent: graph-data-model
title: Build reverse wiki-link index
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [graph, wiki-links, backlinks]
depends_on: [graph-types, graph-construction]
---

## Goal

Add reverse wiki-link edges to the `TicketGraph` so callers can query "which
tickets reference slug X?" as efficiently as "which tickets does slug X
reference?".

## Context

The forward wiki-link map (`wiki_links: HashMap<String, Vec<String>>`) stores
edges from a ticket to slugs it links to. The backlink index inverts this:
for every wiki-link edge `A → B`, add a reverse wiki-link edge `B → A` that
is tagged as `EdgeKind::WikiLink` but distinguishable as a reverse edge
(so callers can avoid double-counting).

The backlink index is built during graph construction, not as a separate
pass. It lives in a separate field or is computed on-demand. Two approaches:

1. **Dual map**: store `wiki_links: HashMap<String, Vec<String>>` (forward)
   and `backlinks: HashMap<String, Vec<String>>` (reverse), built together.
2. **Unified undirected**: store one map but with entries in both directions
   — simpler but loses directionality.

Approach 1 is preferred: keep forward and reverse separate so callers can
distinguish "this ticket links TO" from "this ticket is linked FROM".

The function signature:

```rust
impl TicketGraph {
    pub fn links_from(&self, slug: &str) -> Vec<&str>;
    pub fn links_to(&self, slug: &str) -> Vec<&str>;
}
```

## Acceptance

- [ ] `links_to(slug)` returns all tickets whose body contains `[[slug]]`
  (or `[[slug|...]]`, `[[slug#...]]`)
- [ ] `links_from(slug)` returns all slugs referenced in this ticket's body
  (forward wiki-links, already captured in `ParsedTicket.links`)
- [ ] Backlinks exclude self-references (when a ticket links to itself)
  unless explicitly requested via `include_self: true`
- [ ] Backlinks are stable (same ticket set → same backlinks)
- [ ] Empty when no tickets reference the slug
- [ ] Unit tests: forward/backlink symmetry, self-link exclusion, multiple
  references to the same slug, no references

## Notes

- 2026-08-08 created
- Depends on [[graph-types]] and [[graph-construction]]
- Supersedes the existing [[backlinks-script]] approach: this is the Rust
  native version with a proper API
