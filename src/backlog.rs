//! One walk over `<plan-dir>/{epics,stories,tasks}`, for every reader of it.
//!
//! `board` and `lint` want the same thing -- every ticket file under the three
//! kind directories, either at a git ref or in the working tree. One walk, so
//! they cannot disagree about the extension test or about how many files were
//! found. Do not add a second.

/// Where a backlog read gets its tickets.
pub enum Source<'a> {
    /// A git ref: `ls-tree` for the listing, `show` for each blob.
    Ref(&'a str),
    /// The local working tree, relative to the current directory.
    WorkingTree,
}

/// One ticket file the walk found.
pub struct TicketFile {
    /// The path, as the source named it.
    pub file: String,
    /// The file's content, or `None` when it was found and would not read.
    ///
    /// This is the reason the walk returns a struct rather than a list of
    /// blobs. A reader that drops an unreadable file on the floor cannot
    /// distinguish a backlog that holds nothing from a backlog it opened
    /// nothing in, and it will report the second as the first. Keeping the
    /// entry with no content makes "found but unread" a state the caller has
    /// to look at. Do not turn this back into a skip.
    pub blob: Option<String>,
}

/// The kind directories a backlog is made of, in report order.
const KIND_DIRS: [&str; 3] = ["epics", "stories", "tasks"];

/// Every ticket file under the backlog's three kind directories.
///
/// Entries come back in kind order, and within a kind in the order the source
/// lists them -- sorted by path for the working tree, as `ls-tree` returns
/// them for a ref.
pub fn read_backlog(source: Source<'_>, plan_dir: &str) -> Vec<TicketFile> {
    let mut found = Vec::new();
    for kind in KIND_DIRS {
        let dir = format!("{plan_dir}/{kind}");
        match source {
            Source::Ref(ref_) => read_ref_dir(ref_, &dir, &mut found),
            Source::WorkingTree => read_working_tree_dir(&dir, &mut found),
        }
    }
    found
}

/// Collect one kind directory from a git ref.
fn read_ref_dir(ref_: &str, dir: &str, out: &mut Vec<TicketFile>) {
    // A kind directory that is not at this ref contributes nothing. That is
    // the ordinary shape of a backlog with no epics yet, not a failure to
    // read one -- there is no file here to report as unread.
    let Ok(files) = crate::git::ls_tree_md(ref_, dir) else {
        return;
    };
    for f in files {
        if !f.ends_with(".md") {
            continue;
        }
        // The listing named it, so the file is there. `None` records it
        // without content rather than dropping it, so the caller can say it
        // was found and not read. Do not turn this into a `continue`.
        let blob = crate::git::show_ref(ref_, &f).ok();
        out.push(TicketFile { file: f, blob });
    }
}

/// Collect one kind directory from the working tree.
fn read_working_tree_dir(dir: &str, out: &mut Vec<TicketFile>) {
    // `read_dir` fails on a directory that is not there, so a missing kind
    // directory needs no separate test ahead of this one.
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<std::path::PathBuf> =
        rd.filter_map(|e| e.ok()).map(|e| e.path()).collect();
    entries.sort();
    for entry in entries {
        if entry.extension().is_none_or(|e| e != "md") {
            continue;
        }
        if !entry.is_file() {
            continue;
        }
        // Same as the ref path: found, unread, and said so.
        let blob = std::fs::read_to_string(&entry).ok();
        out.push(TicketFile {
            file: entry.to_string_lossy().to_string(),
            blob,
        });
    }
}
