# Changelog

## [Unreleased]

### Added

- **`planr board`** now prints a source header before the board, showing
  where the tickets were read from: the working-tree path, the resolved
  commit id, and the ref name (the current branch in parentheses, or the
  commit-ish you passed). In working-tree mode a trailing `dirty` marks
  uncommitted changes; ref mode omits it since it reads committed data.

### Changed

- **`planr close` and `planr abandon` now say when it was git that would not
  answer, rather than blaming a missing ticket file.** Looking up a ticket by
  slug starts by listing the plan subdirectory, and a listing that failed was
  treated as a listing that came back empty: the command refused either way --
  which is the right direction -- but it refused with "no task file for 't1'
  on plan/t1", sending the reader to look at a file that may be sitting right
  where they left it. The listing error is now reported as itself.

- **`planr board`** now defaults to the current on-disk working tree
  instead of trunk, matching `planr lint`. Pass a commit-ish to read a
  specific ref (e.g. `planr board main`, `planr board HEAD~2`, or a SHA);
  omit it to see the `.plan` files in whatever worktree or branch you have
  checked out.

### Fixed

- **A task's status is whatever its file says, even when that reads like one
  of the board's stand-ins.** The branch scan reports `(no task file)` and
  `(unreadable)` for a task file it could not read, and it recognized them by
  their text, so a ticket whose frontmatter carried one of those strings was
  reported as a missing file rather than as the invalid status it is. The
  scan now returns which of the three it means.

- **`planr board <ref>` no longer presents a total it computed from ticket
  files it never opened.** The reader skipped a ticket file whose blob it
  could not `show` -- a damaged object, a packfile planr cannot open, a
  permissions problem -- so a backlog planr opened none of rendered as an
  empty board, indistinguishable from a backlog that holds nothing. The
  board now counts the ticket files it found as well as the ones it read,
  and says on stderr when the second number is smaller, naming which of the
  two happened: a board built from part of a backlog, or one that
  established nothing at all. A backlog that is there and holds no tickets
  yet is still silent, here and in `planr lint <ref>`.

- **`planr close story` and `planr close epic` no longer close a parent
  whose children they could not read.** The gate that refuses a close over
  unfinished children built its list of children by listing the plan
  subdirectory and reading each ticket, and it dropped both failures on the
  floor: a listing that failed became an empty listing, and a ticket whose
  blob would not `show` was skipped. A short list reads exactly like a
  complete one in which everything is done, so a story whose only task planr
  could not open was closed with that task still `todo`, and the epic above
  it went the same way. Both reads are now fatal to the close: planr reports
  what it could not read and leaves the parent open. The informational hint
  that suggests closing the parent next still tolerates a failed read --
  losing one line of advice is not worth failing a command that has already
  done its work.

- **planr keeps the ignore rules it did not write, whatever encoding they
  are in.** `.git/info/exclude` is a list of paths, and on Unix a path is
  bytes -- a `/caf\xe9/` rule written years ago under Latin-1 is an ordinary
  thing to find there. planr read the file as text and rewrote it whole, so
  one such byte made the read fail, the failure was taken for an empty file,
  and `claim` wrote back planr's own block alone: `/build/`, `/node_modules/`
  and every other rule gone, exit 0, no warning, and the file is untracked,
  so git cannot put it back. `close` read the same file the same way and
  simply did nothing, leaving its own rule behind for good. The whole
  pipeline now works over bytes: every line planr does not own comes back
  exactly as it went in, and a prune keeps a line it cannot even decode,
  because planr never writes one. A read that fails for any reason other than
  "the file is not there" is now reported instead of being rewritten over.

- **`planr close` finishes its cleanup when run from inside the worktree it
  removes.** Closing your own task from your own worktree is how a worker
  runs the command, and it left the process standing in a directory that no
  longer existed, so every git run after the removal failed at `chdir`
  before git was reached. Two things went with it, neither naming the cause:
  the ignore rule for a custom worktree path could not be dropped, so a rule
  hiding a path that was gone outlived it and silently hid whatever was
  created there next; and the "all tasks under this story are done" hint was
  computed from a failed listing and dropped, so the close that finished a
  story said nothing about it. `close` now steps out to the worktree holding
  trunk before removing anything, and says so if it cannot.

