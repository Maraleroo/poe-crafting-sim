//! Acceptance tests for AB#289 "Umgang mit Dependabot-PRs".
//!
//! This story has no `crafting-core` behaviour. Following ADR 0004, only lean text-level
//! checks are made here: no YAML parser, no structural model of the workflow, no versions
//! or SHAs, so Dependabot updates of `review.yml` do not break the test. The interface is
//! the one named in the tech lead's plan ("Technischer Plan AB#289").
//!
//! Only AC4 has a permanent test. A job condition on the review job is a security
//! property of the workflow: a skipped job counts as a passed required check (ADR 0005).
//! - AC1, AC2 and AC3 only check the wording of `CLAUDE.md` and ADR 0005 (team files and
//!   documentation, ADR 0014). They are checked in the review against the criteria and
//!   the plan.
//! - AC4, part "`review.yml` bleibt unverändert", cannot be checked permanently, because
//!   Dependabot changes the file later. The review checks in the PR that
//!   `git diff origin/main...HEAD -- .github/workflows/review.yml` is empty.
//!
//! Red state: the property already holds before this story, so the test is green from
//! the start. PO decision on AB#289: the test designer proved locally that the test
//! detects a job condition. The line `    if: github.actor != 'dependabot[bot]'` was
//! inserted directly below `  review:` in `.github/workflows/review.yml`; the test then
//! failed with "the job `review` ... must not have a job-level condition, found line
//! 20: `    if: github.actor != 'dependabot[bot]'`". The change was discarded with
//! `git checkout -- .github/workflows/review.yml` and never committed.

use std::fs;
use std::path::{Path, PathBuf};

const REVIEW_WORKFLOW: &str = ".github/workflows/review.yml";
const REVIEW_JOB_LINE: &str = "  review:";

fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crafting-core must live inside the repository root")
}

fn repo_path(relative: &str) -> PathBuf {
    repo_root().join(relative)
}

fn read_text(relative: &str) -> String {
    let path = repo_path(relative);
    assert!(path.is_file(), "{relative} must exist");
    let bytes = fs::read(&path).unwrap_or_else(|e| panic!("cannot read {relative}: {e}"));
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Number of leading spaces of a line.
fn indentation(line: &str) -> usize {
    line.len() - line.trim_start_matches(' ').len()
}

/// Blank lines and comment lines never end a block.
fn is_blank_or_comment(line: &str) -> bool {
    let content = line.trim();
    content.is_empty() || content.starts_with('#')
}

/// The lines of the job block that starts at the line `  review:`, with their line
/// numbers (1-based). The block ends at the next line that is neither blank nor a comment
/// and is indented by two spaces or fewer (the next job or a top-level key), or at the end
/// of the file.
fn review_job_block(text: &str) -> Vec<(usize, &str)> {
    let lines: Vec<(usize, &str)> = text
        .lines()
        .map(|line| line.trim_end_matches('\r'))
        .enumerate()
        .map(|(index, line)| (index + 1, line))
        .collect();
    let start = lines
        .iter()
        .position(|(_, line)| line.trim_end() == REVIEW_JOB_LINE)
        .unwrap_or_else(|| {
            panic!("{REVIEW_WORKFLOW} must contain the job line `{REVIEW_JOB_LINE}`")
        });
    lines[start + 1..]
        .iter()
        .take_while(|(_, line)| is_blank_or_comment(line) || indentation(line) > 2)
        .copied()
        .collect()
}

// AC4: „`review.yml` bleibt unverändert, der Review-Job bekommt keine Job-Bedingung."
#[test]
fn ac4_review_job_has_no_job_condition() {
    // Given the review workflow
    let text = read_text(REVIEW_WORKFLOW);

    // When looking at the lines of the job `review` that are its direct keys
    // (exactly four spaces of indentation)
    let block = review_job_block(&text);
    assert!(
        !block.is_empty(),
        "the job `review` in {REVIEW_WORKFLOW} must not be empty"
    );
    let job_conditions: Vec<String> = block
        .iter()
        .filter(|(_, line)| indentation(line) == 4 && line.trim_start().starts_with("if:"))
        .map(|(number, line)| format!("line {number}: `{line}`"))
        .collect();

    // Then none of them is a job condition `if:`, so the job is never skipped.
    // Conditions of steps (deeper indentation) stay allowed.
    assert!(
        job_conditions.is_empty(),
        "the job `review` in {REVIEW_WORKFLOW} must not have a job-level condition, \
         because a skipped job passes a required check (ADR 0005), found {}",
        job_conditions.join(", ")
    );
}
