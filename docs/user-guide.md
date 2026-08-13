# planr user guide

## Abandoning a ticket

Use the `abandon` command when a ticket is overtaken by events (OBE) or
intentionally will not be done. Provide a free-text message explaining why:

```bash
planr abandon task obsolete-task "OBE — requirement dropped"
planr abandon story postponed-story "Won't do: deferred to Q3 planning"
```

If the message is omitted or `-` is passed, the message is read from stdin
(like `git commit`):

```bash
planr abandon task obsolete-task <<EOF
OBE — the feature was replaced by the new search API.
EOF
```

The command writes `status: abandoned`, a refreshed `updated` date into the
frontmatter, and appends a `## Reason Abandoned` section with your message.
It does not require a worker validation or review verdict. An existing
`plan/<slug>` branch is treated as active work: `abandon` refuses and leaves
the branch and worktree untouched, so cleanup is an explicit human decision.

An abandoned ticket does **not** satisfy `depends_on`; only `status: done`
unblocks a dependency. Update the dependency relationship or abandon the
dependent ticket separately.
