//! Acceptance tests for AB#100 "CI-Review zuverlässig und nachvollziehbar machen".
//!
//! This story has no `crafting-core` behaviour. Following ADR 0004, only lean
//! text-level checks are made here: a file contains or does not contain a given string,
//! a line with a given text exists. There are no YAML or Markdown parsers. The strings
//! are the interfaces named in the tech lead's plan ("Technischer Plan AB#100" and its
//! adjustment on the story).
//!
//! AC2 has two sentences, so it has two tests: one for the denied calls in the log, one
//! for keeping secrets out of it. The second test also checks the PO's addition from
//! the plan adjustment: the checkout does not store the workflow token in `.git/config`.
//!
//! What is checked outside `cargo test` (verification task AB#109 after the merge):
//! - AC1: the cause of the missing comment on PR #17 is recorded on the work item.
//! - AC2: that a real denied call appears in the job log and no secret does.
//! - AC3: that a real run posts exactly one comment. The red reasons for "no comment"
//!   and "more than one comment" cannot be triggered on purpose after the merge; the PO
//!   agreed that the text-level checks here count for them.
//! - AC4: that the run log shows an Opus model.
//! - AC5: the two test PRs for review criterion 8.
//!
//! Red state before the implementation: the tests for AC2 (denied calls), AC2 (secrets,
//! because of `persist-credentials: false`), AC3 and AC4 fail, because `review.yml` does
//! not contain the new texts yet.

use std::fs;
use std::path::{Path, PathBuf};

const REVIEW_WORKFLOW: &str = ".github/workflows/review.yml";
const REVIEWER_AGENT: &str = ".claude/agents/reviewer.md";

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

fn has_line(text: &str, expected: &str) -> bool {
    text.lines().any(|line| line.trim() == expected)
}

// AC2: „Wird im CI-Review ein Tool-Aufruf abgelehnt, zeigt das Log des Jobs, welcher
// Aufruf es war. Secrets erscheinen dabei nicht im Log."
// Part 1: the log shows the denied calls.
#[test]
fn ac2_job_log_shows_the_denied_tool_calls() {
    // Given the review workflow
    let text = read_text(REVIEW_WORKFLOW);

    // When looking for the step that reports denied tool calls
    let required = [
        // the execution file of the Claude step is read ...
        "execution_file",
        // ... for its list of denied calls ...
        "permission_denials",
        // ... and each run prints their number and the calls
        "Denied tool calls",
    ];

    // Then the workflow reads the denied calls and prints them to the job log
    for needle in required {
        assert!(
            text.contains(needle),
            "{REVIEW_WORKFLOW} must contain `{needle}`, so the job log shows which tool \
             call was denied"
        );
    }
}

// AC2: „Wird im CI-Review ein Tool-Aufruf abgelehnt, zeigt das Log des Jobs, welcher
// Aufruf es war. Secrets erscheinen dabei nicht im Log."
// Part 2: secrets stay out of the log; the checkout keeps no token in `.git/config`
// (PO addition in the plan adjustment).
#[test]
fn ac2_secrets_stay_out_of_the_job_log() {
    // Given the review workflow
    let text = read_text(REVIEW_WORKFLOW);

    // When looking for options that would put the full Claude output or tool results
    // into the log or into a downloadable file
    let forbidden = [
        "show_full_output: true",
        "display_report: true",
        "upload-artifact",
    ];

    // Then none of them is used
    for needle in forbidden {
        assert!(
            !text.contains(needle),
            "{REVIEW_WORKFLOW} must not contain `{needle}`: it would expose tool results \
             instead of only the denied calls"
        );
    }
    // And the checkout does not store the workflow token in .git/config
    assert!(
        has_line(&text, "persist-credentials: false"),
        "{REVIEW_WORKFLOW} must have a line `persist-credentials: false`, so the checkout \
         does not leave the workflow token readable for the reviewer"
    );
}

// AC3: „Das CI-Review postet bei jedem Durchlauf genau einen Kommentar mit seinem
// Ergebnis. Gelingt das nicht, nennt der rote Check den Grund."
#[test]
fn ac3_review_posts_exactly_one_comment_and_the_red_check_names_the_reason() {
    // Given the review workflow
    let text = read_text(REVIEW_WORKFLOW);

    // When looking at how the comment is posted and how the check status is set
    let posts_from_file = text.contains("--body-file");
    let names_missing_comment = text.contains("posted no review comment in this run");
    let names_too_many_comments = text.contains("expected exactly one");

    // Then the comment is posted once from a file, not as an inline body
    assert!(
        posts_from_file,
        "{REVIEW_WORKFLOW} must contain `--body-file`: the review comment is posted from \
         the file Claude wrote"
    );
    // And the red check names the reason when no comment was posted
    assert!(
        names_missing_comment,
        "{REVIEW_WORKFLOW} must contain the red reason `posted no review comment in this run`"
    );
    // And the red check names the reason when more than one comment was posted
    assert!(
        names_too_many_comments,
        "{REVIEW_WORKFLOW} must contain the red reason `expected exactly one` for more \
         than one claude[bot] comment in a run"
    );
}

// AC4: „Das CI-Review läuft mit Claude Opus, wie der lokale Reviewer in `reviewer.md`."
#[test]
fn ac4_ci_review_runs_with_opus_like_the_local_reviewer() {
    // Given the review workflow and the local reviewer agent
    let workflow = read_text(REVIEW_WORKFLOW);
    let reviewer = read_text(REVIEWER_AGENT);

    // When looking for the model each of them uses
    let reviewer_uses_opus = has_line(&reviewer, "model: opus");
    let workflow_uses_opus = workflow.contains("--model opus");

    // Then the local reviewer uses Opus
    assert!(
        reviewer_uses_opus,
        "{REVIEWER_AGENT} must have the line `model: opus`"
    );
    // And the CI review passes the same model alias to Claude
    assert!(
        workflow_uses_opus,
        "{REVIEW_WORKFLOW} must pass `--model opus` in `claude_args`, the same alias as \
         `model: opus` in {REVIEWER_AGENT}"
    );
}
