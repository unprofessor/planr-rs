//! Git plumbing: build a tree, make a commit, move one ref.
//!
//! Everything here works WITHOUT a working tree or a checkout. That is what
//! lets a verb build its declaration on a ref another worktree currently has
//! checked out, and it is why the one-branch-one-worktree rule never binds.
//!
//! The shape every verb follows:
//!   read base tree -> apply content steps -> write tree -> commit-tree -> move a ref
//! Everything before the ref move is unreferenced object construction, which
//! git garbage-collects, so a failure at any earlier point leaves nothing
//! behind.

use std::path::{Path, PathBuf};
use std::process::Command;

fn run_env(args: &[&str], index: Option<&Path>) -> Result<String, String> {
    let mut cmd = Command::new("git");
    cmd.args(args);
    if let Some(index) = index {
        cmd.env("GIT_INDEX_FILE", index);
    }
    let out = cmd
        .output()
        .map_err(|e| format!("git command failed: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&out.stderr);
        Err(stderr
            .lines()
            .rfind(|l| !l.trim().is_empty())
            .unwrap_or("git failed")
            .to_string())
    }
}

fn run(args: &[&str]) -> Result<String, String> {
    run_env(args, None)
}

fn run_stdin(args: &[&str], stdin_data: &str) -> Result<String, String> {
    use std::io::Write;
    use std::process::Stdio;

    let mut child = Command::new("git")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("git command failed: {e}"))?;
    child
        .stdin
        .as_mut()
        .ok_or("cannot write to git stdin")?
        .write_all(stdin_data.as_bytes())
        .map_err(|e| format!("cannot write to git stdin: {e}"))?;
    let out = child
        .wait_with_output()
        .map_err(|e| format!("git command failed: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&out.stderr);
        Err(stderr
            .lines()
            .rfind(|l| !l.trim().is_empty())
            .unwrap_or("git failed")
            .to_string())
    }
}

/// Like [`run_stdin`] but keeps the output as bytes, because `cat-file
/// --batch` frames its records by byte length and a lossy UTF-8 conversion
/// would move the frame boundaries.
fn run_stdin_bytes(args: &[&str], stdin_data: &str) -> Result<Vec<u8>, String> {
    use std::io::Write;
    use std::process::Stdio;

    let mut child = Command::new("git")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("git command failed: {e}"))?;
    child
        .stdin
        .as_mut()
        .ok_or("cannot write to git stdin")?
        .write_all(stdin_data.as_bytes())
        .map_err(|e| format!("cannot write to git stdin: {e}"))?;
    let out = child
        .wait_with_output()
        .map_err(|e| format!("git command failed: {e}"))?;
    if out.status.success() {
        Ok(out.stdout)
    } else {
        let stderr = String::from_utf8_lossy(&out.stderr);
        Err(stderr
            .lines()
            .rfind(|l| !l.trim().is_empty())
            .unwrap_or("git failed")
            .to_string())
    }
}

