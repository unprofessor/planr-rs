---
id: lint-graph-enhancements
aliases: [lint-graph-enhancements]
kind: task
parent: graph-cli
title: Graph-aware lint enhancements
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [lint, graph, enhancements]
depends_on: [graph-types, transitive-queries]
---

## Goal

Add graph-aware lint checks that go beyond what the current three-pass
lint engine (per-file, cross-ref, cycle detection) can catch.

## Context

The existing `lint.rs` already checks parents, depends_on pointers, and
wiki-links at a syntactic level (do the referenced slugs exist?). The graph
module enables structural lint checks:

1. **Orphan detection** — tickets whose parent slug is not in the graph
   (already covered by lint pass 2, but re-exposed via graph query)

2. **Cluster coherence** — warn when closely related tickets (same story,
   heavy wiki-link cross-references) have diverging statuses without
   explicit depends_on edges. For example: ticket A is `done`, ticket B
   is `todo`, they share the same story and cross-reference each other
   via wiki-links, but B does not depend on A.

3. **Missing cross-links** — warn when two tickets in the same story
   reference the same external concept but don't wiki-link each other
   (heuristic: shared `[[slug]]` in body without mutual wiki-link).

4. **Dependency depth warning** — warn when a ticket's transitive
   dependency chain exceeds a threshold (default: 5). Deep chains are
   fragile and suggest over-splitting.

Graph lint checks are integrated as a new phase in `lint.rs` pass 3 (after
cycle detection), gated behind the existing lint engine with new
`LintIssue` entries.

## Acceptance

- [ ] Orphan detection via graph query (parents not in graph) produces
  errors
- [ ] Cluster coherence warns on same-story tickets with heavy cross-links
  but no depends_on edges
- [ ] Missing cross-links heuristic warns on shared `[[slug]]` references
  without mutual wiki-links
- [ ] Dependency depth warning on chains longer than threshold (configurable
  via env var or default 5)
- [ ] All new checks are `Level::Warning` (not Error), except orphan
  detection which is `Level::Error`
- [ ] `planr lint` with an empty backlog has no new failures
- [ ] All existing lint tests pass unchanged

## Notes

- 2026-08-08 created
- Depends on [[graph-types]] for the graph and [[transitive-queries]] for
  depth computation
- Modifies `src/lint.rs` to add a fourth pass (graph checks) and/or
  integrates graph construction into the lint pipeline
- New checks are warnings — they inform rather than block