- **`planr lint <ref>` no longer calls a backlog with no tickets in it
  absent.** A repository that has scaffolded `.plan/epics`, `.plan/stories`
  and `.plan/tasks` and written no tickets yet was told there was no backlog
  at that ref to lint and sent to check `--plan-dir` and the ref, when both
  were right and the backlog was there -- working-tree `lint` and `board`
  are silent on the identical state. Zero tickets is not zero backlog: ref
  mode now asks the ref the same question working-tree mode asks the
  filesystem, whether anything is committed under that path at all, and
  warns only when the answer is nothing. A ref that does not resolve is
  reported as its own thing, quoting git, rather than as a finding about the
  backlog.

- **`planr lint <ref>` says when it could not read the tickets it found.** The
  reader skips a ticket file whose blob it cannot show, so a plan directory
  full of tickets planr opened none of rendered the way a clean backlog does
  -- no output, exit 0, and a clean bill of health on tickets nothing had
  looked at. The report now carries the count of ticket files found alongside
  the count read, and ref mode says which of the two happened: that planr read
  none of the ticket files it found under the plan directory, or that it read
  some of them and what it could not read is missing from the run. Neither
  message blames the plan directory or the ref, which a populated listing has
  just cleared. A backlog holding no ticket files at all is still silent.

- **`planr claim` prints the worktree path it resolved.** `claim --worktree
  ../out` run from a subdirectory printed `/repo/sub/../out`. It opens, but
  it is the path the caller pastes into their next command, and it was not
  the path planr itself had used: the ignore rule is anchored after
  normalizing, so planr wrote `/out` while handing back the caller's `..`.
  The typed path is now normalized once, up front, so the directory git
  creates, the rule that hides it, and the path printed back all name the
  same place -- an absolute path carrying `..` included.

- **The board summary counts only what a ticket table shows.** A `plan/*`
  branch whose slug names no task on trunk is listed in the in-flight table,
  but the tasks table is built from trunk, so no row there describes it --
  and yet it still added one to `total` and one to a status bucket. A ticket
  whose `kind` is missing or unrecognized was counted the same way, and all
  three tables filter on `kind`, so it appeared in none of them: three rows
  against a total of four, with nothing on the page to say which was right.
  Both now count only if some ticket table shows them.

- **A ticket no table can show is named on stderr.** A ticket the reader
  cannot see anywhere has no way of getting fixed. `planr board` now warns
  for each ticket it cannot place -- naming it, saying whether its `kind` is
  unrecognized or its frontmatter failed to parse, and pointing at `planr
  lint`, which explains the file in full. A ticket that carries no `id` of
  its own is named after its file, the way `lint` names it: a file with no
  frontmatter at all, or with frontmatter that simply omits `id`, otherwise
  warned as "(no id)", so two broken files produced two identical lines that
  told the reader nothing about either.

- **A branch whose task is not on trunk is called out by name.** Such a
  branch is listed in flight and counts towards nothing. The branch scan now
  warns on stderr that no task of that slug is among the tickets the board
  read, so the gap between the in-flight table and the totals is explained
  rather than merely correct.

- **The board no longer reports a present ticket as missing.** That warning
  fired whenever a branch's slug was absent from the board's list of trunk
  tasks, and blamed a cause it had not established: "renamed or not
  committed". A ticket whose frontmatter fails to parse reads as every field
  absent, `kind` included, so it drops out of that list while its file sits
  committed and unmoved -- and the reader was sent hunting for it. The board
  now says what it can establish: that the ticket is there and did not parse.
  A list that came back empty says nothing about any individual slug either,
  so it no longer produces one such warning per branch -- it says once that
  the board read no tickets at all, and how many in-flight branches that
  leaves counting towards nothing.

