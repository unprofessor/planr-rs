//! Browser view of the backlog -- a loopback HTTP server that renders the
//! board, each ticket, and the lint report as linked HTML.
//!
//! The backlog is re-read on every request rather than snapshotted at boot.
//! A backlog is edited by agents while this is running, and a page showing
//! what the tree held at startup is worse than no page: it is confidently
//! stale, with nothing on it saying so. Ninety small files cost under a
//! millisecond to walk, so there is nothing to buy by caching.
//!
//! Nothing here writes. `serve` is a reader like `board` and `lint`, and the
//! socket is bound to loopback, so the only thing that reaches it is a
//! browser on this machine.

use std::collections::HashMap;
use std::sync::Arc;

use pulldown_cmark::{html, Event, Options, Parser};
use tiny_http::{Header, Request, Response, Server};

use crate::board::{self, BranchStatus};
use crate::lint;
use crate::ticket::{Kind, ParsedTicket};

/// Threads answering requests.
///
/// A browser opens several connections for one page, and tiny_http is
/// blocking: on one thread a request that stalls in git holds up the rest of
/// the page. Four is enough to keep that from being visible without pretending
/// this is a server that needs a pool.
const WORKERS: usize = 4;

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Serve the backlog until interrupted.
///
/// `port` 0 asks the OS for a free one, which is the default: the port is
/// printed either way, so a fixed one is only needed to keep a bookmark alive.
pub fn run(port: u16, ref_: Option<String>, plan_dir: &str) -> Result<(), String> {
    // Loopback, never 0.0.0.0. The backlog is repository content, and binding
    // it to every interface would publish an unauthenticated copy of it to the
    // network the moment someone runs this on a shared machine. Do not widen
    // the bind address without putting something in front of it.
    let addr = format!("127.0.0.1:{port}");
    let server = Server::http(&addr).map_err(|e| format!("cannot listen on {addr}: {e}"))?;
    let bound = server
        .server_addr()
        .to_ip()
        .ok_or_else(|| "listening socket has no IP address".to_string())?;

    let source = match &ref_ {
        Some(r) => format!("ref '{r}'"),
        None => "the working tree".to_string(),
    };
    println!("planr serve: reading {source} from '{plan_dir}'");
    println!("http://127.0.0.1:{}/", bound.port());

    let server = Arc::new(server);
    let ctx = Arc::new(Context {
        ref_,
        plan_dir: plan_dir.to_string(),
    });

    let mut workers = Vec::new();
    for _ in 0..WORKERS {
        let server = Arc::clone(&server);
        let ctx = Arc::clone(&ctx);
        workers.push(std::thread::spawn(move || {
            while let Ok(request) = server.recv() {
                answer(request, &ctx);
            }
        }));
    }
    for w in workers {
        // A panicked worker has already printed its own message; the others
        // keep serving, so there is nothing to add and nothing to fail on.
        let _ = w.join();
    }
    Ok(())
}

/// What every request needs to know: which backlog, and at what ref.
struct Context {
    ref_: Option<String>,
    plan_dir: String,
}

// ---------------------------------------------------------------------------
// Routing
// ---------------------------------------------------------------------------

fn answer(request: Request, ctx: &Context) {
    let url = request.url().to_string();
    let path = url.split('?').next().unwrap_or("/");
    let path = percent_decode(path);

    let snapshot = Snapshot::read(ctx);
    let index = Index::build(&snapshot.tickets);

    let (code, body) = match path.as_str() {
        "/" => (200, page_board(ctx, &snapshot, &index)),
        "/lint" => (200, page_lint(ctx)),
        p if p.starts_with("/t/") => match index.by_slug.get(&p["/t/".len()..]) {
            Some(t) => (200, page_ticket(t, &snapshot, &index)),
            None => (404, page_missing(&p["/t/".len()..], &index)),
        },
        p if p.starts_with("/dangling/") => (404, page_missing(&p["/dangling/".len()..], &index)),
        _ => (404, page_not_found(&path)),
    };

    let header = "Content-Type: text/html; charset=utf-8"
        .parse::<Header>()
        .expect("a constant header always parses");
    let response = Response::from_string(body)
        .with_status_code(code)
        .with_header(header);
    // A browser that hung up mid-response is ordinary and not this process's
    // problem; there is no caller left to tell.
    let _ = request.respond(response);
}

