//! Acceptance tests for AB#16 "KI-Review in CI".
//!
//! This story has no `crafting-core` behaviour. Following ADR 0004 and ADR 0005, only
//! the file-level contracts are tested here: the review workflow
//! `.github/workflows/review.yml` and the references to it in the team's skills and
//! agent definition.
//!
//! What is checked outside `cargo test`, and why:
//! - The workflow cannot run on the pull request that adds it: the Claude App token
//!   exchange is refused while the workflow file differs from the one on `main`. Its real
//!   behaviour is therefore verified by the PO on a verification PR after the merge
//!   (task AB#22).
//! - AC1: that a review actually runs on a new or updated PR and posts a comment in the
//!   `reviewing-code` format. Here: only the trigger and the action step.
//! - AC2: that the comment is authored by `claude[bot]`. Here: only the preconditions
//!   for the bot identity (OIDC token via `id-token: write`, no `github_token` input).
//! - AC3: the colour of the review check for BLOCKED, APPROVED and ESCALATED. It depends
//!   on the verdict of a real review run and has no file-level contract, so there is no
//!   test for it in this file on purpose.
//! - AC4: that findings are reported and the check is green with `review-override`.
//!   Here: only that the workflow knows the label.
//! - AC5: that changed acceptance tests are detected. Here: only the full history
//!   checkout that makes the `test(...)` commit reachable.
//! - AC7: that the texts describe the CI review instead of the local run. Here: only the
//!   reference to the workflow file; the PO reads the content in the diff.
//!
//! The workflow is read with a small line-based reader (no YAML dependency). It
//! understands the block style used by GitHub workflows, flow lists such as
//! `[main]`, comments and block scalars (`|`, `>`), which it skips.

use std::fs;
use std::path::{Path, PathBuf};

/// Path of the review workflow, relative to the repository root.
const REVIEW_WORKFLOW: &str = ".github/workflows/review.yml";

/// Workflow name, job id and check name from the plan.
const WORKFLOW_NAME: &str = "Review";
const JOB_ID: &str = "review";
const CHECK_NAME: &str = "Claude review";

/// The action that runs the review, and how it authenticates.
const CLAUDE_ACTION: &str = "anthropics/claude-code-action@";
const OAUTH_TOKEN_INPUT: &str = "claude_code_oauth_token";
const OAUTH_TOKEN_VALUE: &str = "${{ secrets.CLAUDE_CODE_OAUTH_TOKEN }}";

/// Label that turns a blocking review into a green check.
const OVERRIDE_LABEL: &str = "review-override";

/// The only permissions the review job may have.
const JOB_PERMISSIONS: [(&str, &str); 3] = [
    ("contents", "read"),
    ("pull-requests", "read"),
    ("id-token", "write"),
];

/// Files that describe the review process (AC7), relative to the repository root.
const REVIEW_DOCS: [&str; 3] = [
    ".claude/skills/reviewing-code/SKILL.md",
    ".claude/agents/reviewer.md",
    ".claude/skills/story/SKILL.md",
];

fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crafting-core must live inside the repository root")
}

fn repo_path(relative: &str) -> PathBuf {
    repo_root().join(relative)
}