- **Every command uses the backlog at the repository root.** A relative
  `--plan-dir` (the default `.plan`) is relative to the repository, but every
  command opened it relative to the directory it was run from, and each was
  wrong about it in its own way. From anywhere but the root, `board` rendered
  empty tables and warned that every in-flight branch named a task that was
  not on trunk; `lint` printed a clean bill of health for a backlog it had
  never opened, exiting 0; `new` created a second backlog under the
  subdirectory, printed the path and exited 0, so `board` in that same
  directory reported a total of zero for a ticket the tool had just made; and
  `claim`, `close` and `review` refused, each naming a file as absent from
  trunk while it sat committed at the root. All of them now work at the
  repository root, so there is one answer to where the backlog is and every
  command gives it. The one thing that does not move with them is a path the
  caller typed: `claim --worktree <relative path>` is resolved against the
  directory planr was invoked in before the root is entered, so it still
  lands where the caller pointed.

- **`planr new` and `planr claim` print paths that resolve from where the
  caller is standing.** Both printed a path relative to the repository root,
  which is not where the caller necessarily is: `$EDITOR $(planr new task
  ...)` run from a subdirectory took the printed path at face value and
  created a stray file under the subdirectory instead of opening the ticket.
  Both print absolute paths now.

- **A plan directory planr cannot read is not an empty one.** `Path::exists`
  answers true for a directory whose permissions forbid opening it, and for a
  plain file sitting where the backlog should be. Every reader below treats
  an I/O error as an empty directory, so either one came out byte-identical
  to a clean backlog: `lint` exit 0, no stdout, no stderr, certifying a
  backlog it had not managed to open. Reading nothing is now told apart from
  there being nothing, for the plan directory and for each of its `epics/`,
  `stories/` and `tasks/` subdirectories -- one unreadable `tasks/` hides
  every task in the backlog. A directory that is simply not there is still
  ordinary, and says what it always said.

- **A prune that cannot drop planr's stale ignore rules says so, and keeps
  them.** Most of the ways the prune can fail returned in silence -- an
  exclude file that could not be locked, listed, opened, or read. Keeping the
  rules is the right direction, but each silent path left behind exactly the
  artifact the prune exists to remove: a rule that hides whatever is created
  at that path, invisible in `git status`. `planr abandon` would report
  success with an empty stderr. Two paths did not even keep them: when the
  prune could not work out where a live worktree sits, or what pattern hides
  it, it carried on and counted that worktree as needing nothing -- so the
  rule that hides it was dropped, which is the one outcome the rest of this
  file fails closed against. Every path now keeps the rules and warns, naming
  what could not be done and why, and still completes the command it was asked
  to run.

- **`planr abandon` takes the abandoned task's ignore rule with it.**
  Abandon refuses until the branch and worktree have been cleaned up by hand,
  so it never learns which path the worktree had and could not remove that
  rule by name -- and `close`, which is what normally removes it, never runs
  for an abandoned task. The rule outlived everything that referred to it and
  went on hiding whatever was created at that path, leaving no trace in
  `git status`. Abandon now drops every rule in planr's own block that no live
  worktree still justifies, keeping the ones they do -- including the shared
  `<plan-dir>/worktrees/` parent -- and never touching a rule the user wrote.

- **The board takes no status from a ticket that never named itself, and says
  so.** A ticket can end up carrying a slug it never claimed three ways: its
  frontmatter failed to parse, which swallows every field; it has no
  frontmatter at all; or it has frontmatter that simply omits `id`. In all
  three the reader names it after its file, and in two of them its status is
  the parser's default `todo`, read from nowhere. That went into the same
  slug-to-status lookup the dependency column and the summary buckets use,
  last write winning, so a duplicate of a finished ticket overwrote its
  `done`: every task depending on it showed BLOCKED-BY on the same screen
  where the ticket itself read `done`, and moved from the `todo` bucket to
  `blocked`. What settles it is not whether the board can place the ticket but
  whether the slug is the ticket's own claim, so only a ticket that declared
  its own `id` answers for that slug, and when two tickets declare the same
  one, neither does -- the board cannot tell which of them the question is
  about. An unanswered dependency already counts as unmet, which is the honest
  answer. None of it happens quietly any more: the board names, on stderr and
  with the file it came from, every ticket it shows under a filename-derived
  slug, every slug two tickets claim, and every ticket no table shows at all.

