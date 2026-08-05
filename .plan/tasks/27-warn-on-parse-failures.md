---
id: warn-on-parse-failures
aliases: [warn-on-parse-failures]
kind: task
parent: port-scripts
title: Warn loudly on unparseable tickets in board/lint instead of silent drop
status: todo
assignee: null
created: 2026-08-01
updated: 2026-08-01
tags: [parser, board, lint, silent-failure]
depends_on: []
---

## Goal

Close the last silent-failure class in the TS port: unparseable ticket files
are currently dropped silently by `src/cli/board.ts` and the lint path
(`catch { continue; }`), so a malformed ticket vanishes from the board and
its slug disappears from the dependency graph without any diagnostic. Make
every consumer report a loud, actionable warning (stderr + exit code where
appropriate) when a ticket file fails to parse.

## Context

Found during epic closeout. Three tickets had `title:` values containing
colon+space (e.g. `title: Reviewer guidance: run flaky/network tests…`),
which is invalid YAML — `eemeli/yaml` throws `Nested mappings are not allowed
in compact mappings`. The old awk parser was lenient (read the raw line);
the real YAML parser is strict (correctly). Effects observed before the data
fix:

- Board silently showed 38/41 tickets — the 3 unparseable ones just vanished.
- `lint.sh` reported a FALSE error: `depends_on 'reviewer-flakiness-guidance'
  does not exist` — because that task's file failed to parse, so lint couldn't
  find its slug when checking `12-done-with-waiver`'s dependency.
- No message anywhere explained why.

This is precisely the "silent-parsing-failure" class the epic
[[port-scripts-to-typescript]] set out to eliminate, but the port's own
`catch { continue; }` introduced a new instance. The data was fixed in commit
9a6301f (quote the 3 titles); this task hardens the code so the next
malformed ticket is impossible to miss.

## Acceptance

- [ ] `src/cli/board.ts`: parsing a ticket from the working tree or a ref
  that throws yields a `warning: <file>: unparseable (<reason>)` line on
  stderr (matching lint's `warning:` prefix convention), and the file is
  still excluded from the board (it can't be rendered) — but the board's
  exit code becomes non-zero so the caller notices.
- [ ] `src/cli/lint.ts` (and/or the shared read path): an unparseable ticket
  yields a `warning:` (or `error:`? decide) line naming the file and the
  parser's reason. Recommendation: warning, because the data fix pattern is
  "quote the value" and the graph checks still run on the rest — but if a
  ticket's slug is depended on by another ticket, escalate to `error:` so
  gating can't silently break. Document the choice in the ticket.
- [ ] The shared read path is extracted if both CLIs duplicate it (a small
  `readTickets()` helper in `src/` used by both) — no copy-paste.
- [ ] Tests: add a fixture with a `title: value: with colon` (unquoted,
  invalid YAML) and assert (a) board prints the warning + non-zero exit,
  (b) lint prints the warning, (c) a ticket whose slug is a dependency of
  another still resolves (the dep's slug is found even when the dep's own
  file is unparseable? or the error escalates — whatever the design decision
  says).
- [ ] `npm test` green; `run-tests.sh` green; `npm run build` works.

## Notes

- 2026-08-01 created. Found during epic closeout; data fixed in 9a6301f.
  Depends on nothing (can be claimed immediately). Root cause pattern:
  unquoted YAML scalars containing `:` — worth a line in TICKET-FORMAT.md
  ("quote title values containing colons").
