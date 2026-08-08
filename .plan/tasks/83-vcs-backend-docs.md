---
id: vcs-backend-docs
aliases: [vcs-backend-docs]
kind: task
parent: vcs-adapter-docs
title: Document VCS capabilities and the Git-first adapter
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [vcs, docs, architecture]
depends_on: [git-regression-suite, filesystem-capability-errors, migration-validation]
---

## Goal

Document the provider boundary, Git implementation, no-VCS behavior, and path
to adding another backend.

## Acceptance

- [ ] README and skill guidance describe source versus workflow capabilities.
- [ ] Git-specific behavior and filesystem-only behavior are clearly separated.
- [ ] A future jj/backend author has an interface and capability checklist.
- [ ] Migration instructions explain audit, review, dry-run, and apply steps.
- [ ] Documentation does not imply that planr owns VCS rename or commit
  semantics.