fn read_text(path: &Path) -> String {
    let bytes = fs::read(path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    String::from_utf8_lossy(&bytes).into_owned()
}

fn review_workflow_text() -> String {
    read_text(&repo_path(REVIEW_WORKFLOW))
}

/// One structural line of a YAML file: its indentation and its text without the
/// indentation and without a trailing comment.
#[derive(Debug, Clone)]
struct Line {
    indent: usize,
    text: String,
}

impl Line {
    /// Text without a leading sequence marker `- `.
    fn content(&self) -> &str {
        let text = self.text.as_str();
        if text == "-" {
            ""
        } else {
            text.strip_prefix("- ").map_or(text, str::trim_start)
        }
    }

    fn is_sequence_item(&self) -> bool {
        self.text == "-" || self.text.starts_with("- ")
    }

    /// Key and value of a `key: value` line, without quotes.
    fn key_value(&self) -> Option<(String, String)> {
        let content = self.content();
        let (key, rest) = match content.chars().next() {
            Some(quote @ ('"' | '\'')) => {
                let end = content[1..].find(quote)? + 1;
                let rest = content[end + 1..].trim_start().strip_prefix(':')?;
                (content[1..end].to_owned(), rest)
            }
            _ => {
                let colon = content
                    .match_indices(':')
                    .map(|(i, _)| i)
                    .find(|&i| content[i + 1..].is_empty() || content[i + 1..].starts_with(' '))?;
                (content[..colon].trim().to_owned(), &content[colon + 1..])
            }
        };
        if !rest.is_empty() && !rest.starts_with(' ') {
            return None;
        }
        Some((key, unquote(rest.trim()).to_owned()))
    }

    fn key(&self) -> Option<String> {
        self.key_value().map(|(key, _)| key)
    }

    fn value(&self) -> String {
        self.key_value().map(|(_, value)| value).unwrap_or_default()
    }
}

fn unquote(value: &str) -> &str {
    for quote in ['"', '\''] {
        if value.len() >= 2 && value.starts_with(quote) && value.ends_with(quote) {
            return &value[1..value.len() - 1];
        }
    }
    value
}

/// Removes a trailing ` # comment` that is not inside quotes.
fn strip_comment(text: &str) -> &str {
    let mut quote: Option<char> = None;
    let mut previous = ' ';
    for (i, c) in text.char_indices() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None if c == '"' || c == '\'' => quote = Some(c),
            None if c == '#' && previous.is_whitespace() => return text[..i].trim_end(),
            None => {}
        }
        previous = c;
    }
    text.trim_end()
}

/// Structural lines of a YAML document. Empty lines, comment lines and the content
/// of block scalars (`key: |` or `key: >`) are left out.
fn structural_lines(yaml: &str) -> Vec<Line> {
    let mut lines = Vec::new();
    let mut scalar_parent: Option<usize> = None;
    for raw in yaml.lines() {
        let trimmed = raw.trim_start();
        let indent = raw.len() - trimmed.len();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(parent) = scalar_parent {
            if indent > parent {
                continue;
            }
            scalar_parent = None;
        }
        if trimmed.starts_with('#') {
            continue;
        }
        let line = Line {
            indent,
            text: strip_comment(trimmed).to_owned(),
        };
        let value = line.value();
        if value.starts_with('|') || value.starts_with('>') {
            scalar_parent = Some(indent);
        }
        lines.push(line);
    }
    lines
}

/// Direct children of a block: the lines with the smallest indentation in it.
fn direct_children(block: &[Line]) -> Vec<&Line> {
    let Some(indent) = block.iter().map(|line| line.indent).min() else {
        return Vec::new();
    };
    block.iter().filter(|line| line.indent == indent).collect()
}

/// Keys of the direct children of a block.
fn direct_keys(block: &[Line]) -> Vec<String> {
    direct_children(block)
        .into_iter()
        .filter_map(Line::key)
        .collect()
}

/// The direct child `key` of a block, and the lines nested below it.
fn child<'a>(block: &'a [Line], key: &str) -> Option<(&'a Line, &'a [Line])> {
    let indent = block.iter().map(|line| line.indent).min()?;
    let position = block
        .iter()
        .position(|line| line.indent == indent && line.key().as_deref() == Some(key))?;
    let line = &block[position];
    let nested = &block[position + 1..];
    let end = nested
        .iter()
        .position(|next| next.indent <= line.indent)
        .unwrap_or(nested.len());
    Some((line, &nested[..end]))
}

fn require_child<'a>(block: &'a [Line], key: &str, context: &str) -> (&'a Line, &'a [Line]) {
    child(block, key).unwrap_or_else(|| panic!("{REVIEW_WORKFLOW}: {context} has no key {key:?}"))
}

