---
id: milestone-record-format
aliases: [milestone-record-format]
kind: task
parent: milestone-schema-lifecycle
title: Define milestone.md format and parser
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [milestones, format, parser]
depends_on: []
---

## Goal

Add the Markdown-plus-frontmatter representation and parser for a milestone
at `.plan/milestones/<kebab-id>/milestone.md`.

## Context

A milestone is a release scope, not a ticket. Its document owns lifecycle
metadata and can contain goals, exit criteria, notes, and release information.
Directory placement, not ticket frontmatter, links epics to the milestone.
Milestone IDs are kebab-case, including normalized version names such as
`v2-0-release`.

## Acceptance

- [ ] A `Milestone` model captures id, title, status, dates, optional release
  metadata, path, and Markdown body.
- [ ] The parser accepts required frontmatter (`id`, `kind: milestone`,
  `title`, `status`) and preserves the body.
- [ ] The parser rejects malformed frontmatter, mismatched directory IDs,
  unsupported statuses, and non-kebab milestone IDs.
- [ ] A milestone template and fixture set are added.
- [ ] Milestone documents are never mistaken for epic/story/task files by the
  existing ticket parser or catalog contract.
- [ ] Unit tests cover valid, malformed, and body-rich milestone documents.

## Notes

- Keep the format Markdown-first; do not introduce a parallel canonical YAML
  manifest.
