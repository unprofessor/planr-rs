//! Review brief generator -- reads a task from a `plan/<slug>` branch and
//! prints the information a reviewer needs.
//!
//! Port of `skills/planr/src/review.ts`.

use crate::git;
use crate::parse::{extract_section, extract_last_review_verdict};
use crate::ticket::parse_ticket;

/// Generate the full review brief for a task on a `plan/<slug>` branch.
pub fn generate_review_brief(slug: &str, trunk: &str, plan_dir: &str) -> Result<String, String> {
    let branch = format!("plan/{slug}");

    // Verify branch exists
    git::rev_parse_verify(&branch).map_err(|_| format!("no such branch: {branch}"))?;

    // Find the task file on the branch
    let task_files = git::ls_tree_md(&branch, &format!("{plan_dir}/tasks"))
        .map_err(|_| format!("no task file for '{slug}' on {branch}"))?;
    let task_pattern = format!(r"/[0-9]+-{}\.md$", regex::escape(slug));
    let re = regex::Regex::new(&task_pattern).unwrap();
    let task_file = task_files.iter().find(|f| re.is_match(f))
        .ok_or_else(|| format!("no task file for '{slug}' on {branch}"))?;

    // Locate worktree for this branch
    let worktree_path = find_worktree_path(&branch);

    // Read the task file from the branch
    let blob = git::show_ref(&branch, task_file)?;
    let ticket = parse_ticket(&blob);

    // Extract sections
    let acceptance = extract_section(&ticket.raw, "Acceptance");
    let validation_raw = extract_section(&ticket.raw, "Validation");
    let validation: Vec<&str> = validation_raw
        .lines()
        .filter(|l| !l.trim().is_empty())
        .collect();
    let validation = validation.join("\n");

    // Diff
    let diff = git::diff_refs(trunk, &branch)?;

    // Reviewer guidance (same text as TS review.sh / review.ts)
    let guidance = "--- reviewer guidance ---\n\
You are an independent reviewer in fresh context. Do NOT trust the worker's\n\
self-validation; re-check everything yourself.\n\
\n\
1. Read ## Acceptance above and the diff.\n\
2. In the worktree, RUN the acceptance checks yourself (tests, commands,\n\
   manual verification).\n\
3. Edit ONLY the task file (never code). Add a ## Review section:\n\
       ## Review\n\
       verdict: approved          # or: changes-requested\n\
       reviewer: <your id>\n\
       date: <YYYY-MM-DD>\n\
       <what you re-checked and the result>\n\
4. If approved: leave status: review, commit, hand back to the leader.\n\
5. If changes-requested: also flip status: in_progress, record concretely what\n\
   failed, commit, hand back. The worker will be re-dispatched.";

    // Build output
    let display_wt = worktree_path.as_deref().unwrap_or("(none -- checkout plan/<slug> to review)");
    let mut out = String::new();
    out.push_str(&format!("branch:    {branch}\n"));
    out.push_str(&format!("task:      {task_file}\n"));
    out.push_str(&format!("worktree:  {display_wt}\n"));
    out.push('\n');
    out.push_str("--- acceptance ---\n");
    out.push_str(&acceptance);
    out.push('\n');
    out.push('\n');
    out.push_str("--- validation (worker self-check) ---\n");
    out.push_str(&validation);
    out.push('\n');
    out.push('\n');
    out.push_str(&format!("--- diff vs {trunk} ---\n"));
    out.push_str(&diff);
    out.push('\n');
    out.push('\n');
    out.push_str(&guidance);
    out.push('\n');

    Ok(out)
}

/// Parse `worktree list --porcelain` to find the worktree path for a branch.
fn find_worktree_path(branch: &str) -> Option<String> {
    let lines = git::worktree_list().ok()?;
    let branch_ref = format!("refs/heads/{branch}");
    let mut current_wt: Option<String> = None;

    for line in &lines {
        if let Some(path) = line.strip_prefix("worktree ") {
            current_wt = Some(path.to_string());
        } else if line.strip_prefix("branch ") == Some(&branch_ref) {
            return current_wt;
        }
    }

    None
}