/// Values of a list, written either as a flow list `[a, b]` or as `- a` lines below.
fn list_values(line: &Line, nested: &[Line]) -> Vec<String> {
    let value = line.value();
    if let Some(inner) = value.strip_prefix('[').and_then(|v| v.strip_suffix(']')) {
        return inner
            .split(',')
            .map(|item| unquote(item.trim()).to_owned())
            .filter(|item| !item.is_empty())
            .collect();
    }
    if !value.is_empty() {
        return vec![value];
    }
    direct_children(nested)
        .into_iter()
        .filter(|item| item.is_sequence_item())
        .map(|item| unquote(item.content()).to_owned())
        .collect()
}

/// Items of a sequence block, each with all its lines.
fn sequence_items(block: &[Line]) -> Vec<&[Line]> {
    let Some(indent) = block.iter().map(|line| line.indent).min() else {
        return Vec::new();
    };
    let starts: Vec<usize> = block
        .iter()
        .enumerate()
        .filter(|(_, line)| line.indent == indent && line.is_sequence_item())
        .map(|(i, _)| i)
        .collect();
    starts
        .iter()
        .enumerate()
        .map(|(n, &start)| {
            let end = starts.get(n + 1).copied().unwrap_or(block.len());
            &block[start..end]
        })
        .collect()
}

/// Value of the first line with the given key anywhere in a block.
fn find_value(block: &[Line], key: &str) -> Option<String> {
    block
        .iter()
        .find(|line| line.key().as_deref() == Some(key))
        .map(Line::value)
}

/// The parsed review workflow.
struct Workflow {
    lines: Vec<Line>,
}

impl Workflow {
    fn load() -> Self {
        Self {
            lines: structural_lines(&review_workflow_text()),
        }
    }

    /// The `on:` block. GitHub also accepts the quoted key `"on"`.
    fn triggers(&self) -> &[Line] {
        let (line, nested) = require_child(&self.lines, "on", "the workflow");
        assert!(
            line.value().is_empty(),
            "{REVIEW_WORKFLOW}: `on:` must be a block of events, found {:?}",
            line.text
        );
        nested
    }

    fn review_job(&self) -> &[Line] {
        let (_, jobs) = require_child(&self.lines, "jobs", "the workflow");
        require_child(jobs, JOB_ID, "`jobs:`").1
    }

    fn review_steps(&self) -> Vec<&[Line]> {
        let (_, steps) = require_child(self.review_job(), "steps", "job `review`");
        sequence_items(steps)
    }

    /// The step of job `review` that uses an action starting with `prefix`.
    fn step_using(&self, prefix: &str) -> Option<&[Line]> {
        self.review_steps()
            .into_iter()
            .find(|step| find_value(step, "uses").is_some_and(|uses| uses.starts_with(prefix)))
    }
}

// AC1: „Gegeben ein PR auf main, wenn er geöffnet oder um neue Commits ergänzt wird,
// dann läuft automatisch ein Review-Workflow und postet einen Kommentar nach dem Format
// aus reviewing-code."
#[test]
fn ac1_review_workflow_runs_claude_on_opened_and_updated_prs_to_main() {
    // Given the review workflow
    let workflow = Workflow::load();

    // When reading its name, its pull request trigger and its review job
    let name = child(&workflow.lines, "name").map(|(line, _)| line.value());
    let (_, pull_request) = require_child(workflow.triggers(), "pull_request", "`on:`");
    let (branches_line, branches_nested) =
        require_child(pull_request, "branches", "`on.pull_request`");
    let branches = list_values(branches_line, branches_nested);
    let (types_line, types_nested) = require_child(pull_request, "types", "`on.pull_request`");
    let types = list_values(types_line, types_nested);
    let check_name = child(workflow.review_job(), "name").map(|(line, _)| line.value());
    let claude_step = workflow.step_using(CLAUDE_ACTION);

    // Then it is the workflow "Review" with the check "Claude review"
    assert_eq!(name.as_deref(), Some(WORKFLOW_NAME), "workflow name");
    assert_eq!(
        check_name.as_deref(),
        Some(CHECK_NAME),
        "check name of job `review`"
    );
    // And it runs for pull requests on main
    assert_eq!(branches, ["main"], "branches of the pull_request trigger");
    // And it runs when a pull request is opened or gets new commits
    for event in ["opened", "synchronize"] {
        assert!(
            types.iter().any(|t| t == event),
            "{REVIEW_WORKFLOW}: pull_request types must include {event:?}, found {types:?}"
        );
    }
    // And the review job runs Claude
    assert!(
        claude_step.is_some(),
        "{REVIEW_WORKFLOW}: job `review` must have a step that uses {CLAUDE_ACTION}"
    );
}