// ---------------------------------------------------------------------------
// Reading the backlog
// ---------------------------------------------------------------------------

/// One request's view of the backlog.
struct Snapshot {
    tickets: Vec<ParsedTicket>,
    branches: Vec<BranchStatus>,
    /// Ticket files found, read or not -- see `board::TrunkTickets`.
    ticket_files: usize,
}

impl Snapshot {
    fn read(ctx: &Context) -> Snapshot {
        let read = match &ctx.ref_ {
            Some(r) => board::read_ref_tickets(r, &ctx.plan_dir),
            None => board::read_working_tree_tickets(&ctx.plan_dir),
        };
        Snapshot {
            tickets: read.tickets,
            branches: board::read_in_flight_branches(&ctx.plan_dir),
            ticket_files: read.ticket_files,
        }
    }
}

/// The relationships the pages navigate on, built per request.
///
/// Deliberately not a graph module. Three maps over one pass is what these
/// pages need, and the `ticket-graph` epic owns the real thing -- an adjacency
/// structure with traversal, transitive queries, and its own output formats.
/// When that lands this comes out; until then, do not grow it into a second
/// implementation of it.
struct Index<'a> {
    /// Slug -> the ticket claiming it. First claim wins, so a contested slug
    /// resolves somewhere rather than nowhere; `contested` records that it
    /// was a coin toss, and the ticket page says so.
    by_slug: HashMap<&'a str, &'a ParsedTicket>,
    contested: std::collections::HashSet<&'a str>,
    children: HashMap<&'a str, Vec<&'a ParsedTicket>>,
    /// Reverse `depends_on`: slug -> the tickets waiting on it.
    dependents: HashMap<&'a str, Vec<&'a ParsedTicket>>,
    /// Reverse wiki-links: slug -> the tickets whose bodies mention it.
    backlinks: HashMap<&'a str, Vec<&'a ParsedTicket>>,
    status_of: HashMap<String, String>,
}