- **A branch no longer loses its warning to a broken file of the same slug.**
  The warning that a ticket is present but did not parse fired on the slug
  alone, so a slug that was both a real task on trunk and the recovered id of
  some unreadable file took it. The board then rendered that task in the tasks
  table and counted it while stderr said the branch "counts towards nothing",
  and the warning the reader could act on -- that the branch's task file
  carries a status planr does not recognize -- never appeared at all. The
  warning is now only for a slug that names no task the board could place.
  It also fires for every way a ticket can fail to be placeable, not just the
  one: keyed on the parse error alone, a file with no frontmatter at all, or
  with a kind planr does not recognize, fell through to the warning that says
  no ticket of that slug was among the tickets the board read -- printed
  directly under the line naming that very ticket, saying the opposite about
  it.

- **`planr lint` says when the plan directory is not there.** `lint` prints
  nothing for a clean backlog, so a typo'd `--plan-dir` was byte-identical to
  a clean bill of health, exit code included: it certified a backlog it had
  never opened. A directory that is simply not there is not an error -- planr
  is meant to be usable in a repository before `planr new` has ever made a
  backlog -- but it is a fact neither `lint` nor `board` can establish any
  other way. `board` shows the tickets it read, and warns when an in-flight
  branch counts towards nothing because it read none; `lint` prints nothing
  either way, so it says it in both of its modes -- the missing directory in
  working-tree mode, and in ref mode, where there is no directory to look at,
  a plan directory that holds no tickets at the ref it was given.

- **planr says when it could not ask git where the repository is.** Being
  outside a repository is ordinary and stays quiet, but any other reason git
  could not answer -- a dubious-ownership refusal, git missing from `PATH` --
  was swallowed just as quietly, and what followed was a report about a
  backlog read from wherever the process happened to start. The two are now
  told apart by reading all of git's message rather than the last line of it,
  which is the line that says "Stopping at filesystem boundary" on a perfectly
  normal run outside a repository. Reading git's wording only works if planr
  knows which wording to expect, so every git planr runs is now pinned to
  `LC_ALL=C`. Left to the environment, a translated locale broke the match and
  turned every ordinary run outside a repository into that same warning. It
  also stops a git message planr quotes back inside an English sentence from
  arriving in another language.

- **`planr claim` refuses a task trunk already records as finished.** The
  guard only rejected `abandoned`, and the branch-side check cannot help
  because `close` deletes the branch. Claiming a closed task therefore
  created a worktree, declined to move the `done` status, and exited 0. Any
  status at or past `in_progress` on trunk now refuses.

- **A failed `planr claim` takes its own ignore rule back out.** The
  rollback removed the worktree but left the rule behind, so the path
  stayed hidden from git for good. Only a rule that call wrote is removed,
  it is removed after the worktree is gone rather than before, and a rule
  any live worktree still sits under stays -- planr's default location is
  one rule covering a shared parent, so removing it on behalf of one failed
  claim would unhide every other worktree beneath it.

- **Concurrent claims no longer lose each other's ignore rules.** Rewriting
  `.git/info/exclude` is a read-modify-write and claims run in parallel by
  design, so two of them could both read the pre-rule file and both write
  it, dropping one rule and leaving that worktree to be staged as a
  gitlink. The edit now takes an exclusive lock of its own -- a separate
  file from `planr.lock`, which a claim already holds shared.

- **`planr board` reads branch names as `%(refname:lstrip=2)`.** The short
  form is the shortest *unambiguous* name, so a tag sharing a branch's name
  made git report `heads/plan/<slug>` and the scan derive a bogus slug.

- **A failed `planr claim` no longer hides a real directory from git.**
  The local ignore rule was written before `git worktree add` ran and
  nothing removed it when the worktree creation failed, so a typo'd
  `--worktree` path left that path permanently excluded -- invisible in
  `git status`, so nothing pointed at the cause. The rule is now written
  only after the worktree exists.

- **`planr close` keeps the ignore rule when it cannot remove the
  worktree.** The removal's error was discarded and the rule dropped
  regardless. `git worktree remove` without `--force` refuses whenever the
  worktree holds untracked or modified files -- a stray build artifact is
  enough -- which left the worktree in place and no longer hidden, exactly
  the gitlink corruption the rule exists to prevent.