/// Read many blobs in ONE `git cat-file --batch` process.
///
/// Reading them one at a time costs a subprocess each, which is what dominates
/// a board once the per-ticket history walk is gone: the walk is O(commits)
/// but the spawns are O(tickets), and a spawn is far more expensive than the
/// read it performs.
///
/// Each spec is an object name such as `main:.plan/tickets/foo.md`. The result
/// is positional, with `None` where git reported the object missing, so a
/// caller can pair results back to specs without a second lookup.
pub fn cat_file_batch(specs: &[String]) -> Result<Vec<Option<String>>, String> {
    if specs.is_empty() {
        return Ok(Vec::new());
    }
    let input = format!("{}\n", specs.join("\n"));
    let out = run_stdin_bytes(&["cat-file", "--batch"], &input)?;

    // Records are `<oid> SP <type> SP <size> LF <contents> LF`, or
    // `<name> SP missing LF`. Framing is by the declared byte length, never by
    // scanning for a delimiter -- ticket bodies contain newlines.
    let mut results = Vec::with_capacity(specs.len());
    let mut pos = 0usize;
    while pos < out.len() && results.len() < specs.len() {
        let Some(nl) = out[pos..].iter().position(|b| *b == b'\n') else {
            break;
        };
        let header = String::from_utf8_lossy(&out[pos..pos + nl]).to_string();
        pos += nl + 1;

        if header.ends_with(" missing") {
            results.push(None);
            continue;
        }
        let Some(size) = header
            .rsplit(' ')
            .next()
            .and_then(|s| s.parse::<usize>().ok())
        else {
            return Err(format!("cannot parse cat-file header: {header}"));
        };
        if pos + size > out.len() {
            return Err("cat-file output ended mid-record".to_string());
        }
        results.push(Some(
            String::from_utf8_lossy(&out[pos..pos + size]).to_string(),
        ));
        pos += size + 1; // skip the trailing LF git appends
    }

    while results.len() < specs.len() {
        results.push(None);
    }
    Ok(results)
}

/// A ref and the commit it points at.
pub struct Ref {
    pub name: String,
    pub tip: String,
}

/// Refs under a prefix, **fully qualified**, in plumbing form -- no decoration,
/// no current branch marker, no "checked out elsewhere" marker.
///
/// Emit `%(refname)`, never `%(refname:short)`. The short form is the shortest
/// *unambiguous* name rather than the path minus its prefix, so git lengthens it
/// when a tag shares the branch's name. Do not reconstruct a full name from a
/// short one anywhere.
///
/// The tip comes back with the name because every caller that asks "is this
/// branch integrated" needs it, and asking git a second time per ref is the
/// difference between one process and one per claimed ticket.
pub fn for_each_ref(prefix: &str) -> Result<Vec<Ref>, String> {
    let out = run(&["for-each-ref", "--format=%(refname) %(objectname)", prefix])?;
    Ok(out
        .lines()
        .filter_map(|l| l.trim().split_once(' '))
        .map(|(name, tip)| Ref {
            name: name.to_string(),
            tip: tip.to_string(),
        })
        .collect())
}

/// Bring the invoking worktree in line for the one path a verb authored.
///
/// Verbs build commits with plumbing and move a ref, which never touches a
/// working tree. That is what lets them commit to a branch another worktree
/// holds -- but when the verb writes to the ref THIS worktree has checked out,
/// the result is a tree that disagrees with its own HEAD: `git status` shows
/// the new ticket as staged for deletion.
///
/// Only the authored path is touched, so a user's unrelated edits are safe.
///
/// **Both sides of the comparison are fully qualified**, and that is not
/// cosmetic. `symbolic-ref --short` returns a branch name, and once ticket refs
/// became qualified the equality could never hold for a `base: own` verb -- so
/// the guard that means "this worktree is on that ref, reconcile it" silently
/// became "elsewhere, skip". The declaration commit still landed, so the folded
/// state stayed right and no test noticed; what was lost was the `annotate`
/// content. A reviewer running `request-changes` from inside the worktree left
/// the stale copy STAGED, and the worker's next ordinary commit reverted the
/// `## Review` note -- the only thing that verb writes.
pub fn sync_path(ref_: &str, commit: &str, path: &str) -> Result<(), String> {
    let head = run(&["symbolic-ref", "--quiet", "HEAD"]).unwrap_or_default();
    if head.trim() != qualify(ref_) {
        return Ok(()); // this worktree is elsewhere; nothing to reconcile
    }
    if show(commit, path).is_ok() {
        run(&[
            "restore",
            &format!("--source={commit}"),
            "--staged",
            "--worktree",
            "--",
            path,
        ])?;
    } else {
        // The verb removed it -- archival. Drop it from index and disk alike.
        let _ = run(&["rm", "-q", "--cached", "--ignore-unmatch", "--", path]);
        let _ = std::fs::remove_file(path);
    }
    Ok(())
}

