---
id: utility-scripts
aliases: [utility-scripts]
kind: story
parent: supplementary-tooling
title: New TS utility scripts
status: todo
assignee: null
created: 2026-07-30
updated: 2026-07-30
tags: []
depends_on: []
---

## Goal

Add two new utility scripts: `backlinks.sh` for discovering which tickets wiki-link to a given slug, and a `pre-commit` hook template that runs `lint.sh` on staged `.plan/` changes to catch dangling refs before they reach trunk.

## Context

Parent epic: [[supplementary-tooling]]. The SKILL.md documents `grep -rn '\[\[slug\]\' .plan/` as the backlinks discovery method — a dedicated TS command is one invocation with proper .plan/ path resolution and frontmatter filtering. The pre-commit hook is noted as out-of-scope in [[port-scripts-to-typescript]] but is a valuable guardrail.

Both are implemented on the TS CLI layer after the port scaffolding is in place.

## Notes

- 2026-07-30 created
- Tasks: `backlinks-script` (backlinks.sh), `precommit-hook` (pre-commit hook template)
