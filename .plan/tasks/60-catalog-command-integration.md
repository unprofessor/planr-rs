---
id: catalog-command-integration
aliases: [catalog-command-integration]
kind: task
parent: workspace-ticket-catalog
title: Use the catalog for ticket lookup and creation
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [catalog, cli, commands]
depends_on: [ticket-catalog, catalog-scope-validation]
---

## Goal

Make ticket lookup and creation understand both unplanned and milestone
locations without adding milestone fields to ticket frontmatter.

## Acceptance

- [ ] New epic creation can target an explicit planned or active milestone;
  omission continues to create an unplanned epic.
- [ ] New stories and tasks inherit the location of their parent.
- [ ] Claim, review, and close lookup can find tickets by slug in a nested
  scope while retaining existing workflow semantics.
- [ ] Unplanned root work remains legal; no implicit milestone gate is added.
- [ ] Path moves and VCS operations remain separate concerns.