impl<'a> Index<'a> {
    fn build(tickets: &'a [ParsedTicket]) -> Index<'a> {
        let mut by_slug: HashMap<&str, &ParsedTicket> = HashMap::new();
        let mut contested = std::collections::HashSet::new();
        let mut children: HashMap<&str, Vec<&ParsedTicket>> = HashMap::new();
        let mut dependents: HashMap<&str, Vec<&ParsedTicket>> = HashMap::new();
        let mut backlinks: HashMap<&str, Vec<&ParsedTicket>> = HashMap::new();

        for t in tickets {
            if t.id.is_empty() {
                continue;
            }
            if by_slug.insert(t.id.as_str(), t).is_some() {
                contested.insert(t.id.as_str());
            }
        }
        for t in tickets {
            if let Some(parent) = &t.parent {
                children.entry(parent.as_str()).or_default().push(t);
            }
            for dep in &t.depends_on {
                dependents.entry(dep.as_str()).or_default().push(t);
            }
            for link in &t.links {
                backlinks.entry(link.as_str()).or_default().push(t);
            }
        }

        Index {
            by_slug,
            contested,
            children,
            dependents,
            backlinks,
            status_of: board::trunk_status_map(tickets),
        }
    }

    fn knows(&self, slug: &str) -> bool {
        self.by_slug.contains_key(slug)
    }
}

// ---------------------------------------------------------------------------
// Pages
// ---------------------------------------------------------------------------

fn page_board(ctx: &Context, snap: &Snapshot, index: &Index) -> String {
    let in_flight: HashMap<&str, &str> = snap
        .branches
        .iter()
        .filter_map(|b| b.status.status().map(|s| (b.slug.as_str(), s)))
        .collect();

    let mut body = String::new();
    body.push_str(&format!(
        "<p class=\"source\">{}</p>",
        escape(&board::source_status_line(ctx.ref_.as_deref()).replace("# ", ""))
    ));

    if snap.tickets.len() < snap.ticket_files {
        body.push_str(&format!(
            "<p class=\"warn\">planr read {} of the {} ticket file(s) under \
             '{}' -- what it could not read is missing from this board.</p>",
            snap.tickets.len(),
            snap.ticket_files,
            escape(&ctx.plan_dir)
        ));
    }

    // In flight leads the board. It is the only section that answers "what is
    // anyone doing right now"; the three backlog tables answer "what exists",
    // which is the slower question and reads fine below the fold.
    if !snap.branches.is_empty() {
        body.push_str("<h2>in flight <span class=\"n\">");
        body.push_str(&snap.branches.len().to_string());
        body.push_str("</span></h2>");
        body.push_str(
            "<table><thead><tr><th>branch</th><th>status</th><th>task</th>\
                       </tr></thead><tbody>",
        );
        for b in &snap.branches {
            body.push_str(&format!(
                "<tr><td><code>{}</code></td><td>{}</td><td>{}</td></tr>",
                escape(&b.branch),
                status_badge(b.status.display()),
                slug_link(&b.slug, index)
            ));
        }
        body.push_str("</tbody></table>");
    }

    for (label, kind) in [
        ("epics", Kind::Epic),
        ("stories", Kind::Story),
        ("tasks", Kind::Task),
    ] {
        let rows: Vec<&ParsedTicket> = snap
            .tickets
            .iter()
            .filter(|t| t.kind == Some(kind.clone()))
            .collect();
        if rows.is_empty() {
            continue;
        }
        let is_tasks = kind == Kind::Task;
        body.push_str(&format!(
            "<h2>{label} <span class=\"n\">{}</span></h2>",
            rows.len()
        ));
        body.push_str("<table><thead><tr><th>id</th><th>status</th><th>parent</th>");
        if is_tasks {
            body.push_str("<th>blocked by</th>");
        }
        body.push_str("<th>title</th></tr></thead><tbody>");
        for t in rows {
            let (status, from_branch) = if is_tasks {
                board::task_status_display(t, &in_flight)
            } else {
                (t.status.clone(), false)
            };
            body.push_str("<tr>");
            body.push_str(&format!("<td>{}</td>", slug_link(&t.id, index)));
            body.push_str(&format!(
                "<td>{}{}</td>",
                status_badge(status.trim_end_matches(" *")),
                if from_branch {
                    " <abbr title=\"from an in-flight branch; trunk still records \
                     the pre-claim value\">*</abbr>"
                } else {
                    ""
                }
            ));
            body.push_str(&format!(
                "<td>{}</td>",
                match &t.parent {
                    Some(p) => slug_link(p, index),
                    None => "<span class=\"none\">-</span>".to_string(),
                }
            ));
            if is_tasks {
                let blocked = board::blocked_by(t, &index.status_of);
                body.push_str(&format!(
                    "<td>{}</td>",
                    if blocked.is_empty() {
                        "<span class=\"none\">-</span>".to_string()
                    } else {
                        slug_links(blocked.split(' '), index)
                    }
                ));
            }
            body.push_str(&format!("<td>{}</td>", escape(&t.title)));
            body.push_str("</tr>");
        }
        body.push_str("</tbody></table>");
    }

    layout("board", None, &body)
}

fn page_ticket(t: &ParsedTicket, snap: &Snapshot, index: &Index) -> String {
    let mut body = String::new();

    if let Some(err) = &t.frontmatter_error {
        body.push_str(&format!(
            "<p class=\"warn\">frontmatter did not parse, so every field below is \
             missing rather than absent: {}</p>",
            escape(err)
        ));
    }
    if index.contested.contains(t.id.as_str()) {
        body.push_str(
            "<p class=\"warn\">more than one file claims this slug -- which one this \
             page shows is arbitrary, and the board counts neither. Run \
             <code>planr lint</code>.</p>",
        );
    }
    if t.id_from_filename {
        body.push_str(
            "<p class=\"warn\">this ticket declares no <code>id</code>; the slug above \
             is taken from its filename.</p>",
        );
    }

    body.push_str("<dl class=\"meta\">");
    body.push_str(&format!("<dt>kind</dt><dd>{}</dd>", kind_name(&t.kind)));
    // One badge, always. The ticket file is what this page is about, and a
    // second pill beside it reads as the same field rendered twice rather
    // than as two sources disagreeing. The branch's value is a footnote to
    // the badge, in prose, naming the branch that carries it.
    let branch = snap.branches.iter().find(|b| b.slug == t.id);
    body.push_str(&format!(
        "<dt>status</dt><dd>{}{}</dd>",
        status_badge(&t.status),
        match branch {
            Some(b) if b.status.status() != Some(t.status.as_str()) => format!(
                " <span class=\"note\"><code>{}</code> reports \
                 <span class=\"branch-st\">{}</span></span>",
                escape(&b.branch),
                escape(b.status.display())
            ),
            _ => String::new(),
        }
    ));
    body.push_str(&format!(
        "<dt>parent</dt><dd>{}</dd>",
        match &t.parent {
            Some(p) => slug_link(p, index),
            None => "<span class=\"none\">-</span>".to_string(),
        }
    ));
    body.push_str(&format!(
        "<dt>depends on</dt><dd>{}</dd>",
        list_or_dash(t.depends_on.iter().map(|s| s.as_str()), index)
    ));
    if let Some(file) = &t.source_file {
        body.push_str(&format!(
            "<dt>file</dt><dd><code>{}</code></dd>",
            escape(file)
        ));
    }
    body.push_str("</dl>");

    // The three reverse edges. These are the whole reason to open this page
    // rather than the file: none of them is written down anywhere in the
    // backlog, and the CLI answers for none of them either.
    for (label, hint, group) in [
        (
            "children",
            "tickets naming this one as their parent",
            index.children.get(t.id.as_str()),
        ),
        (
            "dependents",
            "tickets that cannot start until this one is done",
            index.dependents.get(t.id.as_str()),
        ),
        (
            "backlinks",
            "tickets whose body mentions this one",
            index.backlinks.get(t.id.as_str()),
        ),
    ] {
        let items = match group {
            Some(v) if !v.is_empty() => v,
            _ => continue,
        };
        body.push_str(&format!(
            "<h2>{label} <span class=\"n\">{}</span> \
             <span class=\"hint\">{hint}</span></h2><ul class=\"rel\">",
            items.len()
        ));
        for other in items {
            body.push_str(&format!(
                "<li>{} {} <span class=\"title\">{}</span></li>",
                status_badge(&other.status),
                slug_link(&other.id, index),
                escape(&other.title)
            ));
        }
        body.push_str("</ul>");
    }

    body.push_str("<h2>body</h2><div class=\"md\">");
    body.push_str(&markdown(&t.raw, index));
    body.push_str("</div>");

    layout(&t.id, Some(&t.title), &body)
}

/// A slug nothing in the backlog answers for.
///
/// Every dangling wiki-link points here rather than at a bare 404, because
/// the useful question is not "does this exist" -- the link already says it
/// does not -- but "who thinks it does".
fn page_missing(slug: &str, index: &Index) -> String {
    let mut body = format!(
        "<p class=\"warn\">No ticket in this backlog claims the slug \
         <code>{}</code>.</p>",
        escape(slug)
    );

    let referrers: Vec<&&ParsedTicket> = index
        .backlinks
        .get(slug)
        .into_iter()
        .chain(index.dependents.get(slug))
        .chain(index.children.get(slug))
        .flatten()
        .collect();
    if referrers.is_empty() {
        body.push_str("<p>Nothing refers to it either.</p>");
    } else {
        body.push_str("<h2>referred to by</h2><ul class=\"rel\">");
        let mut seen = std::collections::HashSet::new();
        for other in referrers {
            if !seen.insert(other.id.as_str()) {
                continue;
            }
            body.push_str(&format!(
                "<li>{} {} <span class=\"title\">{}</span></li>",
                status_badge(&other.status),
                slug_link(&other.id, index),
                escape(&other.title)
            ));
        }
        body.push_str("</ul>");
    }

    let near: Vec<&str> = index
        .by_slug
        .keys()
        .filter(|k| k.contains(slug) || slug.contains(**k))
        .copied()
        .collect();
    if !near.is_empty() {
        body.push_str("<h2>did you mean</h2><ul class=\"rel\">");
        for k in near {
            body.push_str(&format!("<li>{}</li>", slug_link(k, index)));
        }
        body.push_str("</ul>");
    }

    layout(slug, Some("no such ticket"), &body)
}

fn page_lint(ctx: &Context) -> String {
    let report = match &ctx.ref_ {
        Some(r) => lint::lint_ref(r, &ctx.plan_dir),
        None => lint::lint_working_tree(&ctx.plan_dir),
    };

    let mut body = format!(
        "<p class=\"source\">{} error(s), {} warning(s) over {} of {} ticket file(s)</p>",
        report.error_count, report.warning_count, report.tickets_read, report.ticket_files
    );
    if report.issues.is_empty() {
        body.push_str("<p>Nothing to report.</p>");
        return layout("lint", None, &body);
    }

    body.push_str(
        "<table><thead><tr><th>level</th><th>ticket</th><th>finding</th>\
                   </tr></thead><tbody>",
    );
    for issue in &report.issues {
        let slug = crate::ticket::slug_from_filename(&issue.file);
        body.push_str(&format!(
            "<tr class=\"{}\"><td>{}</td><td><a href=\"/t/{}\">{}</a></td><td>{}</td></tr>",
            issue.level,
            issue.level,
            percent_encode(&slug),
            escape(&slug),
            escape(&issue.message)
        ));
    }
    body.push_str("</tbody></table>");
    layout("lint", None, &body)
}

fn page_not_found(path: &str) -> String {
    layout(
        "not found",
        None,
        &format!(
            "<p class=\"warn\">No route for <code>{}</code>.</p>",
            escape(path)
        ),
    )
}

// ---------------------------------------------------------------------------
// Markdown
// ---------------------------------------------------------------------------

/// Render a ticket body, with its wiki-links turned into navigation.
///
/// Raw HTML in the source is dropped rather than passed through. A backlog is
/// ordinary repository content, and `serve <ref>` is a reasonable way to read
/// a branch someone else wrote -- which makes a `<script>` in a ticket body a
/// way to run code in the reader's browser. Nothing in a ticket needs raw
/// HTML. Do not re-enable it.
fn markdown(body: &str, index: &Index) -> String {
    let linked = link_wiki_links(body, index);

    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    // Acceptance sections are checklists; without this they render as literal
    // "[ ]" text, which is the least readable part of every ticket.
    opts.insert(Options::ENABLE_TASKLISTS);

    let parser = Parser::new_ext(&linked, opts)
        .filter(|ev| !matches!(ev, Event::Html(_) | Event::InlineHtml(_)));

    let mut out = String::new();
    html::push_html(&mut out, parser);
    out
}

/// Rewrite `[[slug]]` in place as a markdown link.
///
/// Known slugs go to `/t/<slug>` and unknown ones to `/dangling/<slug>`, which
/// is what lets the stylesheet mark a broken link without this having to emit
/// HTML of its own: the route is the signal. The scan comes from
/// `parse::wiki_links`, the same one lint reports dangling links from, so the
/// page and `planr lint` cannot disagree about which links are broken.
fn link_wiki_links(body: &str, index: &Index) -> String {
    let mut out = String::with_capacity(body.len());
    let mut last = 0;
    for link in crate::parse::wiki_links(body) {
        out.push_str(&body[last..link.range.start]);
        let route = if index.knows(&link.slug) {
            "t"
        } else {
            "dangling"
        };
        out.push_str(&format!(
            "[{}](/{route}/{})",
            escape_md(&link.label),
            percent_encode(&link.slug)
        ));
        last = link.range.end;
    }
    out.push_str(&body[last..]);
    out
}

// ---------------------------------------------------------------------------
// HTML helpers
// ---------------------------------------------------------------------------

fn layout(title: &str, subtitle: Option<&str>, body: &str) -> String {
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
         <title>{} -- planr</title><style>{STYLE}</style></head><body>\
         <header><a class=\"home\" href=\"/\">planr</a>\
         <h1>{}{}</h1><nav><a href=\"/\">board</a><a href=\"/lint\">lint</a></nav>\
         </header><main>{body}</main></body></html>",
        escape(title),
        escape(title),
        match subtitle {
            Some(s) if !s.is_empty() => format!(" <span class=\"sub\">{}</span>", escape(s)),
            _ => String::new(),
        }
    )
}

