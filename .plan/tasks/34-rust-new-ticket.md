---
id: rust-new-ticket
aliases: [rust-new-ticket]
kind: task
parent: rust-write-commands
title: "Port new-ticket: embedded templates, exclusive-flock prefix allocation"
status: todo
assignee: null
created: 2026-08-05
updated: 2026-08-05
tags: [rust, new-ticket, flock, templates]
depends_on: [rust-lint, rust-git-lock]
---

## Goal

Port `src/new-ticket.ts` + `src/cli/new-ticket.ts` (~380 LOC TS): ticket
scaffolding with slug/kind/parent guards, template substitution, and
flock-serialized prefix allocation — with the templates compiled into the
binary.

## Context

Parent story: [[rust-write-commands]]. TS counterpart: [[port-new-ticket]].

Guards, in order, with exact messages:

- kind not in {epic, story, task} → `unknown kind: <kind> (want
  epic|story|task)`
- slug must match `^[a-z0-9]+(-[a-z0-9]+)*$` → `bad slug '<slug>': want
  kebab-case (lowercase alphanumerics, single hyphens between segments,
  starting with [a-z0-9])`
- story/task without parent → `parent slug required for <kind>`
- parent must exist: scan all three kind dirs for a file matching
  `^\d+-<regex-escaped-parent>\.md$` → `parent '<parent>' not found under
  <planDir>/ — create the parent first`

Templates: vendor copies of `skills/planr/templates/{epic,story,task}.md`
into `templates/` here and embed with `include_str!`. The TS resolves the
template path relative to its own script location (`process.argv[1]`), which
an installed single binary cannot do — embedding keeps one self-contained
artifact and removes a whole failure mode. Substitute `__SLUG__`,
`__TITLE__`, `__PARENT__` (empty string for epics), `__DATE__` globally.
`__DATE__` is **UTC** `YYYY-MM-DD` (TS uses `toISOString()`) — note that
claim/merge-task use the *local* date; this inconsistency is preserved
deliberately for parity.

Exclusive-flock critical section (replaces the TS `LOCKED_WRITE_SCRIPT`
child; the lock is held in-process per [[rust-git-lock]]):

1. Highest existing `^(\d+)-` prefix in the kind dir + 1, zero-padded to 2.
2. Refuse `already exists: <path>` (TS child exit 2) if the target file
   exists.
3. Write `<planDir>/<kind>s/<NN>-<slug>.md`.
4. Post-write re-scan: exactly one `<NN>-*` file must exist, else
   `internal error: prefix <NN> is shared by <count> files in <dir> after
   creating <path>` (TS child exit 3).

Output contract: stdout is EXACTLY one line — the created path as a relative
path (e.g. `.plan/tasks/03-x.md`). After creation, run the lint engine
in-process over the working tree and echo findings to stderr (informational
only, never fails; TS spawns the `lint.cjs` subprocess — in Rust this is a
function call). Usage error (missing args) → usage line to stderr, exit 1.

## Acceptance

- [ ] Ported `new-ticket.test.ts` cases green (all four guard classes,
  template substitution incl. `aliases: [<slug>]`, prefix allocation)
- [ ] Three concurrent `planr new-ticket` invocations produce three distinct
  sequential prefixes (the flock serializes allocation) and one-line stdouts
- [ ] Pre-existing lint errors surface on stderr while stdout stays a single
  path line, exit 0
- [ ] `cargo test` green

## Notes

- 2026-08-05 created
