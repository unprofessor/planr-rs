//! Helpers shared by the end-to-end scenarios: throwaway repos, the
//! `planr` and `git` runners, and the readers the assertions go through.

use assert_cmd::Command;
use std::path::Path;
use std::process::Output;

/// Create a throwaway git repo at `dir`, seeded with a minimal .plan/.
pub fn init_repo(dir: &Path) {
    // git init
    Command::new("git")
        .args(["init", "-b", "main"])
        .arg(dir)
        .ok()
        .unwrap();
    git_must(dir, &["config", "user.email", "e2e@test"]);
    git_must(dir, &["config", "user.name", "E2E Test"]);

    // Create .plan dirs with a placeholder so git tracks them
    for d in &[".plan/epics", ".plan/stories", ".plan/tasks"] {
        std::fs::create_dir_all(dir.join(d)).unwrap();
    }
    // Git doesn't track empty dirs -- write a .gitkeep
    std::fs::write(dir.join(".plan/.gitkeep"), "").unwrap();

    // Initial commit
    let out = Command::new("git")
        .args(["add", ".plan"])
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git add failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = Command::new("git")
        .args(["commit", "-m", "seed plan"])
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git commit failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Run planr with args, return the output.
pub fn planr(dir: &Path, args: &[&str]) -> Output {
    Command::cargo_bin("planr")
        .unwrap()
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap()
}

/// Run planr, expect success, return stdout.
pub fn planr_ok(dir: &Path, args: &[&str]) -> String {
    let out = planr(dir, args);
    assert!(
        out.status.success(),
        "planr {args:?} failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

/// Run planr, expect failure, return stderr.
pub fn planr_err(dir: &Path, args: &[&str]) -> String {
    let out = planr(dir, args);
    assert!(!out.status.success(), "planr {args:?} expected failure");
    String::from_utf8(out.stderr).unwrap().trim().to_string()
}

/// Run planr, expect success, return (stdout, stderr).
pub fn planr_ok_both(dir: &Path, args: &[&str]) -> (String, String) {
    let out = planr(dir, args);
    assert!(
        out.status.success(),
        "planr {args:?} failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    (
        String::from_utf8(out.stdout).unwrap().trim().to_string(),
        String::from_utf8(out.stderr).unwrap().trim().to_string(),
    )
}

/// Run git in `dir` and require it to succeed.
pub fn git_must(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

pub fn write_file(dir: &Path, rel: &str, body: &str) {
    let path = dir.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
}

pub fn git_stdout(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

/// Read `.git/info/exclude`, or "" when it does not exist.
pub fn read_exclude(dir: &Path) -> String {
    std::fs::read_to_string(dir.join(".git/info/exclude")).unwrap_or_default()
}

/// The same file as bytes. `.git/info/exclude` is a list of paths, and on
/// Unix a path is bytes -- a file holding one the reader cannot decode is
/// exactly the case worth asserting about, and `read_exclude` cannot see it.
pub fn read_exclude_bytes(dir: &Path) -> Vec<u8> {
    std::fs::read(dir.join(".git/info/exclude")).unwrap_or_default()
}

/// Ask git itself whether `path` is ignored *from within `dir`*.
///
/// Reading the exclude file only tells you what was written; git anchors a
/// leading-slash pattern to whichever working tree it is evaluating, so only
/// `check-ignore` run in the right tree answers the question that matters.
pub fn git_ignored(dir: &Path, path: &str) -> bool {
    Command::new("git")
        .args(["check-ignore", "-q", path])
        .current_dir(dir)
        .output()
        .unwrap()
        .status
        .success()
}

pub fn seed_lint_repo(dir: &Path) {
    init_repo(dir);
    // Create epic + story + task via planr
    planr_ok(dir, &["new", "epic", "e1", "Epic One"]);
    planr_ok(dir, &["new", "story", "s1", "Story One", "e1"]);
    planr_ok(dir, &["new", "task", "t1", "Task One", "s1"]);

    // Commit the clean backlog for ref-mode lint
    git_must(dir, &["add", ".plan"]);
    git_must(dir, &["commit", "-m", "clean backlog"]);
}

/// Read the first output line of `planr board [args]` -- the source header.
pub fn board_header(dir: &Path, args: &[&str]) -> String {
    let out = planr(dir, args);
    assert!(out.status.success(), "planr {args:?} failed");
    String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .next()
        .unwrap_or_default()
        .to_string()
}

/// Add a git worktree at `path` on a new `branch`, run from `dir`.
pub fn git_worktree_add(dir: &Path, path: &Path, branch: &str) {
    let out = Command::new("git")
        .args(["worktree", "add", "-b", branch])
        .arg(path)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git worktree add failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Trunk must stay clean after a claim: no dirty status, no `dirty` in the
/// board header, and no gitlink staged by the leader's `git add`.
pub fn assert_trunk_undisturbed(dir: &Path, case: &str) {
    let porcelain = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(dir)
        .output()
        .unwrap();
    let porcelain = String::from_utf8(porcelain.stdout).unwrap();
    assert!(
        porcelain.trim().is_empty(),
        "{case}: claim left trunk dirty: {porcelain}"
    );

    let board = planr_ok(dir, &["board"]);
    let header = board.lines().next().unwrap_or_default();
    assert!(!header.contains("dirty"), "{case}: board header: {header}");

    // The leader's normal flow must not stage the worktree as a gitlink.
    planr_ok(dir, &["new", "task", "t9", "Task Nine", "s1"]);
    git_must(dir, &["add", "-A"]);
    let staged = Command::new("git")
        .args(["ls-files", "-s"])
        .current_dir(dir)
        .output()
        .unwrap();
    let staged = String::from_utf8(staged.stdout).unwrap();
    assert!(
        !staged.contains("160000"),
        "{case}: worktree staged as a gitlink: {staged}"
    );
}

pub fn find_ticket_filename(plan_dir: &Path, kind_dir: &str, slug: &str) -> String {
    let tickets_dir = plan_dir.join(format!(".plan/{kind_dir}"));
    for entry in std::fs::read_dir(tickets_dir).unwrap() {
        let e = entry.unwrap();
        let name = e.file_name().into_string().unwrap();
        if name.ends_with(&format!("-{slug}.md")) {
            return name;
        }
    }
    panic!("{kind_dir}/{slug} not found");
}

pub fn find_task_slug(plan_dir: &Path, slug: &str) -> String {
    find_ticket_filename(plan_dir, "tasks", slug)
}

// ---------------------------------------------------------------------------
// Helpers for specific test paths
// ---------------------------------------------------------------------------

pub fn t1_path_of(dir: &Path) -> String {
    format!(".plan/tasks/{}", find_task_slug(dir, "t1"))
}

pub fn t2_path_of(dir: &Path) -> String {
    format!(".plan/tasks/{}", find_task_slug(dir, "t2"))
}

pub fn t3_path_of(dir: &Path) -> String {
    format!(".plan/tasks/{}", find_task_slug(dir, "t3"))
}

/// Take a claimed task to `review` inside its worktree and commit it, so
/// `close` will merge.
pub fn approve_in_worktree(wt: &Path, root: &Path, slug: &str) {
    let task_file = format!(".plan/tasks/{}", find_task_slug(root, slug));
    let content = std::fs::read_to_string(wt.join(&task_file)).unwrap();
    std::fs::write(
        wt.join(&task_file),
        content.replace("status: in_progress", "status: review")
            + "\n\n## Review\n\nverdict: approved\nreviewer: test\ndate: 2026-09-01\n",
    )
    .unwrap();
    git_must(wt, &["add", "-A"]);
    git_must(wt, &["commit", "-m", &format!("review: {slug}")]);
}

/// Delete the loose object behind `<ref>:<path>`.
///
/// The tree still names the file, so `git ls-tree` lists it and `git show`
/// cannot produce it -- a backlog planr can see and cannot open. Corrupt
/// enough to be worth saying out loud, and cheaper to build than the
/// permissions and packfile damage that produce the same read in the wild.
pub fn destroy_blob(dir: &Path, ref_: &str, path: &str) {
    let out = Command::new("git")
        .args(["rev-parse", &format!("{ref_}:{path}")])
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git rev-parse {ref_}:{path} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let oid = String::from_utf8(out.stdout).unwrap().trim().to_string();
    let obj = dir.join(".git/objects").join(&oid[..2]).join(&oid[2..]);
    std::fs::remove_file(&obj).unwrap_or_else(|e| panic!("could not remove {obj:?}: {e}"));
}
