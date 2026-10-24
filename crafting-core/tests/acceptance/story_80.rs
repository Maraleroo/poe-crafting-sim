//! Acceptance tests for AB#80 "Versionierung nach SemVer festlegen".
//!
//! Criteria 1 to 3 change team files. Following ADR 0004, only lean text-level checks are
//! made for them: a file contains or does not contain a given string, a line with a given
//! text exists. There are no Markdown parsers. Sentences are compared with normalized
//! whitespace, so a line break inside a sentence does not matter, except for criterion 1,
//! which asks for exactly these lines in `CLAUDE.md`.
//!
//! The expected lines of criterion 1 are copied from the work item, HTML escapes resolved.
//! The expected texts of criteria 2 and 3 are the ones from the tech lead's plan
//! ("Technischer Plan AB#80" on the story).
//!
//! Criterion 4 is checked through the public API of `crafting-core` and the web translation
//! file. On purpose, no test contains the concrete version 0.2.0: it would break with the
//! next release. The test checks that the version is a SemVer version greater than 0.1.0.
//! The footer template "Version {version}" is checked by story_5 AC2.
//!
//! Red state before the implementation: all four tests fail, because the new lines and
//! sentences are missing, `CLAUDE.md` still has the old versioning line, the version is
//! still 0.1.0 and `web/i18n/en.json` still contains "crafting-core v".
//!
//! Checked after the merge, outside `cargo test` (verification task of the plan):
//! criterion 5 (the footer on GitHub Pages) and the real behaviour of the review for
//! criterion 3.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

const CLAUDE_MD: &str = "CLAUDE.md";
const FOLLOWING_GIT_WORKFLOW: &str = ".claude/skills/following-git-workflow/SKILL.md";
const DEVELOPER: &str = ".claude/agents/developer.md";
const REVIEWING_CODE: &str = ".claude/skills/reviewing-code/SKILL.md";
const EN_JSON: &str = "web/i18n/en.json";

/// Section of `CLAUDE.md` that holds the versioning lines (AC1).
const WORKFLOW_HEADING: &str = "## Workflow";

/// Lines that replace the old versioning line in `CLAUDE.md`, in this order (AC1).
const VERSIONING_LINES: [&str; 4] = [
    "- Versioning follows Semantic Versioning 2.0.0 (https://semver.org/) strictly.",
    "- The public API is the user-visible behaviour of the website. Once items can be pasted \
     from the game, the accepted item texts (Ctrl+C and Ctrl+Alt+C in Path of Exile 1) are \
     part of it.",
    "- A release is every merge that changes the published website. Merges that only change \
     the team configuration or documentation are not releases.",
    "- Before 1.0.0, every release increments the minor version and resets the patch version \
     to 0.",
];

/// Old versioning line that must be gone from `CLAUDE.md` (AC1).
const OLD_VERSIONING_LINE: &str = "- Versioning follows Semantic Versioning, starting at 0.1.0.";

/// Section of the git workflow skill that says who raises the version and when (AC2).
const VERSIONING_HEADING: &str = "## Versioning";

/// Bullets of the section `## Versioning` in the git workflow skill (AC2).
const VERSIONING_SECTION_BULLETS: [&str; 4] = [
    "- A merge that changes the published website is a release; criterion 8 in \
     `reviewing-code` lists the files that count. In such a story, the developer raises the \
     version in step 5 of the `story` skill, before opening the PR.",
    "- The version is the workspace version in `Cargo.toml` (`[workspace.package] version`). \
     Both crates and the footer of the website use it. Commit `Cargo.lock` with the new \
     version.",
    "- Before 1.0.0, raise the minor version and reset the patch version to 0, for example \
     from 0.2.0 to 0.3.0.",
    "- Commit: `chore: raise version to <version>`.",
];

/// Sentence in step 2 of `developer.md` (AC2).
const DEVELOPER_VERSION_SENTENCE: &str = "If the PR changes the published website, raise the \
    version as described in `following-git-workflow` (section Versioning).";

/// Section of the review skill whose findings block the merge (AC3).
const CRITICAL_HEADING: &str = "## Critical: blocks the merge";

/// Criterion 8 in the section `## Critical: blocks the merge` of the review skill (AC3).
const VERSIONING_CRITERION: &str = "8. Versioning: the PR changes the published website, \
    that is the Trunk build output (files in `web/`, `crafting-core/src/`, `data/`, \
    `Cargo.toml`, `Cargo.lock` or `rust-toolchain.toml`), and does not raise the workspace \
    version (`[workspace.package] version` in `Cargo.toml`) compared to `origin/main`, or \
    raises it differently from `CLAUDE.md` (before 1.0.0: the next minor version with patch \
    version 0). Changes that only touch CI workflows, `.claude/`, `CLAUDE.md`, `docs/` or \
    tests (`crafting-core/tests/`, `#[cfg(test)]` modules) are not releases.";

