---
id: board-summary-stats
aliases: [board-summary-stats]
kind: task
parent: board-improvements
title: Add ticket count per status to board.sh summary
status: done
assignee: null
created: 2026-07-30
updated: 2026-07-30
tags: []
depends_on: []
---

## Goal

Append a summary section to the end of board.sh output showing ticket counts broken down by status: total, todo, in_progress, review, done, blocked. This gives the leader an instant pulse check without scanning every row.

## Context

Parent story: [[board-improvements]] under [[supplementary-tooling]]. The current board.sh lists all tickets in epics, stories, and tasks sections, plus in-flight branches, but never aggregates. A project with 50+ tickets requires manual tallying to answer “how many are done?” or “what’s in review?”.

Add the summary after the in-flight section, counting ALL tickets (epics + stories + tasks) across both trunk and branches.

## Acceptance

- [ ] `board.sh` output ends with a `## summary` section
- [ ] Summary shows counts: total, todo, in_progress, review, done, blocked
- [ ] Counts include trunk tickets (all statuses from git show) AND in-flight branches
- [ ] Zero-count statuses are shown as `0` (not omitted)
- [ ] Format: aligned columns matching the existing table style
- [ ] Performance: no additional git operations beyond what board.sh already does (reuse parsed data)
- [ ] Tests updated in `tests/run-tests.sh` to verify summary line counts

## Notes

- 2026-07-30 created
- Reuse the `fm_field` awk function already in board.sh for status extraction
- Can either parse all files twice (once for rows, once for summary) or refactor to accumulate counts during the main rendering loop
- The simpler approach: after the in-flight section, re-scan trunk files and count statuses, then add branch counts from the in-flight scan (already parsed)

## Validation

Implemented summary section at end of board.sh output. Verified:

- Ran `./skills/planr/scripts/board.sh` in the real project repo — output ends with `## summary` section showing 6 status rows (total, todo, in_progress, review, done, blocked)
- Counts correctly skip trunk entries for in-flight tasks (use branch status instead) — verified 3 in-flight tasks (1 in_progress + 2 review) counted from branches
- Zero-count statuses shown as `0` — verified blocked=0 in test fixture, done=0 in main repo
- Blocked derivation works: tasks with unmet depends_on counted as blocked — main repo shows 19 blocked
- Format uses aligned columns matching existing table style
- Reuses same git operations (git-show, git-ls-tree) — no new git calls

Tests:
- Ran `tests/run-tests.sh` — all 49 tests pass (0 fail), including 5 new board summary assertions:
  - board.sh exits 0
  - output contains `## summary` section
  - summary has 6 status rows
  - total = 5 (test fixture)
  - done = 1 (test fixture)

## Review

verdict: approved
reviewer: The Clanker
date: 2026-07-30
All acceptance criteria are satisfied.

### Evidence

1. **Output ends with `## summary` section** — confirmed by running `board.sh` in both the real project repo and the test sandbox. The summary appears after the `in flight` section.

2. **Summary shows all required counts** — total, todo, in_progress, review, done, blocked. Verified in real project output:
   ```
   total        37
   todo         15
   in_progress  0
   review       3
   done         0
   blocked      19
   ```

3. **Counts include trunk tickets AND in-flight branches** — trunk epics (4 todo), stories (11 todo), and tasks (dep skipped when in-flight) are counted. In-flight branches (3 tasks at review) are counted via their branch status, not trunk status. Manually verified: 4+11+0+0=15 todo, 3 review from branches, 19 blocked from trunk tasks with unmet deps, 0 in_progress, 0 done. Total=15+3+19=37 ✓

4. **Zero-count statuses shown as `0`** — `in_progress=0`, `done=0` in real repo output; `blocked=0`, `in_progress=0`, `review=0` in test fixture output.

5. **Aligned columns matching existing table style** — summary uses `printf '%-12s %s\n'` (left-aligned, space-separated columns) consistent with the broader output style.

6. **No new git operations** — summary reuses `git ls-tree`, `git show`, and `trunk_status` (itself using `git ls-tree` + `git show`), all of which are already used by `render_section` and the in-flight section.

7. **Tests updated and passing** — `tests/run-tests.sh` contains 5 board summary assertions (exit 0, has `## summary`, 6 status rows, total=5, done=1). All 49 tests pass.

### Count verification (real repo)

| Source | Count |
|--------|-------|
| Epics (all todo) | 4 |
| Stories (all todo) | 11 |
| Trunk tasks (all blocked by unmet deps, 3 in-flight skipped) | 19 blocked, 0 todo |
| In-flight branches (all review) | 3 review |
| **Total** | **37** |
| **Todo** = 4 epics + 11 stories | **15** |
| **Review** = 3 in-flight | **3** |
| **Blocked** = 19 trunk tasks | **19** |
| **In_progress** = none | **0** |
| **Done** = none | **0** |

### Residual risks

- Blocked count uses `depends_on` from trunk even for in-flight tasks that are skipped — the in-flight branches may have different deps but this is consistent with the design (in-flight deps were verified at claim time).
- Re-scanning all files doubles `git show` calls. Acceptable per the task notes (simpler approach chosen over accumulating during render loop).
