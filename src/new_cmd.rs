//! `planr new` -- create a new ticket from an embedded template.
//!
//! Guards, template substitution, and flock-serialised prefix allocation.
//! Port of `skills/planr/src/new-ticket.ts`.

use crate::lint;
use crate::lock::PlanrLock;

// ---------------------------------------------------------------------------
// Templates (embedded)
// ---------------------------------------------------------------------------

const TEPIC: &str = include_str!("../templates/epic.md");
const TSTORY: &str = include_str!("../templates/story.md");
const TTASK: &str = include_str!("../templates/task.md");

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const VALID_KINDS: [&str; 3] = ["epic", "story", "task"];
const SLUG_RE: &str = r"^[a-z0-9]+(-[a-z0-9]+)*$";

// ---------------------------------------------------------------------------
// Slug validation
// ---------------------------------------------------------------------------

pub fn validate_slug(slug: &str) -> bool {
    let re = regex::Regex::new(SLUG_RE).unwrap();
    re.is_match(slug)
}

// ---------------------------------------------------------------------------
// Kind helpers
// ---------------------------------------------------------------------------

pub fn kind_to_subdir(kind: &str) -> Result<&'static str, String> {
    match kind {
        "epic" => Ok("epics"),
        "story" => Ok("stories"),
        "task" => Ok("tasks"),
        _ => Err(format!("unknown kind: {kind} (want epic|story|task)")),
    }
}

pub fn is_valid_kind(kind: &str) -> bool {
    VALID_KINDS.contains(&kind)
}

/// Fetch the embedded template content for a given kind.
fn get_template(kind: &str) -> Result<&'static str, String> {
    match kind {
        "epic" => Ok(TEPIC),
        "story" => Ok(TSTORY),
        "task" => Ok(TTASK),
        _ => Err(format!("unknown kind: {kind} (want epic|story|task)")),
    }
}

// ---------------------------------------------------------------------------
// Parent existence
// ---------------------------------------------------------------------------

pub fn parent_exists(parent: &str, plan_dir: &str) -> bool {
    for kd in &["epics", "stories", "tasks"] {
        let dir_path = std::path::Path::new(plan_dir).join(kd);
        if !dir_path.exists() {
            continue;
        }
        let entries = match std::fs::read_dir(&dir_path) {
            Ok(rd) => rd,
            Err(_) => continue,
        };
        let pattern = format!(r"^\d+-{}\.md$", regex::escape(parent));
        let re = match regex::Regex::new(&pattern) {
            Ok(r) => r,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if re.is_match(name) {
                    return true;
                }
            }
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Prefix allocation (under exclusive lock)
// ---------------------------------------------------------------------------

/// Allocate the next sort-hint prefix and write the ticket file.
///
/// This function is called while holding an exclusive `PlanrLock` -- the
/// kernel-level flock ensures concurrent `planr new` invocations serialize.
fn allocate_and_write(dir: &std::path::Path, slug: &str, content: &str) -> Result<String, String> {
    // Read highest existing prefix
    let mut highest: u32 = 0;
    if let Ok(rd) = std::fs::read_dir(dir) {
        for entry in rd.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if let Some(rest) = name.strip_suffix(".md") {
                    if let Some(dash_pos) = rest.find('-') {
                        let prefix_str = &rest[..dash_pos];
                        if let Ok(n) = prefix_str.parse::<u32>() {
                            if n > highest {
                                highest = n;
                            }
                        }
                    }
                }
            }
        }
    }

    let nn = format!("{:02}", highest + 1);
    let filename = format!("{nn}-{slug}.md");
    let path = dir.join(&filename);

    // Refuse if target file exists
    if path.exists() {
        return Err(format!("already exists: {}", path.display()));
    }

    // Write
    std::fs::write(&path, content).map_err(|e| format!("write error: {e}"))?;

    // Post-write verification: exactly one NN-* file must exist
    let count = if let Ok(rd) = std::fs::read_dir(dir) {
        rd.flatten()
            .filter(|e| {
                e.file_name()
                    .to_str()
                    .is_some_and(|n| n.starts_with(&format!("{nn}-")))
            })
            .count()
    } else {
        0
    };
    if count != 1 {
        return Err(format!(
            "internal error: prefix {nn} is shared by {count} files in {} after creating {}",
            dir.display(),
            path.display()
        ));
    }

    Ok(filename)
}

// ---------------------------------------------------------------------------
// Template substitution
// ---------------------------------------------------------------------------