pub fn log_raw(args: &[&str]) -> Result<String, String> {
    let mut full = vec!["log"];
    full.extend_from_slice(args);
    run(&full)
}

/// Whether this repository's history is truncated.
///
/// It matters only where ABSENCE is the answer: a walk that runs out of
/// history reports "no such record" in exactly the way a walk that reached the
/// root does. Anything treating absence as proof has to know the difference.
/// Errs rather than guessing. `unwrap_or(false)` would fail open into the very
/// hole the caller is guarding -- reporting a repository as fully cloned
/// because the question could not be asked.
pub fn is_shallow() -> Result<bool, String> {
    run(&["rev-parse", "--is-shallow-repository"]).map(|out| out.trim() == "true")
}

/// Hand every WHOLE record in `pending` to `on_record`, leaving the partial
/// tail behind for the next read to complete. Returns how many records were
/// consumed and whether the caller asked to stop.
///
/// Split out from [`log_streaming`] because the boundary handling is the part
/// that can be wrong invisibly: a read boundary falls wherever the pipe
/// decides, so a bug here would show up only on histories large enough to fill
/// the buffer. Testing it needs arbitrary chunkings, not a git repository.
fn drain_records(
    pending: &mut Vec<u8>,
    sep: u8,
    on_record: &mut impl FnMut(&str) -> bool,
) -> (usize, bool) {
    let mut start = 0;
    let mut consumed = 0;
    let mut stopped = false;
    while let Some(offset) = pending[start..].iter().position(|b| *b == sep) {
        let end = start + offset;
        let keep_going = on_record(&String::from_utf8_lossy(&pending[start..end]));
        consumed += 1;
        start = end + 1;
        if !keep_going {
            stopped = true;
            break;
        }
    }
    pending.drain(..start);
    (consumed, stopped)
}

/// A child that is reaped however the scope ends, including by unwinding.
///
/// `std::process::Child` deliberately has no reaping `Drop`, so every early
/// return has to reap by hand -- and a panic is an early return no hand can
/// cover. The callback below is the CALLER's code, so a panic in it unwinds
/// straight through this function; without this guard each one left a zombie.
/// The blast radius in a short-lived CLI is nil, which is precisely the
/// argument that keeps being wrong about the next caller.
struct Reaped(std::process::Child);

