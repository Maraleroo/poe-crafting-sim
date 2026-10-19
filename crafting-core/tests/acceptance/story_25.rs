//! Acceptance tests for AB#25 "CI härten".
//!
//! This story has no `crafting-core` behaviour. Following ADR 0004, only lean
//! text-level checks are made here: a file exists, contains or does not contain a given
//! string, a line with a given text exists. There are no YAML, TOML or JSON parsers.
//!
//! On purpose, no test names a specific commit SHA, action version or Rust version:
//! every Dependabot update and every toolchain update would otherwise break a protected
//! test.
//!
//! What is checked outside `cargo test` (verification task AB#36 after the merge):
//! - AC2: that Dependabot runs without configuration errors and opens grouped PRs.
//! - AC3: that the runs of `ci.yml` (including the `deploy` job on `main`),
//!   `review.yml` and `claude.yml` show no Node.js 20 annotation. Here: only that
//!   `actions/checkout` is pinned to a major version that runs on Node 24.
//! - AC6: that the red check actually shows the message. Here: only the text in
//!   `review.yml`.
//! - AC8: that a branch can be created without a permission prompt. Here: only the
//!   allow rule in `.claude/settings.json`.

use std::fs;
use std::path::{Path, PathBuf};

/// Workflows whose actions must be pinned (AC1, AC3), relative to the repository root.
const WORKFLOWS: [&str; 3] = [
    ".github/workflows/ci.yml",
    ".github/workflows/review.yml",
    ".github/workflows/claude.yml",
];

const REVIEW_WORKFLOW: &str = ".github/workflows/review.yml";
const DEPENDABOT_CONFIG: &str = ".github/dependabot.yml";
const RUST_TOOLCHAIN: &str = "rust-toolchain.toml";
const SETTINGS: &str = ".claude/settings.json";
const ADR_0005: &str = "docs/adr/0005-ai-review-in-ci.md";

/// Length of a full git commit SHA in hex characters.
const SHA_LENGTH: usize = 40;

/// Separator between the pinned SHA and the version comment.
const VERSION_COMMENT: &str = " # v";

/// `actions/checkout` runs on Node 24 from this major version on.
const MIN_CHECKOUT_MAJOR: u32 = 5;

/// Key that starts the deny list in `.claude/settings.json`.
const DENY_KEY: &str = "\"deny\"";

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

/// Lines whose trimmed text starts with `uses:` or `- uses:`.
fn uses_lines(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| line.starts_with("uses:") || line.starts_with("- uses:"))
        .map(str::to_owned)
        .collect()
}

/// True if the text after the first `@` is exactly 40 hex characters followed by ` # v`.
fn is_pinned_to_sha_with_version(uses_line: &str) -> bool {
    let Some((_, reference)) = uses_line.split_once('@') else {
        return false;
    };
    let Some(sha) = reference.get(..SHA_LENGTH) else {
        return false;
    };
    sha.chars().all(|c| c.is_ascii_hexdigit())
        && reference[SHA_LENGTH..].starts_with(VERSION_COMMENT)
}

