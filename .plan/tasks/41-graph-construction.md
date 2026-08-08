---
id: graph-construction
aliases: [graph-construction]
kind: task
parent: graph-data-model
title: Build graph from backlog
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [graph, construction]
depends_on: [graph-types]
---

## Goal

Implement the pipeline that takes `Vec<ParsedTicket>` and populates the
`TicketGraph` adjacency maps for hierarchy edges and depends_on edges.
Wiki-link edges are handled by the [[backlink-index]] task.

## Context

Given a vector of parsed tickets (read from trunk or working tree by the
caller), the construction function:

1. Indexes tickets by id into the `tickets` map (slug → ParsedTicket).
2. For each ticket with a non-null `parent`, adds a hierarchy edge from
   parent → child.
3. For each ticket with non-empty `depends_on`, adds a depends_on edge
   from ticket → dependency.
4. Validates referential integrity: if a ticket's `parent` or `depends_on`
   references a slug not in the ticket set, that edge is still added
   (the graph is a superset of the backlog; lint is responsible for
   flagging orphans).

Construction is idempotent: calling it multiple times with the same input
produces identical graphs. The function signature is:

```rust
pub fn build_graph(tickets: &[ParsedTicket]) -> TicketGraph
```

## Acceptance

- [ ] `build_graph()` takes `&[ParsedTicket]` and returns `TicketGraph`
- [ ] Hierarchy edges: for each ticket with `parent: Some(p)`, adds
  `p → ticket.id` to hierarchy map
- [ ] Depends_on edges: for each ticket with non-empty `depends_on`, adds
  `ticket.id → dep` entries to depends_on map
- [ ] Tickets with `parent: null` or absent produce no hierarchy edges
- [ ] Tickets with empty `depends_on` produce no ordering edges
- [ ] Orphan references (parent/dep slug not in ticket set) are still
  added as edges — the graph faithfully represents the backlog even if
  it has errors
- [ ] Construction is order-independent (same result regardless of ticket
  vector order)
- [ ] Construction does not perform I/O (no file reads, no git calls)
- [ ] Unit tests: single ticket, small tree, cross-kind depends_on,
  orphan edges, empty backlog

## Notes

- 2026-08-08 created
- Depends on [[graph-types]] for the data structures
- The caller fetches tickets from trunk/branches; this function is pure