- **Local ignore rules are anchored to the working tree that contains the
  path.** `.git/info/exclude` is shared by the whole clone, but git anchors
  a leading-slash pattern to whichever working tree it is evaluating, so
  the anchor that makes a rule fire is the tree the directory sits in --
  found by longest-prefix match over `git worktree list`. Anchoring to the
  invoking worktree was wrong for a path in another tree, and `close` run
  from a secondary worktree silently removed nothing. Paths are also
  resolved through symlinks now: a worktree reaching the repository via a
  symlinked path used to look like it lay outside the repository and got
  no rule at all (every `$TMPDIR` path on macOS takes that route).

- **`planr close` no longer deletes a worktree nested inside the one it is
  closing.** `git worktree remove` decides a worktree is safe to delete by
  asking `git status --porcelain`, which does not list ignored paths -- and
  planr's own rule hides `<plan-dir>/worktrees/` inside *every* working
  tree. A worker that claims its next task from inside its own worktree
  nests one there by default, so git's safety check could not see it and
  deleted it recursively, uncommitted work and all, while `close` reported
  success. Without the ignore rule git refuses. `close` now looks for
  registered worktrees under the one it is about to remove, leaves it in
  place if it finds any, and says which.

- **Dropping a stale worktree record drops its ignore rule too.** Resuming
  a claim whose worktree was deleted by hand forgot the record but kept the
  rule, and no later `close` would remove it -- `close` only considers the
  path the task holds now, which may be somewhere else entirely. The rule
  stayed forever, hiding anything created at the old path.

- **A failed ignore-rule removal is reported.** Every cleanup path
  discarded the error, so a rule left behind by a read-only `.git` or a
  full disk was invisible twice over: it hides files, and nothing said so.

