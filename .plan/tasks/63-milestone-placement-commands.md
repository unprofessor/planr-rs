---
id: milestone-placement-commands
aliases: [milestone-placement-commands]
kind: task
parent: milestone-placement
title: Add milestone assignment and reassignment commands
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [milestones, cli, moves]
depends_on: [milestone-placement-safety, milestone-state-rules]
---

## Goal

Expose explicit commands for assigning an epic hierarchy to a milestone or
returning it to the unplanned backlog.

## Context

Use milestone-oriented language such as `planr milestone add`, `move`, or an
unassignment form rather than calling the operation archival. Targets may be
planned or active, but not completed. The command reports filesystem changes;
the user handles VCS recording.

## Acceptance

- [ ] A user can assign an unplanned epic to a planned/active milestone.
- [ ] A user can reassign an epic between non-completed milestones.
- [ ] A user can return an epic hierarchy to the unplanned root.
- [ ] The command refuses missing parents, completed targets, split moves, and
  active/review descendants unless explicitly supported by a later policy.
- [ ] Help text states that the operation does not perform VCS moves or commits.