fn slug_link(slug: &str, index: &Index) -> String {
    let route = if index.knows(slug) { "t" } else { "dangling" };
    format!(
        "<a class=\"slug\" href=\"/{route}/{}\">{}</a>",
        percent_encode(slug),
        escape(slug)
    )
}

fn slug_links<'s>(slugs: impl Iterator<Item = &'s str>, index: &Index) -> String {
    slugs
        .filter(|s| !s.is_empty())
        .map(|s| slug_link(s, index))
        .collect::<Vec<_>>()
        .join(" ")
}

fn list_or_dash<'s>(slugs: impl Iterator<Item = &'s str>, index: &Index) -> String {
    let rendered = slug_links(slugs, index);
    if rendered.is_empty() {
        "<span class=\"none\">-</span>".to_string()
    } else {
        rendered
    }
}

fn status_badge(status: &str) -> String {
    let class = match status {
        "todo" | "in_progress" | "review" | "done" | "blocked" | "abandoned" => status,
        _ => "unknown",
    };
    format!("<span class=\"st st-{class}\">{}</span>", escape(status))
}

fn kind_name(kind: &Option<Kind>) -> &'static str {
    match kind {
        Some(Kind::Epic) => "epic",
        Some(Kind::Story) => "story",
        Some(Kind::Task) => "task",
        None => "<span class=\"none\">-</span>",
    }
}

fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// Escape the characters that would end a markdown link label early.
fn escape_md(s: &str) -> String {
    s.replace('\\', r"\\")
        .replace('[', r"\[")
        .replace(']', r"\]")
}

/// Percent-encode everything outside the unreserved set.
///
/// Slugs are kebab-case by convention and need none of this, but a slug is
/// whatever an author typed, and one carrying a `?` or a space would
/// otherwise build a URL that addresses something else.
fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Decode the percent-escapes a browser put in the path.
///
/// Invalid escapes and non-UTF-8 results are left as they arrived: this feeds
/// a map lookup that will simply miss, and a slug is not worth refusing a
/// request over.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
            if let Some(v) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

// ---------------------------------------------------------------------------
// Style
// ---------------------------------------------------------------------------

/// The whole stylesheet, inlined into every page: no CDN, no second request.
///
/// Load-bearing ordering: `a[href^='/dangling/']` has the same specificity as
/// `.md a`, so it only wins by coming after it. Keep the dangling rules last
/// among the link rules, or a broken wiki-link in a body renders in the live
/// link's color and stops reading as broken.
const STYLE: &str = "\
:root{--bg:#fff;--fg:#1a1a1a;--dim:#6b7280;--line:#e5e7eb;--accent:#1d4ed8;\
--warn:#b45309;--warnbg:#fffbeb;--err:#b91c1c;--code:#f6f7f9}\
@media(prefers-color-scheme:dark){:root{--bg:#111317;--fg:#e6e7e9;--dim:#9aa1ac;\
--line:#272b32;--accent:#7aa2f7;--warn:#e0a33a;--warnbg:#2a2416;--err:#f07178;\
--code:#191c22}}\
*{box-sizing:border-box}\
body{margin:0;background:var(--bg);color:var(--fg);\
font:14px/1.55 ui-sans-serif,system-ui,-apple-system,Segoe UI,sans-serif}\
header{border-bottom:1px solid var(--line);padding:14px 20px;display:flex;\
align-items:baseline;gap:14px;flex-wrap:wrap;position:sticky;top:0;\
background:var(--bg);z-index:1}\
.home{font-weight:700;color:var(--fg);text-decoration:none;letter-spacing:.02em}\
header h1{font-size:17px;margin:0;font-weight:600}\
.sub{font-weight:400;color:var(--dim)}\
nav{margin-left:auto;display:flex;gap:14px}\
nav a{color:var(--dim);text-decoration:none}nav a:hover{color:var(--accent)}\
main{padding:20px;max-width:1100px}\
h2{font-size:13px;text-transform:uppercase;letter-spacing:.06em;color:var(--dim);\
margin:26px 0 8px;font-weight:600}\
h2 .n{color:var(--fg);opacity:.6}\
h2 .hint{text-transform:none;letter-spacing:0;font-weight:400;opacity:.75}\
table{border-collapse:collapse;width:100%;display:block;overflow-x:auto}\
th{text-align:left;font-size:11px;text-transform:uppercase;letter-spacing:.05em;\
color:var(--dim);font-weight:600;padding:5px 10px 5px 0;border-bottom:1px solid var(--line)}\
td{padding:5px 10px 5px 0;border-bottom:1px solid var(--line);vertical-align:top}\
tr.error td{background:color-mix(in srgb,var(--err) 8%,transparent)}\
a.slug{font-family:ui-monospace,SFMono-Regular,Menlo,monospace;font-size:13px;\
color:var(--accent);text-decoration:none}\
a.slug:hover{text-decoration:underline}\
.md a{color:var(--accent);text-decoration:underline;text-underline-offset:2px;\
text-decoration-color:color-mix(in srgb,var(--accent) 45%,transparent)}\
.md a:hover{text-decoration-color:currentColor}\
a[href^='/dangling/']{color:var(--err);text-decoration:underline wavy}\
a[href^='/dangling/']:after{content:'?';vertical-align:super;font-size:9px}\
.none{color:var(--dim)}\
.st{display:inline-block;font-size:11px;padding:1px 7px;border-radius:9px;\
border:1px solid var(--line);white-space:nowrap}\
.st-done{color:#15803d;border-color:#15803d66}\
.st-in_progress{color:#1d4ed8;border-color:#1d4ed866}\
.st-review{color:#7c3aed;border-color:#7c3aed66}\
.st-blocked,.st-unknown{color:var(--err);border-color:currentColor}\
.st-abandoned{color:var(--dim);text-decoration:line-through}\
.source{color:var(--dim);font-family:ui-monospace,monospace;font-size:12px;margin:0}\
.warn{background:var(--warnbg);border-left:3px solid var(--warn);padding:8px 12px;\
margin:12px 0}\
.note,.hint{color:var(--dim);font-size:12px}\
.branch-st{font-family:ui-monospace,monospace;color:var(--fg)}\
dl.meta{display:grid;grid-template-columns:max-content 1fr;gap:4px 16px;margin:14px 0}\
dl.meta dt{color:var(--dim);font-size:12px;text-transform:uppercase;\
letter-spacing:.05em}\
dl.meta dd{margin:0}\
ul.rel{list-style:none;padding:0;margin:0}\
ul.rel li{padding:3px 0;border-bottom:1px solid var(--line);display:flex;gap:10px;\
align-items:baseline}\
ul.rel .title{color:var(--dim)}\
.md{max-width:70ch}\
.md h1,.md h2,.md h3{text-transform:none;letter-spacing:0;color:var(--fg);\
font-size:15px;margin:20px 0 6px}\
.md code{background:var(--code);padding:1px 5px;border-radius:3px;\
font-family:ui-monospace,monospace;font-size:12.5px}\
.md pre{background:var(--code);padding:12px;border-radius:5px;overflow-x:auto}\
.md pre code{background:none;padding:0}\
.md blockquote{margin:0;padding-left:12px;border-left:3px solid var(--line);\
color:var(--dim)}\
.md ul{padding-left:20px}\
.md li input[type=checkbox]{margin-right:6px}\
.md table{display:table}\
";

#[cfg(test)]
mod tests {
    use super::STYLE;

    /// `.md a` and `a[href^='/dangling/']` tie on specificity, so source order
    /// decides which one colors a broken wiki-link in a ticket body.
    #[test]
    fn test_dangling_link_rule_follows_the_body_link_rule() {
        let body = STYLE.find(".md a{").expect("no body link rule");
        let dangling = STYLE
            .find("a[href^='/dangling/']{")
            .expect("no dangling link rule");
        assert!(
            dangling > body,
            "the dangling rule must come after `.md a` to win the tie"
        );
    }
}