// AC2: „Gegeben ein Review-Kommentar aus der CI, wenn ich ihn ansehe, dann stammt er vom
// Claude-Bot-Account und nicht von meinem Account."
#[test]
fn ac2_review_uses_claude_app_identity_instead_of_a_github_token() {
    // Given the review workflow
    let workflow = Workflow::load();

    // When reading the Claude step and the permissions of job `review`
    let claude_step = workflow
        .step_using(CLAUDE_ACTION)
        .unwrap_or_else(|| panic!("{REVIEW_WORKFLOW}: no step uses {CLAUDE_ACTION}"));
    let oauth_token = find_value(claude_step, OAUTH_TOKEN_INPUT);
    let (_, permissions) = require_child(workflow.review_job(), "permissions", "job `review`");
    let id_token = find_value(permissions, "id-token");
    let github_token_inputs: Vec<&Line> = workflow
        .lines
        .iter()
        .filter(|line| line.key().as_deref() == Some("github_token"))
        .collect();

    // Then Claude authenticates with the Claude subscription token
    assert_eq!(
        oauth_token.as_deref(),
        Some(OAUTH_TOKEN_VALUE),
        "{REVIEW_WORKFLOW}: input {OAUTH_TOKEN_INPUT} of the Claude step"
    );
    // And the job may request an OIDC token, so the action gets the Claude App token
    assert_eq!(
        id_token.as_deref(),
        Some("write"),
        "{REVIEW_WORKFLOW}: job `review` needs `id-token: write`"
    );
    // And no GitHub token is passed, so the comment is not posted under another identity
    assert!(
        github_token_inputs.is_empty(),
        "{REVIEW_WORKFLOW}: must not pass a github_token input, found {github_token_inputs:?}"
    );
}

// AC4: „Gegeben ein PR mit dem Label review-override, wenn das Review läuft, dann meldet
// es seine Befunde, aber der Check ist grün."
#[test]
fn ac4_review_workflow_knows_the_override_label() {
    // Given the review workflow
    let text = review_workflow_text();

    // When looking for the override label
    let mentions_label = text.contains(OVERRIDE_LABEL);

    // Then the workflow handles the label review-override
    assert!(
        mentions_label,
        "{REVIEW_WORKFLOW} must handle the label {OVERRIDE_LABEL:?}"
    );
}

// AC5: „Gegeben ein PR mit Akzeptanztests, wenn das Review läuft, dann prüft es, dass die
// Tests seit dem test(...)-Commit unverändert sind."
#[test]
fn ac5_review_checks_out_full_history_to_find_the_test_commit() {
    // Given the review workflow
    let workflow = Workflow::load();

    // When reading the checkout step of job `review`
    let checkout = workflow
        .step_using("actions/checkout@")
        .unwrap_or_else(|| panic!("{REVIEW_WORKFLOW}: job `review` has no checkout step"));
    let fetch_depth = find_value(checkout, "fetch-depth");

    // Then the whole history is fetched, so the test(...) commit is reachable
    assert_eq!(
        fetch_depth.as_deref(),
        Some("0"),
        "{REVIEW_WORKFLOW}: the checkout step needs `fetch-depth: 0`"
    );
}

