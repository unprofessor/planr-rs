# planr user guide

## Choosing a worktree path

`claim` hides the worktree it creates from git, so that a worktree landing
inside the working tree is not staged as a gitlink. The rule goes in
`.git/info/exclude`, which every worktree of the clone shares, and git anchors
it to whichever working tree it is evaluating. Its reach is therefore wider
than the one directory it was written for: `claim x --worktree scratch` run
inside one worktree writes `/scratch/`, which also hides a `scratch/` at the
top of trunk and of every sibling worktree -- a path that need not have
anything to do with planr.

Pick an explicit relative `--worktree` name with that in mind. The default
location has no such problem, since nothing else is called
`<plan-dir>/worktrees/`.

## Abandoning a ticket

Use the `abandon` command when a ticket is overtaken by events (OBE) or
intentionally will not be done. Provide a free-text message explaining why:

```bash
planr abandon task obsolete-task "OBE -- requirement dropped"
planr abandon story postponed-story "Won't do: deferred to Q3 planning"
```

If the message is omitted or `-` is passed, the message is read from stdin
(like `git commit`):

```bash
planr abandon task obsolete-task <<EOF
OBE -- the feature was replaced by the new search API.
EOF
```

The command writes `status: abandoned`, a refreshed `updated` date into the
frontmatter, and appends a `## Reason Abandoned` section with your message.
It does not require a worker validation or review verdict. An existing
`plan/<slug>` branch is treated as active work: `abandon` refuses and leaves
the branch and worktree untouched, so cleanup is an explicit human decision.

Because it refuses while a branch exists, `abandon` never learns which path
the task's worktree had, and `close` -- which is what normally removes that
path's local ignore rule -- never runs for an abandoned ticket. So after the
commit lands, `abandon` also tidies `.git/info/exclude`: it drops every rule
in planr's own block that no live worktree still justifies, keeping the ones
that are still in use (including the shared `<plan-dir>/worktrees/` parent)
and never touching a rule you wrote yourself. This runs for every kind of
ticket, including an epic, a story, or a task that was never claimed --
worktrees are repo-wide, and a stale rule left behind silently hides whatever
is created at that path. If the rules cannot be pruned, `abandon` says so on
stderr and still reports the abandonment.

An abandoned ticket does **not** satisfy `depends_on`; only `status: done`
unblocks a dependency. Update the dependency relationship or abandon the
dependent ticket separately.