/// Render a string as a YAML scalar safe to sit after `key: ` on one line.
///
/// serde_yaml picks the minimal representation -- plain when the value is
/// unambiguous, quoted when a colon, a leading indicator character, or a
/// trailing space would otherwise break the parser. Titles of the shape
/// `Foo: bar and baz` are common, and an unquoted one makes the whole
/// frontmatter block unparseable to planr's own reader.
fn yaml_scalar(value: &str) -> String {
    match serde_yaml::to_string(&value) {
        Ok(s) => s.trim_end_matches('\n').to_string(),
        // serde_yaml does not fail on a plain string; quote defensively anyway.
        Err(_) => format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\"")),
    }
}

fn substitute_template(
    template: &str,
    slug: &str,
    title: &str,
    parent: Option<&str>,
    date: &str,
) -> String {
    // Quote the frontmatter title first: the body's `__TITLE__` under `## Goal`
    // is prose and must stay raw, so only the `title:` line gets the scalar.
    template
        .replace(
            "title: __TITLE__",
            &format!("title: {}", yaml_scalar(title)),
        )
        .replace("__SLUG__", slug)
        .replace("__TITLE__", title)
        .replace("__PARENT__", parent.unwrap_or(""))
        .replace("__DATE__", date)
}

/// Compute a UTC YYYY-MM-DD date string from the current system time.
fn utc_date_string() -> String {
    use std::time::SystemTime;
    let now = SystemTime::now();
    let dur = now
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let total_secs = dur.as_secs();

    // Days since epoch
    // Use i64 throughout for the date algorithm (negatives needed for months)
    let days = total_secs as i64 / 86400;
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let d = doy - (153 * mp + 2) / 5 + 1;

    format!("{:04}-{:02}-{:02}", y, m, d)
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Create a new ticket -- validates, allocates prefix under exclusive lock,
/// writes the file, and returns the relative path.
///
/// On success, the caller should:
/// 1. Print the relative path on stdout.
/// 2. Run lint over the working tree and write findings to stderr.
pub fn create_ticket(
    kind: &str,
    slug: &str,
    title: &str,
    parent: Option<&str>,
    plan_dir: &str,
) -> Result<String, String> {
    // 1. Validate kind
    if !is_valid_kind(kind) {
        return Err(format!("unknown kind: {kind} (want epic|story|task)"));
    }

    // 2. Validate slug
    if !validate_slug(slug) {
        return Err(format!(
            "bad slug '{slug}': want kebab-case (lowercase alphanumerics, \
             single hyphens between segments, starting with [a-z0-9])"
        ));
    }

    // 3. Parent required for story and task
    if kind != "epic" && parent.is_none() {
        return Err(format!("parent slug required for {kind}"));
    }

    // 4. Parent must exist (skip for epics)
    if let Some(p) = parent {
        if !parent_exists(p, plan_dir) {
            return Err(format!(
                "parent '{p}' not found under {plan_dir}/ -- create the parent first"
            ));
        }
    }

    // 5. Determine subdirectory
    let subdir = kind_to_subdir(kind)?;
    let dir = std::path::Path::new(plan_dir).join(subdir);
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("cannot create directory {}: {e}", dir.display()))?;

    // 6. Load template
    let template = get_template(kind)?;

    // 7. Compute date (UTC YYYY-MM-DD -- matches TS toISOString() split)
    let today = utc_date_string();

    // 8. Substitute
    let content = substitute_template(template, slug, title, parent, &today);

    // 9. Exclusive lock -> allocate prefix -> write -> verify
    let lock =
        PlanrLock::exclusive(std::path::Path::new(".")).map_err(|e| format!("lock error: {e}"))?;
    let result = allocate_and_write(&dir, slug, &content);
    drop(lock); // Release lock -- the allocate+write critical section is done

    let filename = result?;

    // Return relative path
    let relative = format!("{plan_dir}/{subdir}/{filename}");
    Ok(relative)
}