// AC6: „Gegeben der Review-Workflow, wenn ich ihn lese, dann nutzt er pull_request (nicht
// pull_request_target) und nur die minimal nötigen Rechte."
#[test]
fn ac6_review_workflow_uses_pull_request_and_minimal_permissions() {
    // Given the review workflow
    let text = review_workflow_text();
    let workflow = Workflow::load();

    // When reading its triggers and all its permissions
    let events = direct_keys(workflow.triggers());
    let (workflow_permissions, workflow_permissions_nested) =
        require_child(&workflow.lines, "permissions", "the workflow");
    let (job_permissions, job_permissions_nested) =
        require_child(workflow.review_job(), "permissions", "job `review`");
    let permission_blocks = workflow
        .lines
        .iter()
        .filter(|line| line.key().as_deref() == Some("permissions"))
        .count();
    let job_entries: Vec<(String, String)> = job_permissions_nested
        .iter()
        .map(|line| {
            line.key_value().unwrap_or_else(|| {
                panic!(
                    "{REVIEW_WORKFLOW}: unexpected permission line {:?}",
                    line.text
                )
            })
        })
        .collect();

    // Then pull_request is the only trigger
    assert_eq!(
        events,
        ["pull_request"],
        "{REVIEW_WORKFLOW}: events in `on:`"
    );
    // And pull_request_target appears nowhere
    assert!(
        !text.contains("pull_request_target"),
        "{REVIEW_WORKFLOW} must not mention pull_request_target"
    );
    // And the workflow grants no permissions by default
    assert_eq!(
        workflow_permissions.value(),
        "{}",
        "{REVIEW_WORKFLOW}: workflow-level permissions must be `{{}}`"
    );
    assert!(
        workflow_permissions_nested.is_empty(),
        "{REVIEW_WORKFLOW}: workflow-level permissions must be empty"
    );
    // And only job `review` declares permissions
    assert_eq!(
        permission_blocks, 2,
        "{REVIEW_WORKFLOW}: `permissions:` may appear only at workflow level and in job `review`"
    );
    // And job `review` has exactly the minimal permissions, one key per line
    assert!(
        job_permissions.value().is_empty(),
        "{REVIEW_WORKFLOW}: job permissions must be written one key per line, found {:?}",
        job_permissions.text
    );
    let indent = job_permissions_nested.first().map(|line| line.indent);
    assert!(
        job_permissions_nested
            .iter()
            .all(|line| Some(line.indent) == indent && !line.value().is_empty()),
        "{REVIEW_WORKFLOW}: job permissions must be flat `key: value` lines"
    );
    let mut actual = job_entries;
    actual.sort();
    let mut expected: Vec<(String, String)> = JOB_PERMISSIONS
        .iter()
        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
        .collect();
    expected.sort();
    assert_eq!(
        actual, expected,
        "{REVIEW_WORKFLOW}: permissions of job `review`"
    );
}

// AC7: „Gegeben der neue Ablauf, wenn ich reviewing-code, reviewer.md und Schritt 6 im
// Skill story lese, dann beschreiben sie das Review in der CI statt des lokalen Laufs."
#[test]
fn ac7_review_docs_point_to_the_ci_review_workflow() {
    // Given the review skill, the reviewer agent and the story skill
    let docs: Vec<(&str, String)> = REVIEW_DOCS
        .iter()
        .map(|doc| (*doc, read_text(&repo_path(doc))))
        .collect();

    // When looking for the review workflow in each of them
    let missing: Vec<&str> = docs
        .iter()
        .filter(|(_, content)| !content.contains(REVIEW_WORKFLOW))
        .map(|(doc, _)| *doc)
        .collect();

    // Then each one refers to the CI review workflow
    assert!(
        missing.is_empty(),
        "these files must refer to {REVIEW_WORKFLOW}: {missing:?}"
    );
}
