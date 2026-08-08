---
id: milestone-docs
aliases: [milestone-docs]
kind: task
parent: milestone-verification-docs
title: Document milestone workflows and scope semantics
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [milestones, docs, skill]
depends_on: [milestone-e2e]
---

## Goal

Document the milestone directory layout, Markdown record format, lifecycle,
placement commands, board views, and the distinction between assignment and
archival.

## Acceptance

- [ ] README, skill guidance, and relevant references show the root/unplanned
  and milestone directory layouts.
- [ ] The one-active-milestone rule and completed-milestone archive projection
  are documented.
- [ ] Kebab-case milestone IDs such as `v2-0-release` are documented.
- [ ] Root tickets remain legal without a milestone assignment.
- [ ] Documentation states that filesystem moves do not invoke or record VCS
  rename/commit semantics.
