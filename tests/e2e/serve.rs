//! Scenarios for `planr serve`: what the pages link to, what they refuse to
//! render, and which backlog they read.
//!
//! These drive the real binary over a real socket. The server picks its own
//! port (`--port 0`) and prints it, so the tests never collide with each
//! other or with a developer's own `planr serve`.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::{Child, Command, Stdio};

use crate::common::*;

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

/// A running `planr serve`, killed when the test drops it.
struct Serving {
    child: Child,
    port: u16,
}

impl Drop for Serving {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Start `planr serve` in `dir` and wait until it says which port it took.
fn start(dir: &Path, args: &[&str]) -> Serving {
    let mut child = Command::new(assert_cmd::cargo::cargo_bin("planr"))
        .arg("serve")
        .args(args)
        .current_dir(dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("planr serve did not start");

    // Two lines: the source it is reading, then the URL. Reading the URL is
    // also the readiness signal -- it is printed after the socket is bound.
    let stdout = child.stdout.take().expect("serve stdout was not piped");
    let mut lines = BufReader::new(stdout).lines();
    let _source = lines.next().expect("serve printed nothing").unwrap();
    let url = lines.next().expect("serve printed no URL").unwrap();

    let port = url
        .rsplit(':')
        .next()
        .and_then(|p| p.trim_end_matches('/').parse().ok())
        .unwrap_or_else(|| panic!("could not read a port out of {url:?}"));

    Serving { child, port }
}

/// GET `path` and return the status code and body.
fn get(port: u16, path: &str) -> (u16, String) {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("could not reach serve");
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n"
    )
    .unwrap();

    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).unwrap();
    let text = String::from_utf8_lossy(&raw).into_owned();

    let (head, body) = text
        .split_once("\r\n\r\n")
        .unwrap_or_else(|| panic!("no header/body split in {text:?}"));
    let status = head
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .unwrap_or_else(|| panic!("no status code in {head:?}"));

    (status, body.to_string())
}

/// A backlog with one task pointing at another, one dangling link, a
/// wiki-link quoted in inline code, and a script tag.
fn seed_serve_repo(dir: &Path) {
    seed_lint_repo(dir);
    planr_ok(dir, &["new", "task", "t2", "Task Two", "s1"]);

    let t1 = t1_path_of(dir);
    let body = std::fs::read_to_string(dir.join(&t1)).unwrap();
    write_file(
        dir,
        &t1,
        &format!(
            "{body}\n\n## Notes\n\n\
             Live link to [[t2]] and a broken one to [[no-such-ticket]].\n\n\
             The syntax itself is written `[[quoted-in-code]]`.\n\n\
             <script>alert('xss')</script>\n"
        ),
    );

    // t2 waits on t1, so t1's page has a dependent to show.
    let t2 = t2_path_of(dir);
    let t2body = std::fs::read_to_string(dir.join(&t2)).unwrap();
    write_file(
        dir,
        &t2,
        &t2body.replace("depends_on: []", "depends_on: [t1]"),
    );

    git_must(dir, &["add", ".plan"]);
    git_must(dir, &["commit", "-m", "linked backlog"]);
}

/// Put t1 in flight: a `plan/t1` branch whose task file says `in_progress`,
/// with trunk left at `todo`, which is exactly the disagreement the board and
/// the ticket page have to report.
fn claim_t1_on_a_branch(dir: &Path) {
    let t1 = t1_path_of(dir);
    let trunk = std::fs::read_to_string(dir.join(&t1)).unwrap();

    git_must(dir, &["checkout", "-b", "plan/t1"]);
    write_file(
        dir,
        &t1,
        &trunk.replace("status: todo", "status: in_progress"),
    );
    git_must(dir, &["commit", "-am", "claim t1"]);
    git_must(dir, &["checkout", "main"]);
}

// ---------------------------------------------------------------------------
// Scenarios
// ---------------------------------------------------------------------------

#[test]
fn test_e2e_serve_board_links_every_ticket() {
    let td = tempfile::tempdir().unwrap();
    seed_serve_repo(td.path());
    let s = start(td.path(), &["--port", "0"]);

    let (code, body) = get(s.port, "/");
    assert_eq!(code, 200);
    for slug in ["e1", "s1", "t1", "t2"] {
        assert!(
            body.contains(&format!("href=\"/t/{slug}\"")),
            "board does not link {slug}: {body}"
        );
    }
}

