---
id: rust-lint
aliases: [rust-lint]
kind: task
parent: rust-read-commands
title: Port lint engine + CLI
status: review
assignee: null
created: 2026-08-05
updated: 2026-08-05
tags: [rust, lint]
depends_on: [rust-parse-core, rust-git-lock]
---

## Goal

Port `src/lint.ts` + `src/cli/lint.ts` (~310 LOC TS): the pure three-pass
backlog checker and its CLI (working-tree and ref modes), with every message
string and exit code byte-identical.

## Context

Parent story: [[rust-read-commands]]. TS counterpart: [[port-lint]]. The
engine is pure (input = `[(file, ParsedTicket)]`); the CLI gathers inputs —
working-tree mode scans `<planDir>/{epics,stories,tasks}` with sorted
readdir, skipping unreadable files; ref mode uses `ls_tree_md`/`show_ref`.
Lint takes no lock in TS.

Pass structure (port exactly):

1. **Per-file**: dir-kind from the path containing `/epics/` `/stories/`
   `/tasks/`; filename slug = basename minus `.md` minus `^\d+-` prefix;
   `missing id in frontmatter`; `id '<id>' does not match filename slug
   '<slug>'`; `kind '<kind>' but the file lives in the <dirkind>s directory`;
   `invalid status '<status>' (want todo|in_progress|review|done|blocked)`;
   `duplicate slug '<slug>' (also <file>) — slugs are identity and must be
   unique across the backlog` (duplicate is NOT indexed; subsequent checks
   skip it).
2. **Cross-ref** (sorted by id): epic with parent → `epics must not have a
   parent (found '<parent>')`; story/task without parent → `a <kind> must
   name a parent slug`; missing parent → `parent '<parent>' does not exist —
   roll-up is derived by scanning children, so this <kind> would be
   orphaned`; wrong-kind parent → warning `parent '<parent>' is a <kind> (a
   <kind>'s parent is usually a <expected>)` (story→epic, task→story);
   self-dep → `depends_on itself`; missing dep → `depends_on '<dep>' does
   not exist — claim.sh could never be satisfied`; unresolved wiki-link → a
   warning (message text: `matches no ticket slug (fine if it points at a
   non-ticket note)`, prefixed with the bracketed link target).
3. **Cycle detection**: white/gray/black DFS with an explicit stack; a gray
   hit builds the cycle path `a -> b -> a` → `depends_on cycle: <path> —
   nothing in the cycle can ever be claimed`. Self-deps are SKIPPED here
   (already reported in pass 2) so a self-dep is never double-reported as a
   one-node cycle.

Output contract: one `<level>: <file>: <message>` line per issue in
discovery order; when there are any issues append `lint: N error(s), M
warning(s)`; exit 1 iff errors > 0, else 0; zero inputs → exit 0, silent.

## Acceptance

- [ ] `lint.rs` engine is pure (no fs/git) with the three passes above
- [ ] CLI supports both modes; ref mode reads via the git wrappers
- [ ] Ported `lint.test.ts` cases green; every message string asserted
  byte-identical
- [ ] e2e spot-checks: dangling dep/parent, cycle, self-dep (single report),
  duplicate slug, invalid status, kind/dir mismatch, warning-only classes
  exit 0, empty backlog silent exit 0
- [ ] `cargo test` green

## Validation

All acceptance criteria verified in worktree at
`/home/exfed/projects/wt-rust-lint`:

1. **Lint engine** (`check_backlog`) — three passes implemented:
   - Pass 1: dir-kind from path, filename slug extraction (NN− prefix),
     5 check classes (missing id, id/slug match, kind/dir match, valid
     status, duplicate slug)
   - Pass 2: cross-ref checks — parent existence, parent kind warning,
     depends_on existence (self→"depends_on itself", missing→"claim.sh
     could never be satisfied"), unresolved wiki-link warning
   - Pass 3: cycle DFS with explicit stack; self-deps skipped to avoid
     double-report
2. **CLI** — working-tree mode (sorted readdir) and ref mode (git wrappers);
   output matches TS format exactly.
3. **Smoke test** — `planr lint` on this repo's `.plan/` produces 0 errors,
   13 warnings (matching the TS lint output for the same backlog).
4. **Tests** — 50 total (9 lint-specific + 33 parse/ticket + 8 git/lock),
   all green.
5. **cargo build** — clean (expected dead-code warnings).

All acceptance boxes checked.

## Notes

- 2026-08-05 created. The `depends_on` message text mentions `claim.sh` by
  name — keep it byte-identical for now; the string audit in [[rust-release]]
  rewrites `scripts/` references after parity is proven.
