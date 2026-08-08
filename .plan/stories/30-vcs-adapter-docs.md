---
id: vcs-adapter-docs
aliases: [vcs-adapter-docs]
kind: story
parent: vcs-adapter-boundary
title: Document provider capabilities and backend behavior
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [vcs, docs, architecture]
depends_on: [git-regression-suite, filesystem-capability-errors, migration-validation]
---

## Goal

Document the VCS boundary, Git-first behavior, no-VCS limitations, and the
path for adding another backend.

## Acceptance

- [ ] README and skill documentation describe provider capabilities rather than
  claiming all workflows are VCS-neutral.
- [ ] Git-specific behavior and no-VCS behavior are clearly distinguished.
- [ ] A backend author has enough guidance to implement another provider.
- [ ] Migration instructions explain the audit/review/apply sequence.

## Tasks

- [[vcs-backend-docs]]
