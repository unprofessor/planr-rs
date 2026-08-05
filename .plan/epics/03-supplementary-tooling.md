---
id: supplementary-tooling
aliases: [supplementary-tooling, bash-era-polish]
kind: epic
title: Supplementary tooling and documentation improvements
status: todo
assignee: null
created: 2026-07-30
updated: 2026-07-30
tags: []
---

## Goal

High-value improvements that sit alongside the TypeScript port: board summary
stats, new TS utilities built on the port infrastructure, and documentation
polish. Some tasks touch the bash scripts directly (board.sh summary); others
are new commands implemented on the TS CLI layer after the scaffolding is in
place; documentation is always independent.

## Scope

- **Board improvements**: summary stats (count per status) in board.sh
- **New TS utilities**: a roll-up progress command, a backlinks discoverer, and
  a pre-commit hook installer — all built on the TS CLI layer after the port
  scaffolding is ready
- **Documentation**: optimize SKILL.md frontmatter for triggering, add a
  troubleshooting section

## Out of scope

- Changes to the ticket format, process, or role model (those are stable)
- The TypeScript port itself (tracked in [[port-scripts-to-typescript]])
- Retro-hardening features (verify hook, resume, waiver — tracked in
  [[hotcell-firewall-hardening]])

## Notes

- 2026-07-30 created; 2026-07-30 retitled from "bash-era-polish" to reflect the
  mixed bash + TS nature of the work.
- Board-summary-stats edits board.sh directly (bash); roll-up, backlinks, and
  the pre-commit hook depend on the port infrastructure and are implemented as
  TS commands.
