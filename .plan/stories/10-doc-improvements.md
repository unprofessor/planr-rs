---
id: doc-improvements
aliases: [doc-improvements]
kind: story
parent: supplementary-tooling
title: Documentation polish for SKILL.md and references
status: todo
assignee: null
created: 2026-07-30
updated: 2026-07-30
tags: []
depends_on: []
---

## Goal

Improve the planr skill documentation: optimize the SKILL.md frontmatter description for better agent triggering, and add a troubleshooting section covering common issues (merge conflicts, stale worktrees, interrupted workers, cross-platform sed).

## Context

Parent epic: [[supplementary-tooling]]. The existing documentation is thorough but the frontmatter description can be improved per skill-creator guidance (more "pushy", listing specific trigger phrases). There is also no troubleshooting section — common failure modes like merge conflicts, stale worktrees, and worker interruption are handled in scripts but not documented in one place for the leader to reference.

## Notes

- 2026-07-30 created
- Tasks: `trigger-description` (optimize frontmatter), `troubleshooting-guide` (add troubleshooting section)