/// Run lint on the working tree (in-process) and return findings for stderr.
pub fn lint_findings(plan_dir: &str) -> String {
    let report = lint::lint_working_tree(plan_dir);
    lint::render_report(&report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    // ---- slug validation ----

    #[test]
    fn test_accepts_simple_kebab_case() {
        assert!(validate_slug("http-proxy"));
    }

    #[test]
    fn test_accepts_single_segment() {
        assert!(validate_slug("lint"));
    }

    #[test]
    fn test_accepts_digits() {
        assert!(validate_slug("v1-ship"));
    }

    #[test]
    fn test_rejects_leading_hyphen() {
        assert!(!validate_slug("-bad"));
    }

    #[test]
    fn test_rejects_trailing_hyphen() {
        assert!(!validate_slug("bad-"));
    }

    #[test]
    fn test_rejects_double_hyphen() {
        assert!(!validate_slug("bad--slug"));
    }

    #[test]
    fn test_rejects_uppercase() {
        assert!(!validate_slug("Bad-Slug"));
    }

    #[test]
    fn test_rejects_empty() {
        assert!(!validate_slug(""));
    }

    #[test]
    fn test_rejects_special_chars() {
        assert!(!validate_slug("slug_with_underscore"));
    }

    // ---- kind helpers ----

    #[test]
    fn test_kind_to_subdir_epic() {
        assert_eq!(kind_to_subdir("epic").unwrap(), "epics");
    }

    #[test]
    fn test_kind_to_subdir_story() {
        assert_eq!(kind_to_subdir("story").unwrap(), "stories");
    }

    #[test]
    fn test_kind_to_subdir_task() {
        assert_eq!(kind_to_subdir("task").unwrap(), "tasks");
    }

    #[test]
    fn test_kind_to_subdir_unknown() {
        assert!(kind_to_subdir("foo").is_err());
    }

    #[test]
    fn test_is_valid_kind_accepts_all() {
        assert!(is_valid_kind("epic"));
        assert!(is_valid_kind("story"));
        assert!(is_valid_kind("task"));
    }

    #[test]
    fn test_is_valid_kind_rejects_invalid() {
        assert!(!is_valid_kind("foo"));
    }

    // ---- parent exists (in isolated temp dir) ----

    fn setup_temp_plan() -> tempfile::TempDir {
        let td = tempfile::tempdir().unwrap();
        for kd in &["epics", "stories", "tasks"] {
            fs::create_dir_all(td.path().join(kd)).unwrap();
        }
        td
    }

    fn write_ticket(dir: &Path, prefix: &str, slug: &str) {
        let content = format!("---\nid: {slug}\n---\n");
        fs::write(dir.join(format!("{prefix}-{slug}.md")), content).unwrap();
    }

    #[test]
    fn test_parent_exists_found_in_epics() {
        let td = setup_temp_plan();
        write_ticket(&td.path().join("epics"), "01", "parent-epic");
        assert!(parent_exists("parent-epic", td.path().to_str().unwrap()));
    }

    #[test]
    fn test_parent_exists_found_in_stories() {
        let td = setup_temp_plan();
        write_ticket(&td.path().join("stories"), "02", "parent-story");
        assert!(parent_exists("parent-story", td.path().to_str().unwrap()));
    }

    #[test]
    fn test_parent_exists_found_in_tasks() {
        let td = setup_temp_plan();
        write_ticket(&td.path().join("tasks"), "03", "parent-task");
        assert!(parent_exists("parent-task", td.path().to_str().unwrap()));
    }

    #[test]
    fn test_parent_exists_not_found() {
        let td = setup_temp_plan();
        assert!(!parent_exists("nonexistent", td.path().to_str().unwrap()));
    }

    // ---- template substitution ----

    #[test]
    fn test_substitute_template() {
        let tpl = "id: __SLUG__\ntitle: __TITLE__\nparent: __PARENT__\ndate: __DATE__\n";
        let result = substitute_template(
            tpl,
            "my-slug",
            "My Title",
            Some("parent-epic"),
            "2026-08-05",
        );
        assert!(result.contains("id: my-slug"));
        assert!(result.contains("title: My Title"));
        assert!(result.contains("parent: parent-epic"));
        assert!(result.contains("date: 2026-08-05"));
    }

    #[test]
    fn test_substitute_template_quotes_colon_title() {
        // Issue #1: an unquoted colon-bearing title made planr's own reader
        // fail on the file planr had just written.
        let tpl = "id: __SLUG__\ntitle: __TITLE__\n";
        let result = substitute_template(
            tpl,
            "shadow-remote",
            "The shadow remote: git-native sync",
            None,
            "2026-08-25",
        );
        assert!(
            result.contains("title: 'The shadow remote: git-native sync'"),
            "expected a quoted title, got: {result}"
        );
    }

    #[test]
    fn test_substitute_template_body_title_stays_raw() {
        // Only the frontmatter `title:` is a YAML scalar; the `## Goal` copy
        // is prose and must not pick up quotes.
        let tpl = "title: __TITLE__\n---\n\n## Goal\n\n__TITLE__\n";
        let result = substitute_template(tpl, "s", "A title: with a colon", None, "2026-08-25");
        assert!(result.contains("title: 'A title: with a colon'"));
        assert!(result.ends_with("## Goal\n\nA title: with a colon\n"));
    }

    #[test]
    fn test_scaffolded_ticket_round_trips_through_the_parser() {
        // The end-to-end invariant the issue was really about: whatever
        // `planr new` writes, planr must be able to read back.
        for title in [
            "Plain title",
            "Sanitary history: boundary rev, rewriter, range scan",
            "#hash leading",
            "- dash leading",
            "quotes \"inside\" it",
            "trailing space ",
            "123",
            "null",
        ] {
            let content =
                substitute_template(TTASK, "my-task", title, Some("my-story"), "2026-08-25");
            let t = crate::ticket::parse_ticket(&content);
            assert_eq!(t.frontmatter_error, None, "title {title:?}: {content}");
            assert_eq!(t.title, title, "title {title:?} did not round-trip");
            assert_eq!(t.id, "my-task");
            assert_eq!(t.kind, Some(crate::ticket::Kind::Task));
        }
    }

    #[test]
    fn test_substitute_template_no_parent() {
        let tpl = "parent: __PARENT__\n";
        let result = substitute_template(tpl, "slug", "Title", None, "2026-08-05");
        assert_eq!(result, "parent: \n");
    }

    // ---- prefix allocation ----

    #[test]
    fn test_allocate_and_write_first() {
        let td = tempfile::tempdir().unwrap();
        let path = allocate_and_write(td.path(), "my-task", "# content\n").unwrap();
        assert_eq!(path, "01-my-task.md");
        assert!(td.path().join("01-my-task.md").exists());
    }

    #[test]
    fn test_allocate_and_write_increments() {
        let td = tempfile::tempdir().unwrap();
        fs::write(td.path().join("01-first.md"), "").unwrap();
        let path = allocate_and_write(td.path(), "second", "").unwrap();
        assert_eq!(path, "02-second.md");
    }

    #[test]
    fn test_allocate_and_write_post_write_verification_ok() {
        // Normal case: single prefix 01 -> next write gets 02, unique
        let td = tempfile::tempdir().unwrap();
        fs::write(td.path().join("01-a.md"), "").unwrap();
        let result = allocate_and_write(td.path(), "second", "");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "02-second.md");
    }

    #[test]
    fn test_allocate_and_write_increments_same_slug() {
        // Calling with same slug twice gives different sequential prefixes
        let td = tempfile::tempdir().unwrap();
        let r1 = allocate_and_write(td.path(), "my-task", "");
        assert_eq!(r1.as_deref(), Ok("01-my-task.md"));
        let r2 = allocate_and_write(td.path(), "my-task", "");
        assert_eq!(r2.as_deref(), Ok("02-my-task.md"));
        // Both files exist
        assert!(td.path().join("01-my-task.md").exists());
        assert!(td.path().join("02-my-task.md").exists());
    }

    // ---- create_ticket guards ----

    #[test]
    fn test_create_ticket_bad_kind() {
        let td = setup_temp_plan();
        let r = create_ticket("foo", "slug", "Title", None, td.path().to_str().unwrap());
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("unknown kind"));
    }

    #[test]
    fn test_create_ticket_bad_slug() {
        let td = setup_temp_plan();
        let r = create_ticket(
            "task",
            "Bad-Slug",
            "Title",
            Some("parent"),
            td.path().to_str().unwrap(),
        );
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("bad slug"));
    }

    #[test]
    fn test_create_ticket_missing_parent() {
        let td = setup_temp_plan();
        let r = create_ticket(
            "task",
            "good-slug",
            "Title",
            None,
            td.path().to_str().unwrap(),
        );
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("parent slug required for task"));
    }

    #[test]
    fn test_create_ticket_parent_not_found() {
        let td = setup_temp_plan();
        let r = create_ticket(
            "task",
            "good-slug",
            "Title",
            Some("nonexistent"),
            td.path().to_str().unwrap(),
        );
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("parent 'nonexistent' not found"));
    }

    // ---- utc_date_string ----

    #[test]
    fn test_utc_date_string_format() {
        let s = utc_date_string();
        // Should match YYYY-MM-DD
        assert_eq!(s.len(), 10, "bad date format: {s}");
        assert_eq!(&s[4..5], "-");
        assert_eq!(&s[7..8], "-");
        let y: u32 = s[..4].parse().unwrap();
        let m: u32 = s[5..7].parse().unwrap();
        let d: u32 = s[8..].parse().unwrap();
        assert!((2025..=2030).contains(&y), "year out of range: {s}");
        assert!((1..=12).contains(&m), "month out of range: {s}");
        assert!((1..=31).contains(&d), "day out of range: {s}");
    }
}
