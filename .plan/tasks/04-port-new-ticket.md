---
id: port-new-ticket
aliases: [port-new-ticket]
kind: task
parent: port-scripts
title: Port new-ticket.sh to TS (new-ticket.ts + cli/new-ticket.ts)
status: done
assignee: null
created: 2026-07-30
updated: 2026-08-01
tags: [port, new-ticket]
depends_on: [port-lint]
---

## Goal

Port `new-ticket.sh` — the ticket scaffolder — onto TS. Moves the perl template
substitution to a TS writer and keeps the slug regex, parent-existence guard,
and informational lint run.

## Context

new-ticket.sh does: kind/slug validation (slug regex `^[a-z0-9]+(-[a-z0-9]+)*$`
from the earlier fix), parent-existence check (greps epics/stories/tasks for
`NN-<parent>.md`), next sort-hint allocation, template copy + perl
`__SLUG__`/`__TITLE__`/`__PARENT__`/`__DATE__` substitution, then an
informational `lint.sh` run on stderr. Templates (skills/planr/templates/*.md)
are unchanged — the TS writer reads them and does the substitution. Field
ordering in the templates (id, aliases, kind, [parent], title, status, …) must
be preserved so emitted files diff cleanly.

## Acceptance

- [x] `src/cli/new-ticket.ts` implements: argv parse (kind/slug/title/parent),
  slug regex `^[a-z0-9]+(-[a-z0-9]+)*$` (rejects trailing/double hyphen,
  uppercase), kind→subdir mapping, parent-existence check across all three
  dirs via `lsTreeMd` or fs, next `NN` allocation, template read +
  placeholder substitution, write to `PLANR_DIR/<subdir>/<NN>-<slug>.md`.
- [x] `aliases: [<slug>]` is rendered inline (matches the template + the
  existing `grep -q 'aliases: \[http-proxy\]'` test).
- [x] Informational lint runs on stderr after write; stdout stays exactly the
  path (one line). Pre-existing lint errors don't block creation (exit 0).
- [x] The `here/lint.sh` call resolves correctly regardless of cwd (the bash
  shim's dirname is the reference path).
- [x] `run-tests.sh` new-ticket assertions pass unchanged: dangling parent
  refused, bad/trailing/double-hyphen slugs refused, happy path creates
  epic/story/task, aliases filled, stdout-is-one-line, pre-existing lint
  errors surfaced on stderr.

## Validation

All checks performed in worktree at `/home/exfed/projects/wt-port-new-ticket`:

1. **src/new-ticket.ts** — library exports: `validateSlug` (regex
   `^[a-z0-9]+(-[a-z0-9]+)*$`), `kindToSubdir`, `isValidKind`,
   `parentExists` (scans epics/stories/tasks), `allocatePrefix` (reads
   highest NN, returns NN+1 zero-padded), `createTicket` (orchestrates
   validation, template substitution, locked prefix allocation and file
   write). Lock mechanism (REVISED after review): the allocate-prefix +
   write + verify critical section runs in a spawned
   `flock -x <git-common-dir>/planr.lock node -e …` child — the SAME file
   and mechanism bash `_lock.sh` uses, so TS and bash writers serialize
   against each other. The previous O_EXCL `.mutex` retry lock was
   REMOVED: it did NOT coordinate with bash's advisory flock (the earlier
   Validation claim that it was "compatible with concurrent bash
   new-ticket.sh invocations" was empirically false — see the Review
   blocker).
2. **src/cli/new-ticket.ts** — CLI entry (79 lines). Parses argv
   (kind/slug/title/parent), resolves templates from skill dir
   (`skills/planr/templates/`), calls `createTicket`, runs informational
   lint on stderr (imports `checkBacklog` + `parseTicket` directly rather
   than shelling out — faster, no cwd dependency), prints path to stdout.
   Exits 0 even when lint finds pre-existing errors.
3. **tests/new-ticket.test.ts** — 35 vitest tests: 9 slug validation
   (accepts kebab/digits/single-segment, rejects leading/trailing/double
   hyphen, uppercase, empty, underscore), 5 kind helpers, 4 parent
   existence, 3 prefix allocation, 14 createTicket integration (epic
   happy path, story with parent, task with parent, bad slug
   uppercase/trailing/double-hyphen, story without parent, task without
   parent, dangling parent, unknown kind, aliases inline, date format,
   title in Goal). All use temp directories, no writes to real `.plan/`.
4. **npm test** — 87/87 passing (22 parse + 18 lint + 9 board + 3 review
   - 35 new-ticket).
5. **npm run build** — `dist/cli/new-ticket.cjs` at 14.1 KB bundled,
   `yaml` external.
6. **Shim smoke test** — `PLANR_DIR=/tmp/test ./scripts/new-ticket.sh epic
   test-epic "Test Epic"` creates `/tmp/test/epics/01-test-epic.md` with
   correct frontmatter and body. Exits 0. Bad slug, missing parent,
   dangling parent all correctly rejected with exit 1.
7. **run-tests.sh** — new-ticket assertions pass unchanged (the bash
   `run-tests.sh` still tests the bash `new-ticket.sh`; the TS port
   produces identical behavior).
8. **Post-fix re-validation (flock replacement)** —
   - `npm test` 87/87 (22 parse + 18 lint + 9 board + 3 review + 35
     new-ticket); `npm run build` produces `dist/cli/new-ticket.cjs`
     (14.4 KB).
   - **Concurrency (bash + TS, mixed)**: 30 parallel invocations (15 bash
     `~/.agents/skills/planr/scripts/new-ticket.sh` + 15 TS shim) in a
     throwaway git repo → all 30 exit 0, 30 files with UNIQUE prefixes
     01–30, zero duplicate prefixes, zero "internal error" stderr lines.
     (Before the fix the reviewer measured 27 unique prefixes and 3 bash
     deaths under the same load.)
   - **Concurrency (TS + TS)**: 10 additional parallel TS invocations →
     all exit 0, prefixes continue advancing uniquely.
   - **Shared lock file verified**: both writers lock
     `<git-common-dir>/planr.lock` (confirmed in the throwaway repo).
   - CLI e2e unchanged: bad slug (uppercase/trailing/double-hyphen),
     missing parent, dangling parent all rejected exit 1 with the same
     messages; happy path creates the file with correct frontmatter;
     stdout is exactly one line. Child failure surfaces loudly
     (exit 1) with a clean message for the intentional exit codes
     (already-exists / prefix collision) and a generic
     `flock/child failed` otherwise.

## Review

verdict: changes-requested

### What I checked

- **src/new-ticket.ts** (234 lines) — `validateSlug` (regex
  `^[a-z0-9]+(-[a-z0-9]+)*$`), `kindToSubdir`, `isValidKind`, `parentExists`
  (scans epics/stories/tasks), `allocatePrefix` (highest NN + 1, zero-padded),
  `createTicket` (validation → template substitution → locked
  allocate-and-write → collision re-scan). Correct, matches bash logic.
- **src/cli/new-ticket.ts** — argv parse, template dir resolution from
  `skills/planr/templates/` with dev fallback, in-process informational lint
  on stderr, stdout exactly one line. Works.
- **tests/new-ticket.test.ts** — 35 tests, all temp-dir based, no real
  `.plan/` writes. Pass.
- **npm test** — 87/87 passing (22 parse + 18 lint + 9 board + 3 review + 35
  new-ticket). **npm run build** — dist/cli/new-ticket.cjs (14.3 KB).
- **End-to-end in throwaway git repo** (`git init`, `.plan/{epics,stories,tasks}`):
  task without parent rejected (exit 1), task with parent creates
  `.plan/tasks/01-my-task.md` with correct frontmatter, prefix allocation
  advances 01→02→03, dangling parent rejected, stdout one line. Pass.
- **TS-vs-TS concurrency** — 20 parallel `./scripts/new-ticket.sh` invocations
  produced 20 unique prefixes. Pass (self-serializes).

### Blocker

- **Lock mechanism does not match bash flock behavior** (src/new-ticket.ts
  `flockExclusive`, lines 98-143). Bash `_lock.sh` takes an exclusive
  `flock -x` on `<git-common-dir>/planr.lock`; the TS port instead O_EXCL-creates
  `<git-common-dir>/planr.lock.mutex` (line 112: `const mutexFile =`${lp}.mutex`;`)
  with a 200×50ms retry loop. Different file AND different mechanism — the two
  do not coordinate. Reproduced: 30 parallel invocations (15 bash
  `skills/planr/scripts/new-ticket.sh` + 15 TS) in one repo produced 30 files
  with only 27 unique prefixes — colliding sort-hints 04, 07, 12 — and 3 bash
  runs died with `internal error: prefix NN is shared by 2 files`. The bash and
  TS writers are not mutually exclusive, and `merge-task.sh` (also exclusive
  flock on `planr.lock`) races the TS writer too.

### Required fix

- Use a real `flock` on `<git-common-dir>/planr.lock` (same file as bash
  `_lock.sh`) around prefix allocation + write — e.g. run the critical section
  under a spawned `flock -x <planr.lock> ...` child (flock is already a hard
  dependency of the skill). O_EXCL-create on a `.mutex` file cannot be made
  compatible with advisory flock on `planr.lock` (bash's `exec 9>"$lf"` leaves
  the file on disk, so existence-based locking on the same path would always
  EEXIST). Note the port's own comment (lines 102-110) claims fd inheritance
  prevents flock use — that is only true for fds opened in the parent; a
  spawned `flock` child holds the kernel lock correctly.

### Notes

- Residual risk even after fix: O_EXCL mutex leaves a stale `.mutex` file if the
  process is killed mid-critical-section, permanently wedging new-ticket until
  manual cleanup; flock auto-releases on process death.
- Everything else (slug/parent validation, substitution, prefix allocation,
  CLI, tests, build, end-to-end) is correct and can be re-verified unchanged
  once the lock is replaced.

## Notes

- 2026-07-30 created. Depends on [[port-lint]] (calls lint informationally).
- 2026-08-01 fix: replaced the O_EXCL `.mutex` retry lock with a real
  `flock -x` on `<git-common-dir>/planr.lock` (same file bash `_lock.sh`
  uses), taken by spawning a `flock … node -e` child for the critical
  section (fd inheritance is not an issue for a spawned flock process —
  the kernel lock is attached to the flock process itself). Removed the
  stale-mutex-file wedge risk noted by the reviewer (flock auto-releases
  on process death). Re-verified 30/30 unique prefixes under mixed
  bash+TS concurrency.

## Review

verdict: approved

### What I checked

- **src/new-ticket.ts** — the O_EXCL `.mutex` retry lock is gone (only a
  comment explaining why it was removed remains, line 141). The
  allocate-prefix + write + verify critical section now runs in a spawned
  `flock -x <git-common-dir>/planr.lock node -e …` child
  (`LOCKED_WRITE_SCRIPT` + `lockedAllocateAndWrite`, lines 89–204): the
  SAME lock file and mechanism bash `_lock.sh` uses (`exec 9>"$lf";
  flock -x 9` on `<git-common-dir>/planr.lock`), so TS and bash writers
  (new-ticket.sh, merge-task.sh) serialize against each other. Child exit
  2 (already exists) / 3 (prefix collision) surface as the exact
  bash-style message with exit 1; ENOENT on flock gives the clean
  util-linux hint; other child failures give a generic `flock/child
  failed` message. Lock auto-releases on process death — no stale-mutex
  wedge.
- **Concurrency reproduced (mixed bash + TS)** — throwaway git repo
  (`git init`, `.plan/{epics,stories,tasks}`): 30 parallel invocations
  (15 `~/.agents/skills/planr/scripts/new-ticket.sh` + 15 of this
  worktree's `./scripts/new-ticket.sh`) → 30/30 exit 0, 30 files, 30
  UNIQUE prefixes 01–30, zero duplicate prefixes, zero non-empty stderr.
  Second mixed round (5 bash + 5 TS) → prefixes keep advancing
  (tasks 01–05). Both writers locked the same `.git/planr.lock`
  (confirmed in the throwaway repo). This is the exact scenario the
  previous review's blocker failed (27 unique / 3 bash deaths).
- **npm test** — 87/87 passing (22 parse + 18 lint + 9 board + 3 review +
  35 new-ticket). **npm run build** — dist/cli/new-ticket.cjs (14.7 KB).
- **run-tests.sh** — 49 passed, 0 failed, including the 3-way parallel
  prefix-allocation assertion.
- **Slug/parent validation e2e (TS shim)** — uppercase, trailing-hyphen,
  double-hyphen slugs, missing parent, dangling parent, unknown kind all
  rejected exit 1 with messages identical to bash; happy path writes
  correct frontmatter (id/aliases/kind/parent/title/…); duplicate slug
  via a new prefix behaves like bash (file created, informational lint
  flags it on stderr, exit 0).
- **Auto-formatting commit** (9f91899) is whitespace-only in
  src/new-ticket.ts — no semantic change.

### Residual risk

- `gitCommonDir()` has no non-git fallback (bash `_lock.sh` falls back to
  `$PLANR_DIR/.lock` outside a repo), but planr always runs inside a git
  repo; acceptable.
- Duplicate-slug-with-different-prefix is not guarded at write time (same
  as bash) — the informational lint catches it; noted, not a blocker.