/// Version before this story; the website version must be greater (AC4).
const FIRST_VERSION: (u64, u64, u64) = (0, 1, 0);

/// Old footer text that must be gone from the English translation file (AC4).
const OLD_FOOTER_TEXT: &str = "crafting-core v";

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

/// Collapses every run of whitespace, including line breaks, into one space.
fn normalize_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Sentences from `expected` that the normalized `text` does not contain.
fn missing_sentences(text: &str, expected: &[&str]) -> Vec<String> {
    let text = normalize_whitespace(text);
    expected
        .iter()
        .map(|sentence| normalize_whitespace(sentence))
        .filter(|sentence| !text.contains(sentence))
        .collect()
}

/// Text of the section with the heading line `heading`, up to the next line that starts
/// with `## `, or to the end of the file.
fn section(text: &str, heading: &str, file: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let begin = lines
        .iter()
        .position(|line| line.trim_end() == heading)
        .unwrap_or_else(|| panic!("{file} must have a section `{heading}`"));
    let end = lines[begin + 1..]
        .iter()
        .position(|line| line.starts_with("## "))
        .map_or(lines.len(), |offset| begin + 1 + offset);
    lines[begin..end].join("\n")
}

/// Text of the numbered step `step`: from the line that starts with `<step>. ` up to the
/// line that starts with `<step + 1>. `, or to the end of the file.
fn numbered_step(text: &str, step: u32, file: &str) -> String {
    let start = format!("{step}. ");
    let next = format!("{}. ", step + 1);
    let lines: Vec<&str> = text.lines().collect();
    let begin = lines
        .iter()
        .position(|line| line.starts_with(&start))
        .unwrap_or_else(|| panic!("{file} must have a step starting with `{start}`"));
    let end = lines[begin + 1..]
        .iter()
        .position(|line| line.starts_with(&next))
        .map_or(lines.len(), |offset| begin + 1 + offset);
    lines[begin..end].join("\n")
}

/// A numeric SemVer identifier: digits only, no leading zero unless it is `0`.
fn is_numeric_identifier(part: &str) -> bool {
    !part.is_empty()
        && part.chars().all(|c| c.is_ascii_digit())
        && (part == "0" || !part.starts_with('0'))
}

/// Dot-separated pre-release or build identifiers: non-empty, ASCII alphanumerics and
/// hyphens. Numeric pre-release identifiers must not have leading zeros.
fn are_valid_identifiers(text: &str, pre_release: bool) -> bool {
    text.split('.').all(|part| {
        !part.is_empty()
            && part.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
            && (!pre_release
                || !part.chars().all(|c| c.is_ascii_digit())
                || is_numeric_identifier(part))
    })
}

/// Major, minor and patch version if `version` is a valid Semantic Versioning 2.0.0
/// version (https://semver.org/), otherwise `None`.
fn semver_core(version: &str) -> Option<(u64, u64, u64)> {
    let (rest, build) = match version.split_once('+') {
        Some((rest, build)) => (rest, Some(build)),
        None => (version, None),
    };
    let (core, pre_release) = match rest.split_once('-') {
        Some((core, pre_release)) => (core, Some(pre_release)),
        None => (rest, None),
    };
    if build.is_some_and(|b| !are_valid_identifiers(b, false))
        || pre_release.is_some_and(|p| !are_valid_identifiers(p, true))
    {
        return None;
    }
    let parts: Vec<&str> = core.split('.').collect();
    if parts.len() != 3 || !parts.iter().all(|part| is_numeric_identifier(part)) {
        return None;
    }
    Some((
        parts[0].parse().ok()?,
        parts[1].parse().ok()?,
        parts[2].parse().ok()?,
    ))
}

// AC1: „`CLAUDE.md`, Abschnitt Workflow: Die Zeile zur Versionierung wird ersetzt durch
// genau diese Zeilen:
//    - Versioning follows Semantic Versioning 2.0.0 (https://semver.org/) strictly.
//    - The public API is the user-visible behaviour of the website. Once items can be
//      pasted from the game, the accepted item texts (Ctrl+C and Ctrl+Alt+C in Path of
//      Exile 1) are part of it.
//    - A release is every merge that changes the published website. Merges that only
//      change the team configuration or documentation are not releases.
//    - Before 1.0.0, every release increments the minor version and resets the patch
//      version to 0."
// (In the work item, each of the four lines is one line; they are wrapped here only to
// fit the line length.)
#[test]
fn ac1_claude_md_workflow_defines_strict_semver_lines() {
    // Given CLAUDE.md
    let claude_md = read_text(CLAUDE_MD);

    // When reading the section `## Workflow`
    let workflow = section(&claude_md, WORKFLOW_HEADING, CLAUDE_MD);
    let lines: Vec<&str> = workflow.lines().map(str::trim_end).collect();
    let missing: Vec<&str> = VERSIONING_LINES
        .iter()
        .copied()
        .filter(|expected| !lines.contains(expected))
        .collect();
    let in_order = lines
        .windows(VERSIONING_LINES.len())
        .any(|window| window == VERSIONING_LINES);
    let has_old_line = claude_md
        .lines()
        .any(|line| line.trim_end() == OLD_VERSIONING_LINE);

    // Then the section contains exactly these four lines
    assert!(
        missing.is_empty(),
        "section `{WORKFLOW_HEADING}` of {CLAUDE_MD} must contain these lines: {missing:#?}"
    );
    // And they stand together in the given order, where the old line was
    assert!(
        in_order,
        "the four versioning lines in {CLAUDE_MD} must follow each other in the order of \
         criterion 1"
    );
    // And the old versioning line is gone
    assert!(
        !has_old_line,
        "{CLAUDE_MD} must no longer contain the line {OLD_VERSIONING_LINE:?}"
    );
}

