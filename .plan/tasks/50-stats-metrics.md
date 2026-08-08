---
id: stats-metrics
aliases: [stats-metrics]
kind: task
parent: graph-visualize
title: Graph metrics and statistics
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [graph, metrics, statistics]
depends_on: [graph-types, topological-sort]
---

## Goal

Compute and render summary statistics about the graph structure: node/edge
counts, degree distribution, chain lengths, component analysis, and
cycle detection.

## Context

Metrics give users a high-level understanding of the backlog's structural
complexity:

```rust
pub struct GraphMetrics {
    pub node_count: usize,
    pub edge_counts: EdgeCounts,
    pub degree_stats: DegreeStats,
    pub longest_chain: usize,           // max depth in depends_on DAG
    pub connected_components: usize,    // weakly connected (all edge types)
    pub cycle_count: usize,             // cycles in depends_on subgraph
    pub hierarchy_depth: usize,         // max depth of hierarchy tree
    pub orphan_count: usize,            // tickets with missing parent slug
}

pub struct EdgeCounts {
    pub hierarchy: usize,
    pub depends_on: usize,
    pub wiki_links: usize,
    pub self_loops: usize,
}

pub struct DegreeStats {
    pub min_out: usize,
    pub max_out: usize,
    pub avg_out: f64,
    pub min_in: usize,
    pub max_in: usize,
    pub avg_in: f64,
}
```

```rust
pub fn compute_metrics(graph: &TicketGraph) -> GraphMetrics;
pub fn render_metrics(metrics: &GraphMetrics) -> String;
```

## Acceptance

- [ ] `compute_metrics()` returns all fields of `GraphMetrics`
- [ ] Edge counts are accurate per kind
- [ ] Degree stats compute min/max/avg for both in-degree and out-degree
- [ ] `longest_chain` uses topological sort depth
- [ ] `connected_components` counts weakly connected components (undirected
  view across all edge types)
- [ ] `cycle_count` counts distinct cycles in depends_on subgraph
- [ ] `hierarchy_depth` is max depth from any root epic to deepest task
- [ ] `orphan_count` counts tickets with non-existent parent slug
- [ ] `render_metrics()` produces human-readable output with labels and
  values aligned
- [ ] Unit tests: empty graph, single node, linear chain, two disconnected
  components, full backlog

## Notes

- 2026-08-08 created
- Depends on [[graph-types]] and [[topological-sort]] (for longest chain)
- Connected components uses a simple DFS over the undirected view
- Orphan count reuses graph's referential integrity (parents not in ticket
  set) rather than re-parsing