- **planr's ignore rules end with a blank line, so a rule appended by hand
  stays the user's.** planr writes its block last, so `echo '/mydir/' >>
  .git/info/exclude` -- the obvious way to add one -- landed *inside* that
  block; planr then read the line as its own, declined to write a duplicate
  for the same path, and `close` deleted it.

- **`planr close` no longer deletes an ignore rule planr did not write.**
  Rules were deduplicated against the whole exclude file, so a claim whose
  path matched a line the user had written adopted it silently and `close`
  later removed it. planr now owns only the patterns under its own header,
  and keeps a rule that another live worktree still resolves to. Its header
  is also dropped once its own section is empty, rather than being stranded
  by any unrelated anchored rule elsewhere in the file.

- **Ignore patterns escape glob metacharacters.** A gitignore pattern is a
  glob, so a worktree at `wt[1]` was written as `/wt[1]/` -- a character
  class matching `wt1` -- leaving the real directory visible and staged as
  a gitlink.

- **Only a `todo` task can be claimed, and a claim rewrites nothing else.**
  The guard listed the statuses a claim would not touch and fell one short
  of the vocabulary: `blocked` was missing, so a worker who marked their
  branch blocked had it silently reopened as `in_progress` on the next
  claim. Stated the other way round -- `todo` is claimable, everything else
  is left alone -- adding a status can no longer break it. A task blocked on
  trunk is now refused rather than claimed, and the refusal says how to
  unblock it (`status: todo` on trunk, committed), since no planr command
  moves a ticket back to `todo`.

- **An unreadable worktree path counts as held, not as gone.** The holder
  check and `close`'s cleanup used `Path::exists`, which reports `false` for
  any I/O error, so a live worktree behind a permission error read as a
  stale record: its admin record was destroyed and a second agent took over
  a task someone was working. Both now fail closed on an error. Note the
  limit: a path under an *unmounted* mountpoint answers `ENOENT`, which is
  not an error, so a worktree on a volume that happens to be unmounted is
  indistinguishable from one deleted by hand -- git's own `prunable` flag
  makes the same call. Dropping a record therefore warns, naming the path
  and pointing at `git worktree repair`.

- **A worktree path that cannot be written as an ignore rule is refused.**
  A path holding a line break rendered as two lines in
  `.git/info/exclude`: the worktree stayed visible and was staged as a
  gitlink, the claim reported success, and neither fragment could ever be
  removed -- so `/wt` and `evil/` went on hiding unrelated paths in every
  worktree indefinitely. A path that is not one line of valid UTF-8 now
  fails the claim instead.

- **A backslash in a worktree path is a filename character on Unix.** The
  path was normalized as if `\` were a separator, so `wt\1` became the
  pattern `/wt/1/`, the real worktree stayed visible, and `git add` staged
  it as a gitlink -- the same failure the glob escaping prevents for `[`.

- **A failed `planr claim` no longer leaves its branch behind.**
  `git worktree add -b <branch> <path>` creates the branch *before* it
  validates the path, so a refused path left `plan/<slug>` in place:
  `planr board` listed an in-flight branch for a task nobody claimed, and
  `planr abandon` refused the task for having an active branch.

- **Quoted statuses are read like every other command reads them.** The
  claim guards used a frontmatter reader that did not strip YAML quotes, so
  `status: "done"` -- lint-clean, and shown as `done` on the board -- got
  past the terminal checks and was rewritten to `in_progress` and
  committed, reopening a finished task.

- **Claims of the same task serialize.** The holder check and the
  `worktree_add` it guards could interleave with a second claim of the same
  slug, so the loser got git's raw `'<path>' already exists` -- the message
  the check exists to replace. The lock is per slug, so claims of different
  tasks still run in parallel.

- **A stale worktree record is dropped one at a time.** Resuming a claim
  ran `git worktree prune`, which is repo-global and would also forget any
  worktree merely unreachable at that moment -- an unmounted volume, a
  network path -- orphaning it as a side effect of an unrelated claim.

- **`planr close` reports a cleanup it could not finish.** When
  `worktree remove` refuses (untracked files in the worktree), the branch
  delete that follows fails too; both errors were discarded and `close`
  printed unqualified success while `board` kept showing the task in
  flight. It now warns on stderr and names the command to finish the job.

- **`planr claim --no-worktree` refuses a task held by a worktree.** The
  holder check sat below the opt-out's early return, so an agent could
  `--no-worktree` claim a task another agent had checked out and be told it
  succeeded. Note the limit: the check finds holders that registered a
  worktree, and `--no-worktree` registers nothing, so two `--no-worktree`
  claims of the same task still both report success. Mutual exclusion for
  that path needs a marker the opt-out writes too, which this does not add.

- **`planr claim` refuses a branch that already reports the task as `done`
  or `abandoned`.** The terminal-status guard reads trunk, but a branch can
  be ahead of it, so resuming a claim on a finished or dead ticket rebuilt
  the worktree and reported an ordinary claim.

- **`planr board` no longer drops a branch whose plan directory is
  missing.** The last `Err(_) => continue` arm in the branch scan pushed no
  row, so such a branch vanished from the in-flight section, the counts,
  and the warnings alike.

- **`planr claim` resumes instead of refusing on a stale worktree
  record.** git keeps listing a worktree whose directory was deleted with
  `rm -rf` until something prunes it, so the refusal named a path that was
  not there and locked the task out permanently. A claim now refuses only
  when the worktree is really present, and prunes the stale record
  otherwise. A live holder is still refused.

- **Resuming a claim no longer rolls the branch back to `in_progress`.**
  Any status differing from the flip was rewritten, so re-claiming a task
  whose branch had reached `review` discarded the finished review and then
  made `close` refuse the task for having the wrong status. A claim now
  flips only a branch that has not started work.

- **`planr board` tells an invalid status apart from a missing ticket.**
  Both took the "no readable task file" warning, which sent the reader
  hunting for a file that was sitting where they left it; a typo'd status
  now says so and points at `planr lint`. The status list is shared with
  `lint` rather than duplicated, so the two cannot drift.

- **`planr board` no longer drops a task whose branch has no readable
  ticket.** The summary skipped a task that had an in-flight branch on the
  assumption the branch would supply its status, but a branch reporting
  `(no task file)` was counted nowhere -- so the ticket disappeared from
  every bucket and from `total`. Such a branch is a legitimate state (a
  renumbered file, an uncommitted ticket, a branch made by hand), so the
  count now falls back to the trunk status, and `planr board` writes a
  warning naming the branch to stderr -- a rename that detaches a branch
  from its ticket is otherwise invisible. Warnings go to stderr so the
  board on stdout stays parseable.

- **`planr board` marks a task whose status comes from an in-flight branch.**
  `claim` flips the status on the worktree branch and leaves trunk alone, so
  the tasks table reported a claimed task as `todo` for the whole life of the
  work while the summary counted it as `in_progress`. The STATUS column now
  shows the branch value with a trailing `*`, and a legend under the table
  says where it came from. A branch that has no readable task file keeps its
  placeholder in the in-flight section only, rather than inventing a ticket
  status.

- **`planr board` now shows the in-flight section from trunk.** The
  branch scan parsed the decorated output of `git branch --list`, stripping
  only the `* ` (current branch) and two-space markers. Git marks a branch
  that is checked out in a linked worktree with `+ ` instead, which is what
  every `planr claim` produces, so each name came back as `+ plan/<slug>`,
  the ref lookup failed, and the row was dropped without a word. The leader
  on trunk saw no `## in flight` section and a summary counting every claimed
  task as `todo`. The scan now asks git for the ref name rather than parsing
  decoration.

- **`planr claim <slug>` without `--worktree` now does the full claim**
  ([#4]). Since 0.3.0 an omitted `--worktree` was treated as
  `--no-worktree`, so a bare `claim` printed `claimed: <slug>` and exited 0
  having created no branch, no worktree, no status flip, and no commit --
  an agent that trusted that output would start editing on trunk, and two
  agents could each "claim" the same task without either showing as
  `in_progress`. Omitting the flag now uses the documented default path
  `<plan-dir>/worktrees/wt-<slug>`, the same as passing `--worktree` with
  no value. `--no-worktree` remains the only way to opt out.
  `claim` also now adds an ignore rule to `.git/info/exclude` for any
  worktree that lands inside the repository -- the default location and an
  explicit `--worktree` path alike. A worktree inside the working tree is
  an embedded repo, so without a rule `git add` staged it as a gitlink (a
  bogus submodule that a fresh clone cannot resolve) and trunk read dirty
  until someone ran `git rm --cached`. The rule is local because a worktree
  is local, so nothing new appears for the leader to commit; a worktree
  outside the repository gets no rule. `planr close` drops the rule again
  when it removes the worktree -- a stale rule would silently hide whatever
  is created at that path later. The shared rule for the default location
  covers a directory planr reuses for every claim, so it survives.

- **`planr claim` on a task that is already claimed** now refuses with
  `refuse claim: task '<slug>' is already claimed; its worktree is at
  <path>` instead of surfacing git's `fatal: '<path>' already exists`,
  which told an agent nothing about what went wrong.

- **`planr claim` can resume a claim whose worktree was removed.**
  `worktree_add` passed the trunk ref as the commit-ish even when the
  branch already existed, so rebuilding the worktree died with
  `fatal: '<trunk>' is already used by worktree at ...`. An existing branch
  is now its own starting point, and the status flip -- a no-op on a branch
  that already reads `in_progress` -- no longer tries to commit nothing.

- **`planr new` quotes the `title:` it writes** ([#1]). A colon in the title
  (`"Sanitary history: boundary rev, rewriter"`) produced frontmatter that
  planr's own YAML reader could not parse -- silently at creation, then as
  lint errors later. Titles are now emitted as YAML scalars, quoted whenever
  a colon, a leading indicator character, or a trailing space would otherwise
  break the parse.
- **`planr lint` reports a frontmatter parse failure as itself** ([#1]).
  A block that fails to parse reads as every-field-missing, so lint used to
  cascade into `missing id`, `kind '<missing>'`, and `must name a parent`
  findings about fields that were present -- and children of the broken
  ticket picked up wrong-parent-kind warnings. Lint now emits one error
  naming the parse failure, and still resolves the ticket's slug and kind
  from its path so its children stay clean.
- **`planr lint` says `stories directory`**, not `storys directory`.

## [0.3.1]

### Fixed

- **CI release job** fixed two build failures: Windows target now uses
  `shell: bash` so `$TARGET` resolves correctly (was empty under
  PowerShell); aarch64 Linux now sets `CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER`
  so Cargo uses the cross-compiler instead of x86_64 `rust-lld`.
- **`cargo binstall` support**: `pkg-fmt` changed from the invalid value
  `"tar.gz"` to `"tgz"` (the correct format identifier). Archives are
  now published as `.tgz` files to match binstall expectations.

## [0.3.0]

### Changed

- **`planr abandon`** replaces `--reason obe|wont-do` with a free-text
  positional message argument that supports stdin (like `git commit`).
  The message is appended as a `## Reason Abandoned` prose section instead
  of being stored in frontmatter; the `reason:` frontmatter field is removed.

- **`planr claim --worktree`/`--no-worktree`** -- the `--worktree` flag now
  accepts an optional path argument (`--worktree <path>`); passing the flag
  without a value uses `<plan-dir>/worktrees/wt-<slug>` as the default.
  The new `--no-worktree` flag skips worktree creation, returning
  `claimed: <slug>` for agents that manage their own workspace.
  The old positional worktree argument has been removed.

### Docs

- **README** overhauled: `cargo install planr` promoted to primary install
  method; source build demoted to a subsection; prebuilt binaries section
  now honestly states none are published yet; Dependencies and Binary size
  sections removed as superfluous.
- **Usage section** condensed to `planr --help` reference; inline command
  list and Subcommand details table removed in favor of self-documenting
  CLI (no more drift).
- **Abandoning a ticket** narrative moved from README to a new
  [user guide](docs/user-guide.md) with a link from the Usage section.

### Internal

- **CI**: added `release` job that builds binaries for 5 targets
  (x86_64/aarch64 Linux, x86_64/aarch64 macOS, x86_64 Windows) on tag
  push and attaches them to the GitHub release. Archives match the
  `[package.metadata.binstall]` pattern so `cargo binstall planr` works
  out of the box.
- **ASCII-only sweep**: replaced em dashes and arrows with ASCII
  equivalents across CHANGELOG, user guide, and Rust source comments.
  Test harness strings with non-ASCII are preserved (they verify
  user-input passthrough).

## [0.2.0] -- 2026-08-11

### Added

- **`planr abandon` command** -- a separate, explicit workflow for tickets
  that are overtaken by events (OBE) or will not be done, bypassing the
  review gate:
  - Supports all ticket kinds: `planr abandon task|story|epic <slug> --reason obe|wont-do`
  - Records `status: abandoned`, `reason: <obe|wont-do>`, and refreshed `updated` date
    in frontmatter; commits directly on trunk.
  - Refuses to abandon tickets with an active `plan/<slug>` branch or worktree --
    never merges or discards work silently.
  - Rejects re-abandoning an already abandoned ticket.
  - `abandoned` is accepted by lint and counted by the board summary.
  - Abandoned dependencies intentionally remain blocking: only `status: done`
    unblocks a `depends_on` relationship.
  - Full unit and end-to-end test coverage.

- **`close task` now hints** when the parent story can also be closed (all
  sibling tasks done). Similarly, `close story` hints when the parent epic
  can be closed.

### Changed

- **Lint** accepts `abandoned` as a valid status in addition to the existing
  `todo`, `in_progress`, `review`, `done`, and `blocked`.

- **Board summary** renders a separate `abandoned` count row alongside the
  existing status counts.

- **Project layout** updated in README to reflect the new `abandon.rs` module.

### Fixed

- The `close story` and `close epic` gate messages no longer reference
  legacy script paths; they use the `planr close` command form consistently.

### Internal

- Migrated CI to GitHub Actions (fmt, clippy, tests on every push/PR).
- Integrated `semvertag-shell` for build-time git-derived versioning and
  `cargo-semvertag` for version-regression checks in CI.
- Removed all non-ASCII characters and applied `cargo clippy --fix` / `cargo fmt`.
- Licensed project under MIT.

[#1]: https://github.com/unprofessor/planr-rs/issues/1
[#4]: https://github.com/unprofessor/planr-rs/issues/4