impl Drop for Reaped {
    fn drop(&mut self) {
        // Both are no-ops once the child has been waited for, which is the
        // ordinary path: `kill` reports InvalidInput and `wait` returns the
        // status it already has.
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Walk `git log` a record at a time, and stop when the caller has enough.
///
/// [`log_raw`] reads git to completion before a single record is parsed, so a
/// walk that needs only the newest few commits still pays for every commit in
/// history. This one hands whole records to `on_record` as they arrive, and
/// KILLS the child the moment it returns false. Draining to EOF instead would
/// bound the parsing and nothing else -- the walk itself is the cost.
///
/// Records are terminated by `sep`, which is a byte rather than something the
/// caller's format string is scanned for: the record layout belongs to the
/// caller, not here. A record straddling two reads is held over.
///
/// Returns the number of records consumed -- one per commit git emitted,
/// which is the measured cost of the walk.
pub fn log_streaming(
    args: &[&str],
    sep: u8,
    mut on_record: impl FnMut(&str) -> bool,
) -> Result<usize, String> {
    use std::io::Read;
    use std::process::Stdio;

    let mut child = Reaped(
        Command::new("git")
            .arg("log")
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("git command failed: {e}"))?,
    );
    let child = &mut child.0;

    // Both pipes are drained concurrently. Only stdout is read in this thread,
    // so stderr left until the end would deadlock the pair as soon as a
    // command filled its ~64KB buffer while we were still reading stdout --
    // git blocked on the write, us blocked on the read. `git log` is not
    // chatty enough to reach that today, which is exactly why the guard has to
    // be structural: the next caller of this primitive will not check.
    let mut errors = child.stderr.take().ok_or("cannot read git stderr")?;
    let draining = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = errors.read_to_end(&mut buf);
        buf
    });

    let mut stdout = child.stdout.take().ok_or("cannot read git stdout")?;
    let mut chunk = [0u8; 32 * 1024];
    let mut pending: Vec<u8> = Vec::new();
    let mut consumed = 0usize;
    let mut stopped = false;

    loop {
        let n = match stdout.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = draining.join();
                return Err(format!("cannot read git output: {e}"));
            }
        };
        pending.extend_from_slice(&chunk[..n]);

        let (n, halt) = drain_records(&mut pending, sep, &mut on_record);
        consumed += n;
        stopped = halt;
        if stopped {
            break;
        }
    }

    // Closing the read end first means git sees EPIPE rather than blocking on
    // a full pipe while it waits to be killed.
    drop(stdout);

    if stopped {
        // An early stop is the success case, so nothing here is reported as a
        // failure: a killed child has no meaningful exit status, and git
        // writing into a closed pipe is expected. A genuinely broken
        // invocation -- bad revision, not a repository -- produces no records
        // at all, so it cannot reach this branch; it is caught below.
        let _ = child.kill();
        let _ = child.wait();
        let _ = draining.join();
        return Ok(consumed);
    }

    // A trailing record with no separator. Formats here terminate every record
    // with one, so this normally holds only git's own trailing newline.
    if !pending.is_empty() {
        let record = String::from_utf8_lossy(&pending);
        if !record.trim().is_empty() {
            consumed += 1;
            on_record(&record);
        }
    }

    let status = child
        .wait()
        .map_err(|e| format!("git command failed: {e}"))?;
    let errors = draining.join().unwrap_or_default();
    if status.success() {
        Ok(consumed)
    } else {
        let stderr = String::from_utf8_lossy(&errors);
        Err(stderr
            .lines()
            .rfind(|l| !l.trim().is_empty())
            .unwrap_or("git failed")
            .to_string())
    }
}

pub fn rev_parse(ref_: &str) -> Result<String, String> {
    Ok(run(&["rev-parse", "--verify", "--quiet", ref_])?
        .trim()
        .to_string())
}

pub fn ref_exists(ref_: &str) -> bool {
    rev_parse(ref_).map(|s| !s.is_empty()).unwrap_or(false)
}

/// Read a file's content at a ref. Errors when the path is absent there.
pub fn show(ref_: &str, path: &str) -> Result<String, String> {
    run(&["show", &format!("{ref_}:{path}")])
}

/// How many commits `tip` carries that `base` cannot reach -- i.e. what would
/// be lost if `tip`'s ref were deleted.
pub fn count_unreachable(base: &str, tip: &str) -> Result<usize, String> {
    let out = run(&["rev-list", "--count", &format!("{base}..{tip}")])?;
    out.trim()
        .parse()
        .map_err(|e| format!("cannot count commits on {tip}: {e}"))
}

/// Commits reachable from `includes` that nothing in `excludes` can reach --
/// `git rev-list <includes> --not <excludes>` -- in ONE process.
///
/// Every ancestry question this model asks is a question about a SET, and
/// answering it pairwise with `merge-base --is-ancestor` costs a process per
/// pair. The live callers ask two of them: *has this branch been integrated?*
/// (`rev_list([tip], [trunk])` is empty) and *which of these commits can the
/// answering ref not reach?*
///
/// An empty `includes` returns early to save a process, not for correctness:
/// `git rev-list --not <ref>` exits 0 with no output. An empty backlog reaches
/// it on every command.
pub fn rev_list(includes: &[String], excludes: &[String]) -> Result<Vec<String>, String> {
    if includes.is_empty() {
        return Ok(Vec::new());
    }
    let mut args: Vec<&str> = vec!["rev-list"];
    args.extend(includes.iter().map(String::as_str));
    if !excludes.is_empty() {
        args.push("--not");
        args.extend(excludes.iter().map(String::as_str));
    }
    let out = run(&args)?;
    Ok(out
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect())
}