// AC2: „Die Team-Konfiguration legt fest, wer die Version in welchem Schritt einer Story
// erhöht."
// Per the plan: the developer raises the version in step 5 of the `story` skill, written in
// a new section `## Versioning` of `following-git-workflow` and referenced from step 2 of
// `developer.md`.
#[test]
fn ac2_team_configuration_says_who_raises_the_version_in_which_step() {
    // Given the git workflow skill and the developer agent
    let git_workflow = read_text(FOLLOWING_GIT_WORKFLOW);
    let developer = read_text(DEVELOPER);

    // When reading the section `## Versioning` and step 2 of the developer agent
    let versioning = section(&git_workflow, VERSIONING_HEADING, FOLLOWING_GIT_WORKFLOW);
    let missing_in_skill = missing_sentences(&versioning, &VERSIONING_SECTION_BULLETS);
    let step_2 = numbered_step(&developer, 2, DEVELOPER);
    let missing_in_developer = missing_sentences(&step_2, &[DEVELOPER_VERSION_SENTENCE]);

    // Then the skill says who raises the version, in which step and how
    assert!(
        missing_in_skill.is_empty(),
        "section `{VERSIONING_HEADING}` of {FOLLOWING_GIT_WORKFLOW} must contain: \
         {missing_in_skill:#?}"
    );
    // And the developer agent points to it in step 2
    assert!(
        missing_in_developer.is_empty(),
        "step 2 of {DEVELOPER} must contain: {missing_in_developer:#?}"
    );
}

// AC3: „Ein PR, der die veröffentlichte Website ändert, ohne die Version zu erhöhen, wird
// im Review blockiert."
// The real behaviour of the review is checked after the merge (verification task).
#[test]
fn ac3_review_blocks_website_changes_without_version_raise() {
    // Given the review skill
    let skill = read_text(REVIEWING_CODE);

    // When reading the section of findings that block the merge
    let critical = section(&skill, CRITICAL_HEADING, REVIEWING_CODE);
    let missing = missing_sentences(&critical, &[VERSIONING_CRITERION]);

    // Then it contains criterion 8 about versioning
    assert!(
        missing.is_empty(),
        "section `{CRITICAL_HEADING}` of {REVIEWING_CODE} must contain: {missing:#?}"
    );
}

// AC4: „Der Footer der Website zeigt statt "crafting-core v0.1.0" den Text "Version 0.2.0"."
// Checked as a contract, not as a fixed value: the version is a SemVer version greater
// than 0.1.0, and the English translation file no longer contains "crafting-core v". The
// file content stands in for the rendered footer; the preview and the verification after
// the merge check the rendered page.
#[test]
fn ac4_footer_shows_a_raised_semver_version_without_crafting_core_prefix() {
    // Given crafting-core and the English translation file of the web page
    let content = read_text(EN_JSON);
    let translations: Value = serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("{EN_JSON} is not valid JSON: {e}"));
    let Value::Object(translations) = translations else {
        panic!("{EN_JSON} must contain a JSON object at the top level");
    };

    // When the footer asks crafting-core for its version
    let version = crafting_core::version();
    let core = semver_core(version);
    let old_texts: Vec<String> = translations
        .iter()
        .filter(|(_, value)| value.as_str().is_some_and(|v| v.contains(OLD_FOOTER_TEXT)))
        .map(|(key, value)| format!("{key}: {value}"))
        .collect();

    // Then the version is a Semantic Versioning 2.0.0 version
    let core = core.unwrap_or_else(|| {
        panic!("crafting_core::version() must be a SemVer 2.0.0 version, got {version:?}")
    });
    // And it is greater than 0.1.0
    assert!(
        core > FIRST_VERSION,
        "crafting_core::version() must be greater than 0.1.0, got {version:?}"
    );
    // And no text of the web page still says "crafting-core v"
    assert!(
        old_texts.is_empty(),
        "{EN_JSON} must not contain {OLD_FOOTER_TEXT:?}, found: {old_texts:#?}"
    );
}