#[test]
fn test_e2e_serve_ticket_shows_reverse_edges() {
    let td = tempfile::tempdir().unwrap();
    seed_serve_repo(td.path());
    let s = start(td.path(), &["--port", "0"]);

    let (code, body) = get(s.port, "/t/t1");
    assert_eq!(code, 200);
    // t2 declares `depends_on: [t1]`, and nothing in t1 records that. The
    // reverse edge is the whole reason this page beats reading the file.
    assert!(
        body.contains("dependents"),
        "t1 page has no dependents section"
    );
    assert!(
        body.contains("href=\"/t/t2\""),
        "t1 page does not link its dependent t2: {body}"
    );
    // s1 is t1's parent, so s1's page must list t1 as a child.
    let (_, s1) = get(s.port, "/t/s1");
    assert!(s1.contains("children"), "s1 page has no children section");
    assert!(
        s1.contains("href=\"/t/t1\""),
        "s1 does not link its child t1"
    );
}

#[test]
fn test_e2e_serve_wiki_links_navigate_and_dangle() {
    let td = tempfile::tempdir().unwrap();
    seed_serve_repo(td.path());
    let s = start(td.path(), &["--port", "0"]);

    let (_, body) = get(s.port, "/t/t1");
    assert!(
        body.contains("href=\"/t/t2\""),
        "live wiki-link did not become a link: {body}"
    );
    assert!(
        body.contains("href=\"/dangling/no-such-ticket\""),
        "dangling wiki-link did not route to /dangling/: {body}"
    );

    // The dangling page names who refers to the slug rather than 404ing bare.
    let (code, missing) = get(s.port, "/dangling/no-such-ticket");
    assert_eq!(code, 404, "a slug nothing claims must not answer 200");
    assert!(
        missing.contains("referred to by") && missing.contains("href=\"/t/t1\""),
        "dangling page does not name its referrer: {missing}"
    );
}

#[test]
fn test_e2e_serve_leaves_wiki_links_in_code_alone() {
    let td = tempfile::tempdir().unwrap();
    seed_serve_repo(td.path());
    let s = start(td.path(), &["--port", "0"]);

    let (_, body) = get(s.port, "/t/t1");
    assert!(
        body.contains("<code>[[quoted-in-code]]</code>"),
        "a wiki-link quoted in code was rewritten: {body}"
    );
    assert!(
        !body.contains("quoted-in-code\">") && !body.contains("/dangling/quoted-in-code"),
        "a wiki-link quoted in code became a link: {body}"
    );
}

#[test]
fn test_e2e_serve_drops_raw_html_from_bodies() {
    let td = tempfile::tempdir().unwrap();
    seed_serve_repo(td.path());
    let s = start(td.path(), &["--port", "0"]);

    let (_, body) = get(s.port, "/t/t1");
    // `serve <ref>` is a reasonable way to read a branch someone else wrote.
    // A ticket body must never be able to run code in the reader's browser.
    assert!(
        !body.contains("<script>"),
        "a script tag in a ticket body reached the page: {body}"
    );
    assert!(
        !body.contains("alert('xss')"),
        "script contents survived into the page: {body}"
    );
}

#[test]
fn test_e2e_serve_ref_mode_reads_the_commit_not_the_tree() {
    let td = tempfile::tempdir().unwrap();
    seed_serve_repo(td.path());

    // On disk only -- never committed.
    planr_ok(
        td.path(),
        &["new", "task", "uncommitted", "Not Committed", "s1"],
    );

    let tree = start(td.path(), &["--port", "0"]);
    let (_, from_tree) = get(tree.port, "/");
    assert!(
        from_tree.contains("href=\"/t/uncommitted\""),
        "working-tree mode missed an uncommitted ticket"
    );
    drop(tree);

    let at_ref = start(td.path(), &["HEAD", "--port", "0"]);
    let (_, from_ref) = get(at_ref.port, "/");
    assert!(
        !from_ref.contains("href=\"/t/uncommitted\""),
        "ref mode showed a ticket that is not in the commit: {from_ref}"
    );
    assert!(
        from_ref.contains("href=\"/t/t1\""),
        "ref mode missed a committed ticket"
    );
}

