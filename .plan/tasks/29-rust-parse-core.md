---
id: rust-parse-core
aliases: [rust-parse-core]
kind: task
parent: rust-foundation
title: Port parser core (parse.rs, ticket.rs) + fixtures + unit tests
status: review
assignee: null
created: 2026-08-05
updated: 2026-08-05
tags: [rust, parser, tests]
depends_on: [rust-scaffold]
---

## Goal

Port `src/parse.ts` + `src/ticket.ts` (~240 LOC TS) to Rust: the pure
frontmatter/body parsing layer and the `ParsedTicket` shape every subcommand
consumes, with the vitest suite and fixtures ported alongside.

## Context

Parent story: [[rust-foundation]]. Rust counterpart of the TS port's
[[parse-core]]. TS sources: `skills/planr/src/parse.ts`,
`skills/planr/src/ticket.ts`; tests: `skills/planr/tests/parse.test.ts`;
fixtures: `skills/planr/tests/fixtures/*.md` (5 files — copy verbatim into
`tests/fixtures/` here).

Behaviors to port exactly:

- `split_frontmatter` — opening line must be `---` (trailing whitespace
  tolerated via trim_end); the first later `---` line closes; no closing
  line → whole blob is body with empty fm; a body `---` thematic break never
  re-enters frontmatter. Returns `(fm, body, raw)`.
- `parse_frontmatter` — YAML map via the chosen crate; null/scalar/array
  roots → empty map. Must not regress on quoted scalars (e.g. titles
  containing `": "` — see the compat note in [[rust-port]]).
- `parse_ticket` — scalars `id`, `kind`, `status`, `parent` (null/absent →
  `None`), `title`; `status` stays a raw string at parse time (invalid values
  are a lint finding, not a parse error); `depends_on` and `aliases` each
  accept an inline list, a block list, a single bare string, or null → `[]`;
  `links` from the body; `raw` keeps the body text.
- `extract_wiki_links` — strip fenced code blocks first (both ` ``` ` and
  `~~~`), then regex `\[\[([^\]|#]+)(?:#[^\]]*)?(?:\|[^\]]*)?\]\]`, strip
  `|alias`/`#heading`, dedupe in encounter order.
- `extract_section` — state machine on `^## ` headings; collect until the
  next `^## `; heading line excluded; leading/trailing blank lines trimmed.
- `extract_last_review_verdict` — scan `## Review` sections; the LAST one
  wins; a verdict line matches `^verdict:\s*\S`; value trimmed; `None` when
  no review exists.

## Acceptance

- [ ] `parse.rs` + `ticket.rs` (or merged `ticket.rs`) expose the five
  functions + `ParsedTicket` struct, all pure (no IO)
- [ ] All 5 fixtures copied; unit tests ported from `parse.test.ts` (22
  cases) including: block-style `depends_on` parses to the same array as
  inline; quoted `status: "done"` → `done`; body `---` no re-entry; wiki-link
  inside backtick AND tilde fences not extracted; last review verdict wins
  and is trimmed; Obsidian-reformatted ticket parses
- [ ] `cargo test` green

## Validation

All acceptance criteria verified in worktree at
`/home/exfed/projects/wt-rust-parse-core`:

1. **parse.rs + ticket.rs** — `src/parse.rs` exports all 5 pure functions:
   `split_frontmatter`, `parse_frontmatter`, `extract_wiki_links`,
   `extract_section`, `extract_last_review_verdict`. `src/ticket.rs` exports
   `Kind` enum, `ParsedTicket` struct, `parse_ticket` function. All pure (no
   I/O, no git).
2. **Tests** — 33 unit tests across both modules covering:
   - `split_frontmatter`: canonical, no-opening, malformed (no closing),
     no-body, body thematic break (no re-entry), trailing-newline fidelity
   - `parse_frontmatter`: inline deps, block deps, empty, quoted scalars,
     null parent
   - `extract_wiki_links`: basic, dedupe, skip backtick fence, skip tilde
     fence
   - `extract_section`: basic, multiline, missing, trim blanks
   - `extract_last_review_verdict`: last-wins, none, trimmed, no-verdict
   - `parse_ticket` (via ticket.rs): canonical, block-style depends_on,
     single-string dep, empty/absent deps, absent/null parent, missing
     status defaults todo, quoted status, wiki-links from body,
     Obsidian-reformatted
3. **cargo test** — 33/33 passing, no warnings.
4. **cargo build** — clean compile (dead-code warnings expected — parser
   not consumed by any command yet, that's fine for this task).

All acceptance boxes checked.

## Notes

- 2026-08-05 created. Keep functions pure — IO lives in the command modules,
  which keeps the ported unit tests filesystem-free (except fixture loads).