/// Major version from the trailing comment `# vN...`, if there is one.
fn comment_major_version(line: &str) -> Option<u32> {
    let (_, version) = line.rsplit_once("# v")?;
    let digits: String = version.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

/// Text of `.claude/settings.json` before and after the `"deny"` key.
fn settings_split_at_deny() -> (String, String) {
    let text = read_text(SETTINGS);
    let (before, after) = text
        .split_once(DENY_KEY)
        .unwrap_or_else(|| panic!("{SETTINGS} must contain the key {DENY_KEY}"));
    (before.to_owned(), after.to_owned())
}

// AC1: „Alle Actions in den Workflows sind auf Commit-SHAs festgelegt, mit der Version
// als Kommentar."
#[test]
fn ac1_all_workflow_actions_are_pinned_to_commit_shas_with_version_comment() {
    for workflow in WORKFLOWS {
        // Given a workflow of this repository
        let text = read_text(workflow);

        // When reading its `uses:` lines
        let lines = uses_lines(&text);

        // Then it uses at least one action
        assert!(
            !lines.is_empty(),
            "{workflow} must contain at least one `uses:` line"
        );
        // And every action is pinned to a full commit SHA with the version as a comment
        let unpinned: Vec<&String> = lines
            .iter()
            .filter(|line| !is_pinned_to_sha_with_version(line))
            .collect();
        assert!(
            unpinned.is_empty(),
            "{workflow}: every `uses:` line must have the form \
             `uses: owner/repo@<40 hex characters> # vX.Y.Z`, these do not: {unpinned:?}"
        );
    }
}

// AC2: „Dependabot ist für GitHub Actions und Cargo eingerichtet."
#[test]
fn ac2_dependabot_is_configured_for_github_actions_and_cargo() {
    // Given the Dependabot configuration
    let text = read_text(DEPENDABOT_CONFIG);

    // When reading its lines
    let lines: Vec<&str> = text.lines().collect();
    let has_ecosystem = |ecosystem: &str| {
        lines
            .iter()
            .any(|line| line.contains("package-ecosystem") && line.contains(ecosystem))
    };
    let interval_lines: Vec<&&str> = lines
        .iter()
        .filter(|line| line.contains("interval:"))
        .collect();
    let groups_lines = lines.iter().filter(|line| line.trim() == "groups:").count();

    // Then it is a version 2 configuration
    assert!(
        text.contains("version: 2"),
        "{DEPENDABOT_CONFIG} must contain `version: 2`"
    );
    // And it covers GitHub Actions and Cargo
    assert!(
        has_ecosystem("github-actions"),
        "{DEPENDABOT_CONFIG} must have a `package-ecosystem` line for `github-actions`"
    );
    assert!(
        has_ecosystem("cargo"),
        "{DEPENDABOT_CONFIG} must have a `package-ecosystem` line for `cargo`"
    );
    // And both run monthly (PO decision)
    assert!(
        interval_lines.len() >= 2,
        "{DEPENDABOT_CONFIG} must have at least two `interval:` lines, found {}",
        interval_lines.len()
    );
    let not_monthly: Vec<&&&str> = interval_lines
        .iter()
        .filter(|line| !line.contains("monthly"))
        .collect();
    assert!(
        not_monthly.is_empty(),
        "{DEPENDABOT_CONFIG}: every `interval:` line must be `monthly`, these are not: \
         {not_monthly:?}"
    );
    // And both are grouped into one PR per ecosystem
    assert!(
        groups_lines >= 2,
        "{DEPENDABOT_CONFIG} must have at least two `groups:` lines, found {groups_lines}"
    );
}

// AC3: „actions/checkout ist aktualisiert; es gibt keine Node-20-Warnung mehr."
#[test]
fn ac3_checkout_is_updated_to_a_node_24_major_version() {
    // Given all workflows of this repository
    let texts: Vec<(&str, String)> = WORKFLOWS
        .iter()
        .map(|workflow| (*workflow, read_text(workflow)))
        .collect();

    // When reading the lines that reference actions/checkout
    let checkout_lines: Vec<(&str, &str)> = texts
        .iter()
        .flat_map(|(workflow, text)| {
            text.lines()
                .filter(|line| line.contains("actions/checkout@"))
                .map(move |line| (*workflow, line.trim()))
        })
        .collect();

    // Then actions/checkout is used
    assert!(
        !checkout_lines.is_empty(),
        "at least one workflow must use `actions/checkout@`"
    );
    for (workflow, line) in checkout_lines {
        // And it is not referenced by a version tag
        assert!(
            !line.contains("actions/checkout@v"),
            "{workflow}: `actions/checkout` must be pinned to a commit SHA, not a tag: {line}"
        );
        // And its version comment names a major version that runs on Node 24
        let major = comment_major_version(line);
        assert!(
            major.is_some_and(|major| major >= MIN_CHECKOUT_MAJOR),
            "{workflow}: `actions/checkout` must have a version comment `# vN` with \
             N >= {MIN_CHECKOUT_MAJOR}, found {major:?} in: {line}"
        );
    }
}

// AC4: „rust-toolchain.toml nennt eine feste Rust-Version, passend zu ADR 0003."
#[test]
fn ac4_rust_toolchain_names_a_fixed_rust_version() {
    // Given the Rust toolchain file
    let text = read_text(RUST_TOOLCHAIN);

    // When reading the quoted value of its `channel` line
    let channel_line = text
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("channel"))
        .unwrap_or_else(|| panic!("{RUST_TOOLCHAIN} must have a `channel` line"));
    let channel = channel_line
        .split('"')
        .nth(1)
        .unwrap_or_else(|| panic!("the `channel` value must be quoted: {channel_line}"));

    // Then it is a fixed version of three dot-separated numbers
    let parts: Vec<&str> = channel.split('.').collect();
    assert!(
        parts.len() == 3
            && parts
                .iter()
                .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit())),
        "{RUST_TOOLCHAIN}: `channel` must be a fixed version like \"X.Y.Z\", found \"{channel}\""
    );
    // And the components and target from ADR 0003 are still there
    for required in ["rustfmt", "clippy", "wasm32-unknown-unknown"] {
        assert!(
            text.contains(required),
            "{RUST_TOOLCHAIN} must still contain `{required}`"
        );
    }
}

