---
id: graph-cli
aliases: [graph-cli]
kind: story
parent: ticket-graph
title: CLI integration and graph-aware enhancements
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [graph, cli, lint, board]
depends_on: [graph-query-traverse, graph-visualize]
---

## Goal

Integrate the graph module into the planr CLI as a first-class `planr graph`
subcommand, and enhance existing subcommands (board, lint) with graph-aware
information.

## Context

With the graph data model, query API, and visualizers in place, this story
wires everything into the CLI and enriches existing workflows:

**`planr graph` subcommand** — a new top-level command with sub-subcommands:

```
planr graph neighbors <slug>           # show direct neighbors
planr graph deps <slug>                # transitive dependencies
planr graph dependents <slug>          # transitive dependents
planr graph path <a> <b>               # shortest path between two tickets
planr graph visualize [--format mermaid|dot|ascii] [--filter slug]
planr graph stats                      # graph-wide metrics
planr graph topo                       # topological order (depends_on DAG)
```

**Board enhancements** — the board output gains optional graph annotations:

- `planr board --graph` appends a small ASCII dependency summary showing
  longest chain and blocked-by clusters.
- Story/epic rows show completion percentage derived from graph traversal
  (not just direct children count).

**Lint enhancements** — additional graph-aware checks:

- Orphan detection: tickets whose `parent` field points to a slug that
  doesn't appear in the graph (already exists in lint.rs pass 2, but graph
  module makes it a reusable query).
- Cluster-based warnings: warn when closely related tickets (same story,
  heavy wiki-link cross-references) have very different priority/status
  without explicit depends_on edges.

## Acceptance

- [ ] [[graph-subcommand]] done: `planr graph <query|visualize|stats>`
  subcommand with all sub-subcommands working against working tree or ref
- [ ] [[board-graph-enhancements]] done: `planr board --graph` shows
  additional graph-derived summary sections
- [ ] [[lint-graph-enhancements]] done: graph-aware lint checks produce
  new warning categories
- [ ] All existing `planr board` and `planr lint` tests pass unchanged
  (graph features are opt-in)
- [ ] `planr graph` honours `PLANR_DIR` and `PLANR_TRUNK` env vars
- [ ] `planr graph visualize` piped to a file works for Mermaid/DOT
- [ ] Error messages follow existing planr conventions (stderr, exit 1)
- [ ] Unit + integration tests for each sub-subcommand

## Notes

- 2026-08-08 created
- Depends on both [[graph-query-traverse]] and [[graph-visualize]]
- The `graph` subcommand is a new module, not a refactor of existing ones
- Existing `backlinks-script` task under [[utility-scripts]] is superseded:
  `planr graph neighbors <slug> --kind wiki-link` replaces it
