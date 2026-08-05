---
id: board-improvements
aliases: [board-improvements]
kind: story
parent: supplementary-tooling
title: Board summary stats and roll-up progress
status: todo
assignee: null
created: 2026-07-30
updated: 2026-07-30
tags: []
depends_on: []
---

## Goal

Enhance the board.sh output with summary statistics (ticket counts per status) and add a roll-up progress command so the leader can see story/epic completion at a glance without manual counting.

## Context

Parent epic: [[supplementary-tooling]]. The current board.sh lists all tickets but gives no aggregate view — a leader running a project with 40+ tickets has to mentally tally progress. Stories and epics have no %-complete indicator.

Board-summary-stats edits board.sh directly (bash). Roll-up-progress is implemented as a new TS command on the CLI layer once the port scaffolding is ready.

## Notes

- 2026-07-30 created
- Tasks: `board-summary-stats` (summary counts), `roll-up-progress` (progress computation)