pub fn is_ancestor(a: &str, b: &str) -> bool {
    Command::new("git")
        .args(["merge-base", "--is-ancestor", a, b])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// A scratch index, so tree building never disturbs the real one.
pub struct ScratchIndex {
    path: PathBuf,
}

impl ScratchIndex {
    /// Populate a scratch index from `base`'s tree.
    pub fn from_ref(base: &str) -> Result<ScratchIndex, String> {
        let unique = format!(
            "planr-next-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        );
        let path = std::env::temp_dir().join(unique);
        let index = ScratchIndex { path };
        run_env(&["read-tree", base], Some(&index.path))?;
        Ok(index)
    }

    pub fn put(&self, path: &str, content: &str) -> Result<(), String> {
        let sha = run_stdin(&["hash-object", "-w", "--stdin"], content)?
            .trim()
            .to_string();
        run_env(
            &[
                "update-index",
                "--add",
                "--cacheinfo",
                &format!("100644,{sha},{path}"),
            ],
            Some(&self.path),
        )?;
        Ok(())
    }

    pub fn remove(&self, path: &str) -> Result<(), String> {
        run_env(&["update-index", "--force-remove", path], Some(&self.path))?;
        Ok(())
    }

    pub fn write_tree(&self) -> Result<String, String> {
        Ok(run_env(&["write-tree"], Some(&self.path))?
            .trim()
            .to_string())
    }
}

impl Drop for ScratchIndex {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

pub fn commit_tree(tree: &str, parents: &[&str], message: &str) -> Result<String, String> {
    let mut args: Vec<String> = vec!["commit-tree".to_string(), tree.to_string()];
    for p in parents {
        args.push("-p".to_string());
        args.push(p.to_string());
    }
    args.push("-m".to_string());
    args.push(message.to_string());
    let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    Ok(run(&refs)?.trim().to_string())
}

/// `refs/heads/<name>`, unless it already names a ref path.
///
/// Idempotent on purpose. A ticket's own ref is resolved fully qualified --
/// the short form lets a same-named tag win git's lookup order and shadow the
/// branch -- while trunk stays as the user wrote it, because `--trunk` accepts
/// a commit-ish and `refs/heads/HEAD` names nothing. These helpers take both,
/// so neither caller has to know which it is holding.
fn qualify(name: &str) -> String {
    if name.starts_with("refs/") {
        name.to_string()
    } else {
        format!("refs/heads/{name}")
    }
}

/// Create a branch atomically: the empty old-value means "must not exist", so
/// two concurrent claims of one ticket resolve by compare-and-swap and the
/// loser gets a clear failure rather than a silent overwrite.
pub fn create_ref(name: &str, new: &str) -> Result<(), String> {
    run(&["update-ref", &qualify(name), new, ""])
        .map_err(|e| format!("cannot create branch '{name}': {e}"))?;
    Ok(())
}

/// Move a ref, asserting what it currently points at.
///
/// `git update-ref` takes an expected-old-value for ANY ref move, not just
/// creation, so every effect can be a compare-and-swap for the price of one
/// argument. Without it `claim` was atomic while `submit` and `approve` were
/// not, which made the safety look like a property of claiming rather than of
/// moving a ref.
pub fn update_ref(name: &str, new: &str, old: &str) -> Result<(), String> {
    run(&["update-ref", &qualify(name), new, old]).map_err(|e| {
        format!("cannot move '{name}': {e}\nit moved since this verb read it -- re-run to work from the new tip")
    })?;
    Ok(())
}

pub fn delete_ref(name: &str) -> Result<(), String> {
    if !ref_exists(name) {
        return Ok(()); // idempotent -- a re-run after partial failure completes
    }
    run(&["update-ref", "-d", &qualify(name)])?;
    Ok(())
}

/// Integrate `source` into `target`. Fast-forwards when it can; otherwise
/// builds a real merge commit without checking anything out. A conflict is
/// reported rather than resolved -- the target is left untouched.
pub fn merge_into(target: &str, source: &str, message: &str) -> Result<String, String> {
    let target_sha = rev_parse(target)?;
    let source_sha = rev_parse(source)?;

    if is_ancestor(&target_sha, &source_sha) {
        update_ref(target, &source_sha, &target_sha)?;
        return Ok(source_sha);
    }

    let tree = run(&["merge-tree", "--write-tree", &target_sha, &source_sha]).map_err(|e| {
        format!("merge conflict integrating '{source}' into '{target}': {e}\nrebase the branch onto {target} and re-run; {target} is untouched")
    })?;
    let tree = tree
        .lines()
        .next()
        .ok_or("merge-tree produced no tree")?
        .trim()
        .to_string();
    let merge = commit_tree(&tree, &[&target_sha, &source_sha], message)?;
    update_ref(target, &merge, &target_sha)?;
    Ok(merge)
}

/// Record `source` as merged into `target` WITHOUT applying its changes,
/// using a tree supplied by the caller. This is `merge -s ours` with one path
/// taken from the other side: the source's commits become reachable from the
/// target, so nothing is garbage-collected, while the target's tree is
/// unchanged apart from that path.
///
/// Never fast-forwards -- a fast-forward would apply the work, which is the
/// one thing this exists to avoid.
pub fn merge_ticket_only(
    target: &str,
    source: &str,
    tree: &str,
    message: &str,
) -> Result<String, String> {
    let target_sha = rev_parse(target)?;
    let source_sha = rev_parse(source)?;
    let commit = commit_tree(tree, &[&target_sha, &source_sha], message)?;
    update_ref(target, &commit, &target_sha)?;
    Ok(commit)
}

/// `git worktree add <path> <branch>` -- checking out an EXISTING branch.
///
/// Deliberately not `crate::git::worktree_add`: that helper omits the branch
/// argument when the branch already exists, which makes git invent a new
/// branch named after the path. The classic commands never hit it because they create
/// branch and worktree together; here the ref is created first, by design.
pub fn worktree_add(path: &str, branch: &str) -> Result<(), String> {
    run(&["worktree", "add", path, branch]).map(|_| ())
}

pub fn worktree_remove(path: &str) -> Result<(), String> {
    run(&["worktree", "remove", "--force", path]).map(|_| ())
}

#[cfg(test)]
mod streaming {
    use super::*;

    const SEP: u8 = b'\x1e';

    /// Feed one byte stream through [`drain_records`] in fixed-size chunks,
    /// as a pipe would deliver it.
    fn records_at_chunk_size(stream: &[u8], size: usize) -> Vec<String> {
        let mut got = Vec::new();
        let mut pending = Vec::new();
        let mut sink = |r: &str| {
            got.push(r.to_string());
            true
        };
        for chunk in stream.chunks(size) {
            pending.extend_from_slice(chunk);
            drain_records(&mut pending, SEP, &mut sink);
        }
        got
    }

    #[test]
    fn a_record_split_across_reads_is_still_one_record() {
        // The boundary falls wherever the pipe decides, so every boundary has
        // to produce the same records -- including one inside a separator's
        // own record and one exactly on it.
        let want: Vec<String> = (0..7)
            .map(|i| format!("commit-{i}-{}", "x".repeat(i * 3)))
            .collect();
        let stream: Vec<u8> = want
            .iter()
            .flat_map(|r| r.bytes().chain(std::iter::once(SEP)))
            .collect();

        for size in 1..=stream.len() + 2 {
            assert_eq!(
                records_at_chunk_size(&stream, size),
                want,
                "records differ when read in chunks of {size} bytes"
            );
        }
    }

    #[test]
    fn a_partial_tail_is_held_over_rather_than_delivered() {
        let mut pending = Vec::from(&b"one\x1etw"[..]);
        let mut got = Vec::new();
        let (n, stopped) = drain_records(&mut pending, SEP, &mut |r: &str| {
            got.push(r.to_string());
            true
        });
        assert_eq!((n, stopped), (1, false));
        assert_eq!(got, vec!["one".to_string()]);
        assert_eq!(pending, b"tw", "the partial record must survive the read");
    }

    #[test]
    fn a_stop_leaves_the_rest_of_the_buffer_alone() {
        let mut pending = Vec::from(&b"one\x1etwo\x1ethree\x1e"[..]);
        let mut got = Vec::new();
        let (n, stopped) = drain_records(&mut pending, SEP, &mut |r: &str| {
            got.push(r.to_string());
            r != "two"
        });
        assert_eq!((n, stopped), (2, true));
        assert_eq!(got, vec!["one".to_string(), "two".to_string()]);
        assert_eq!(
            pending, b"three\x1e",
            "records after the stop must not be consumed"
        );
    }

    /// The process half of [`log_streaming`], against the repository the test
    /// itself is running in -- the only git history a unit test has without
    /// changing the process-wide working directory, which parallel tests share.
    /// Skipped rather than failed where there is none, e.g. an unpacked crate.
    fn in_a_repository() -> bool {
        rev_parse("HEAD").map(|s| !s.is_empty()).unwrap_or(false)
    }

    #[test]
    fn streaming_a_log_yields_the_same_records_as_reading_it_whole() {
        if !in_a_repository() {
            return;
        }
        // Padded so the output runs well past the read buffer: without that,
        // a boundary bug never gets the chance to show.
        let format = format!("--format=%H{}\x1e", "p".repeat(4096));
        let args = ["-n", "40", &format, "HEAD"];

        let whole = log_raw(&args).unwrap();
        let want: Vec<&str> = whole
            .split('\x1e')
            .filter(|r| !r.trim().is_empty())
            .collect();

        let mut got = Vec::new();
        let n = log_streaming(&args, b'\x1e', |r| {
            got.push(r.trim_matches('\n').to_string());
            true
        })
        .unwrap();

        assert_eq!(n, want.len());
        assert_eq!(got.len(), want.len());
        for (got, want) in got.iter().zip(&want) {
            assert_eq!(got, want.trim_matches('\n'));
        }
    }

    #[test]
    fn stopping_early_is_not_an_error_and_reads_no_further() {
        if !in_a_repository() {
            return;
        }
        // Reads THIS repository, so the expectation has to come from it rather
        // than be hardcoded: a shallow checkout has as little as one commit,
        // and asserting a fixed count made the test pass locally and fail on
        // CI, which clones with `fetch-depth: 1` by default. Count first, then
        // stop one short of the end -- that is what proves the walk was cut
        // rather than merely finished.
        let args = ["--format=%H\x1e", "HEAD"];
        let available = log_streaming(&args, b'\x1e', |_| true).unwrap();
        if available < 2 {
            return;
        }
        let stop_at = available - 1;

        let mut seen = 0;
        let n = log_streaming(&args, b'\x1e', |_| {
            seen += 1;
            seen < stop_at
        })
        .unwrap();
        assert_eq!((n, seen), (stop_at, stop_at));
        assert!(
            n < available,
            "the walk read everything instead of stopping"
        );
    }

    #[test]
    fn a_genuine_git_failure_is_still_reported() {
        // The one thing an early stop must not paper over: a killed child and
        // a broken invocation both end the walk without EOF.
        let mut seen = 0;
        let err = log_streaming(&["--format=%H\x1e", "no/such/revision"], b'\x1e', |_| {
            seen += 1;
            true
        })
        .expect_err("a bad revision must not look like a bounded walk");
        assert!(!err.trim().is_empty(), "a failure with no message");
        assert_eq!(seen, 0, "a failed walk should have produced no records");
    }
}
