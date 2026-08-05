---
id: parser-foundation
aliases: [parser-foundation]
kind: story
parent: port-scripts-to-typescript
title: Build the TS parser foundation (no scripts touched)
status: done
assignee: null
created: 2026-07-30
updated: 2026-08-01
tags: [parser, typescript, foundation]
depends_on: []
---

## Goal

Stand up the typed parsing layer that every ported script will call, proven
against fixtures (including an Obsidian-reformatted file and a body `---`
thematic break) before any script is changed. This is the derisk step for the
whole port: if the parser handles every existing ticket shape and the
known-fragile cases, the migration is on solid ground.

## Context

Today all six scripts parse frontmatter with awk helpers (`fm_field`/`fm_list`)
that toggle an in-frontmatter flag on *any* `---` line — so a body thematic
break re-enters parsing — and parse inline-list `depends_on` with a regex that
silently reads block-style YAML as empty (disabling gating). The scout
inventory (skills/planr/scripts/lint.sh, board.sh, claim.sh, merge-task.sh,
review.sh) cataloged 5 scalar fields read (id, kind, status, parent, title),
1 inline-list (depends_on), and 4 body-parse sites (## Acceptance, ##
Validation, last ## Review verdict, [[wiki-links]]). Only the YAML is hard;
the body is ~50 lines of regex.

Research chose `eemeli/yaml` over gray-matter (stale, CVE-2025-64718) and
js-yaml (smaller but no comment preservation). See
[[port-scripts-to-typescript]] notes for the full decision record.

## Acceptance

- [ ] `package.json` declares devDeps (typescript, esbuild, vitest) and one
  runtime dep (`yaml` = eemeli/yaml); `tsconfig.json` sets
  `erasableSyntaxOnly: true` (no enum/namespace — keeps native `.ts` execution
  on Node 25 possible) and strict mode.
- [ ] `src/parse.ts` exports pure functions taking a string blob, no IO:
  `splitFrontmatter(blob) -> { fm, body, raw }` reading only the FIRST
  `---\n…\n---` block (never re-enters on a body `---`);
  `parseFrontmatter(fm) -> Record<string, unknown>` via `yaml.parse`;
  `extractWikiLinks(body) -> string[]` (regex `\[\[([^\]|#]+)(?:#…)?(?:\|…)?\]\]`,
  skipping fenced code blocks, stripping `|alias`/`#heading`, deduped);
  `extractSection(body, name) -> string` (state machine on `^##`, returns
  lines until the next `^##`, heading line excluded);
  `extractLastReviewVerdict(body) -> string | null` (last `## Review` block
  wins, `verdict:` value trimmed).
- [ ] `src/ticket.ts` exports typed shapes: `Kind` (`const` object + union, not
  enum), `Status`, `ParsedTicket` (the 5 read scalars + `depends_on: string[]`
  - `aliases` + `links: string[]` + raw body).
- [ ] `tests/fixtures/` includes: a canonical task; an **Obsidian-reformatted**
  fixture (block-style `depends_on`, sorted keys, quoted `status: "done"`,
  `aliases:` as block list); a fixture with a body `---` thematic break
  followed by a line that looks like frontmatter; a fixture with multiple
  `## Review` blocks (proves last-wins); a fixture with `[[a|label]]`,
  `[[b#heading]]`, and a `[[c]]` inside a fenced code block (proves code-fence
  skip).
- [ ] `tests/parse.test.ts` (vitest) asserts every fixture parses to the
  expected `ParsedTicket`, including: block-style `depends_on` parses to the
  same array as inline; quoted `status: "done"` parses to `done`; body `---`
  does NOT re-enter frontmatter; wiki-link in a code fence is NOT extracted;
  last review verdict wins and is trimmed.
- [ ] A `parseTicket(blob)` convenience composes the above and is the single
  entry point scripts will use.
- [ ] `npm test` (vitest) passes; `npm run build` (esbuild `--bundle
  --platform=node --format=cjs`) produces a single `dist/parse.js` with no
  `node_modules` required at runtime (smoke-test: `node -e "require('./dist/parse.js')"`
  succeeds in a clean dir).

## Notes

- 2026-07-30 created. Start here — this story gates every other story in the
  epic. Do NOT touch any `scripts/*.sh` in this story.
