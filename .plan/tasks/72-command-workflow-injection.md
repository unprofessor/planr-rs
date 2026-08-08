---
id: command-workflow-injection
aliases: [command-workflow-injection]
kind: task
parent: vcs-command-integration
title: Route workflow commands through VCS capabilities
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [vcs, commands, workflow]
depends_on: [vcs-workflow-contract, command-source-injection]
---

## Goal

Replace direct Git workflow calls in new, claim, review, and close with
provider capabilities while keeping filesystem-only milestone commands
independent.

## Acceptance

- [ ] Workflow commands request capabilities through an injected provider.
- [ ] No workflow command directly shells out to Git outside the adapter.
- [ ] Unsupported capabilities produce actionable errors.
- [ ] Existing review approval and close gates remain enforced.
- [ ] Filesystem-only milestone creation, placement, and lifecycle do not
  require workflow capabilities.
