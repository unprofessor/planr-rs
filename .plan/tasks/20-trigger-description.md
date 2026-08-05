---
id: trigger-description
aliases: [trigger-description]
kind: task
parent: doc-improvements
title: Optimize SKILL.md frontmatter for triggering
status: done
assignee: null
created: 2026-07-30
updated: 2026-07-30
tags: []
depends_on: []
---

## Goal

Rewrite the `description` field in SKILL.md frontmatter to improve triggering accuracy: make it more “pushy” with specific user phrases and contexts per skill-creator best practices, while keeping it under ~150 words.

## Context

Parent story: [[doc-improvements]] under [[supplementary-tooling]]. The current description is functional but can be optimized using the skill-creator’s guidance: descriptions should list specific trigger phrases and contexts so the agent reliably consults the skill when relevant. The skill-creator has a description optimization loop (`scripts.run_loop`) that can measure trigger rates, but this task is the manual first pass.

Current description (87 words):
> Trunk-based planning and backlog management for multi-agent development. A leader maintains epics, stories, and tasks as one file per ticket on trunk; workers implement one task each in a dedicated git worktree branch; a reviewer independently verifies completion before merge. Tickets form a dependency graph (any ticket can gate any other) and may cross-reference each other with Obsidian-compatible [[wiki-links]]. Use when planning work, maintaining a backlog, splitting work into tickets, coordinating parallel agents, reviewing a completed task, picking up a task to implement, or linting the backlog for dangling references and dependency cycles.

## Acceptance

- [ ] Description includes specific trigger phrases the agent should watch for (“we need to plan this”, “split into tasks”, “what’s the dependency order”, “who’s reviewing”, “setup worktrees”, etc.)
- [ ] Description is more directive / “pushy” about when to use the skill (avoiding undertriggering)
- [ ] Description stays under ~150 words
- [ ] No factual changes to the skill’s capabilities or data model
- [ ] The rest of SKILL.md (body) is unchanged by this task

## Validation

All acceptance criteria checked against the updated `skills/planr/SKILL.md`:

- [x] **Trigger phrases present**: "we need to plan this", "split into tasks", "what's the dependency order", "who's reviewing", "setup worktrees" all appear in the new description
- [x] **Directive / pushy tone**: Description opens with "Use this skill when the developer says..." — imperative, context-first framing
- [x] **Under 150 words**: New description is 102 words (verified via `wc -w`)
- [x] **No factual changes**: Same capabilities (leader/worker/reviewer roles, one-file-per-ticket, worktrees, dependency graph, wiki-links) — no new claims or data model changes
- [x] **Only description changed**: `git diff skills/planr/SKILL.md` shows exactly one-line change, only the `description:` frontmatter field; body unchanged

## Review

verdict: approved
reviewer: The Clanker
date: 2026-07-30

### Acceptance criteria verification

| Criterion | Status | Evidence |
|---|---|---|
| Trigger phrases present | ✅ Pass | All 5 required phrases confirmed via grep: "we need to plan this", "split into tasks", "what's the dependency order", "who's reviewing", "setup worktrees" |
| Directive / pushy tone | ✅ Pass | Description opens with "Use this skill when the developer says..." — imperative, context-first framing |
| Under ~150 words | ✅ Pass | `wc -w` reports 101 words (well under 150) |
| No factual changes to capabilities/data model | ✅ Pass | `git diff` shows only the one `description:` field changed; body (roles, scripts, workflows, ticket format, etc.) is completely unchanged |
| Rest of SKILL.md (body) unchanged | ✅ Pass | Only line 4 (the `description:` frontmatter field) differs between trunk and branch |

### Validation commands run

- `git diff main...HEAD -- skills/planr/SKILL.md` — confirmed only description field changed, body untouched
- `wc -w` on new description — 101 words
- `grep` for each required trigger phrase — all 5 found

## Notes

- 2026-07-30 created
- See skill-creator guidance: "make the skill descriptions a little bit 'pushy'" and "include both what the skill does AND specific contexts for when to use it"
- The description is the ONLY thing Claude sees before deciding to load the skill body, so it needs to be both specific and compelling
- After this manual pass, consider running the skill-creator's `scripts.run_loop` for data-driven optimization with trigger eval queries (future task candidate)