#[test]
fn test_e2e_serve_in_flight_leads_the_board() {
    let td = tempfile::tempdir().unwrap();
    seed_serve_repo(td.path());
    claim_t1_on_a_branch(td.path());
    let s = start(td.path(), &["--port", "0"]);

    let (_, body) = get(s.port, "/");
    let in_flight = body
        .find("in flight")
        .expect("board has no in-flight section");
    let first_table = body.find("<h2>epics").expect("board has no epics section");
    assert!(
        in_flight < first_table,
        "the in-flight section is not the first thing on the board: {body}"
    );
}

#[test]
fn test_e2e_serve_ticket_status_is_one_badge() {
    let td = tempfile::tempdir().unwrap();
    seed_serve_repo(td.path());
    claim_t1_on_a_branch(td.path());
    let s = start(td.path(), &["--port", "0"]);

    // Trunk still says todo while the branch says in_progress. The branch is
    // where the work is, so the field's one badge is in_progress; both reports
    // and who made them live in the hover-over.
    let (_, body) = get(s.port, "/t/t1");
    let meta = body
        .split_once("<dl class=\"meta\">")
        .and_then(|(_, rest)| rest.split_once("</dl>"))
        .map(|(m, _)| m)
        .expect("ticket page has no metadata block");
    let (field, pop) = meta
        .split_once("<div class=\"pop\">")
        .expect("the status field offers no hover-over for the branch that disagrees");
    assert_eq!(
        field.matches("class=\"st ").count(),
        1,
        "the status field itself renders more than one badge: {field}"
    );
    assert!(
        field.contains("class=\"st st-in_progress\""),
        "the field shows trunk's status rather than the claiming branch's: {field}"
    );
    // Both sources, each named, each status a real pill -- the same classes
    // the field uses, so a status reads as a status at a glance.
    assert!(
        pop.contains("<code>plan/t1</code>") && pop.contains("<code>trunk</code>"),
        "the hover-over does not name both sources: {pop}"
    );
    assert!(
        pop.contains("class=\"st st-in_progress\"") && pop.contains("class=\"st st-todo\""),
        "the hover-over does not badge both reported statuses: {pop}"
    );
}

#[test]
fn test_e2e_serve_board_does_not_asterisk_a_branch_status() {
    let td = tempfile::tempdir().unwrap();
    seed_serve_repo(td.path());
    claim_t1_on_a_branch(td.path());
    let s = start(td.path(), &["--port", "0"]);

    let (_, body) = get(s.port, "/");
    assert!(
        !body.contains("<abbr"),
        "the board still marks a branch status with an asterisk: {body}"
    );
    // The provenance survives the asterisk's removal, as hover text.
    assert!(
        body.contains("title=\"reported by plan/t1; trunk still records todo\""),
        "the board no longer says where t1's status came from: {body}"
    );
}

#[test]
fn test_e2e_serve_lint_rows_read_like_every_other_page() {
    let td = tempfile::tempdir().unwrap();
    seed_serve_repo(td.path());
    let s = start(td.path(), &["--port", "0"]);

    let (code, body) = get(s.port, "/lint");
    assert_eq!(code, 200);
    // The ticket column is the same monospace slug link the board uses rather
    // than a bare anchor -- one link style across the pages, and a filename no
    // ticket answers for routes to /dangling/ instead of a dead /t/ URL.
    assert!(
        body.contains("<a class=\"slug\" href=\"/t/t1\">"),
        "the lint page does not link its ticket column like the board: {body}"
    );
    // A finding's level is the first thing to read, so it carries a color of
    // its own instead of arriving as plain text.
    assert!(
        body.contains("class=\"lv lv-warning\""),
        "the lint page does not mark a finding's level: {body}"
    );
}

#[test]
fn test_e2e_serve_unknown_routes_404() {
    let td = tempfile::tempdir().unwrap();
    seed_serve_repo(td.path());
    let s = start(td.path(), &["--port", "0"]);

    assert_eq!(get(s.port, "/nope").0, 404);
    assert_eq!(get(s.port, "/t/not-a-ticket").0, 404);
    // The lint page answers whether or not the backlog is clean.
    assert_eq!(get(s.port, "/lint").0, 200);
}