// AC5: „Die deny-Regeln erfassen auch Kurzformen wie -l ohne Leerzeichen."
#[test]
fn ac5_deny_rules_cover_short_label_flags_without_space() {
    // Given the team's permission settings
    let (_, deny) = settings_split_at_deny();

    // When reading the deny rules for labels on new PRs and issues
    let required = [
        "\"Bash(gh pr create -l*)\"",
        "\"Bash(gh pr create * -l*)\"",
        "\"Bash(gh issue create -l*)\"",
        "\"Bash(gh issue create * -l*)\"",
        "\"Bash(gh pr create *--label*)\"",
        "\"Bash(gh issue create *--label*)\"",
    ];
    let unanchored = [
        "\"Bash(gh pr create *-l*)\"",
        "\"Bash(gh issue create *-l*)\"",
    ];

    // Then `-l` directly followed by its value is denied, and `--label` still is
    for rule in required {
        assert!(
            deny.contains(rule),
            "{SETTINGS} must deny {rule} (after {DENY_KEY})"
        );
    }
    // And the unanchored form, which would also block words like "file-level", is not used
    let text = read_text(SETTINGS);
    for rule in unanchored {
        assert!(
            !text.contains(rule),
            "{SETTINGS} must not contain the unanchored rule {rule}"
        );
    }
}

// AC6: „Die Fehlermeldung von "Claude review" nennt beim Override den nötigen Re-run."
#[test]
fn ac6_review_error_message_names_the_rerun_for_the_override() {
    // Given the review workflow
    let text = read_text(REVIEW_WORKFLOW);

    // When looking for the error annotation that explains the override
    let error_line = text.lines().find(|line| {
        line.contains("::error")
            && line.contains("review-override")
            && line.contains("re-runs the Claude review job")
    });

    // Then it tells the PO to re-run the job after adding the label
    assert!(
        error_line.is_some(),
        "{REVIEW_WORKFLOW} must have a `::error` line that names `review-override` and \
         says the PO `re-runs the Claude review job`"
    );
}

// AC7: „ADR 0005 dokumentiert, dass ein Entfernen von review-override nach einem grünen
// Lauf erst beim nächsten Lauf wirkt."
#[test]
fn ac7_adr_0005_documents_that_removing_the_override_applies_on_the_next_run() {
    // Given ADR 0005
    let text = read_text(ADR_0005);

    // When looking for the amendment of this story
    let has_amendment = text.contains("## Amendment (AB#25");
    let documents_next_run = text.contains("the check stays green until the next run");

    // Then it has an amendment for AB#25
    assert!(
        has_amendment,
        "{ADR_0005} must contain an amendment `## Amendment (AB#25`"
    );
    // And it says that the check stays green until the next run
    assert!(
        documents_next_run,
        "{ADR_0005} must say that `the check stays green until the next run`"
    );
}

// AC8: „settings.json erlaubt "git fetch origin" ohne Rückfrage, damit Branches nach
// Schritt 2 des Story-Skills ohne Unterbrechung erstellt werden."
#[test]
fn ac8_settings_allow_git_fetch_origin_without_prompt() {
    // Given the team's permission settings
    let (allow, _) = settings_split_at_deny();

    // When looking at the rules before the deny list
    let rule = "\"Bash(git fetch origin)\"";

    // Then `git fetch origin` is allowed
    assert!(
        allow.contains(rule),
        "{SETTINGS} must allow {rule} (before {DENY_KEY})"
    );
}
