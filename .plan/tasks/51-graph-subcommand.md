---
id: graph-subcommand
aliases: [graph-subcommand]
kind: task
parent: graph-cli
title: 'planr graph subcommand'
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [cli, subcommand, graph]
depends_on: [neighbor-queries, transitive-queries, path-finding, topological-sort, mermaid-output, dot-output, ascii-terminal, stats-metrics]
---

## Goal

Add a `planr graph` subcommand to the CLI that exposes graph queries and
visualization to the user. This is the primary integration point for all
graph functionality.

## Context

A new subcommand `planr graph` with its own sub-subcommands:

```
planr graph neighbors <slug> [--kind hierarchy|depends-on|wiki-link|all]
planr graph deps <slug>                    # transitive dependencies
planr graph dependents <slug>              # transitive dependents
planr graph path <from> <to>              # shortest path
planr graph ancestors <slug>              # hierarchy parent chain
planr graph descendants <slug>            # hierarchy children chain
planr graph tree [slug]                   # ASCII hierarchy tree (optional root)
planr graph topo [--filter slug...]       # topological order
planr graph critical-path [--filter slug...]
planr graph stats                         # graph-wide metrics
planr graph visualize [--format mermaid|dot|ascii] [--filter slug]
```

The subcommand reads the backlog (working tree or trunk via `--ref`), builds
the graph, and runs the requested query. Output follows planr conventions
(stdout for results, stderr for errors, exit 1 on failure).

Implemented as `src/graph_cmd.rs` with the CLI routing added to `main.rs`.
The graph construction is called once per invocation and reused across all
sub-subcommands.

## Acceptance

- [ ] `planr graph` appears in `--help` output with sub-subcommand
  descriptions
- [ ] All sub-subcommands work as described
- [ ] `--ref` flag reads from a git ref instead of working tree
- [ ] Honours `PLANR_DIR` and `PLANR_TRUNK` env vars
- [ ] `planr graph visualize --format dot | dot -Tsvg` produces valid SVG
- [ ] Error on non-existent slug prints "no such ticket: <slug>" to stderr
  and exits 1
- [ ] Error on cycle in topo prints cycle info to stderr and exits 1
- [ ] Empty backlog produces "no tickets found" message
- [ ] Integration tests: invoke each sub-subcommand against the real backlog

## Notes

- 2026-08-08 created
- Depends on all graph API modules (queries, traversal, visualization)
- New file: `src/graph_cmd.rs` — registered in `main.rs`
- The subcommand builds the graph once, then dispatches to the relevant
  query/visualization function
