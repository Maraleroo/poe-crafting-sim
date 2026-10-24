//! Acceptance tests for AB#87 "Retrospektiven nur auf echte Probleme stützen".
//!
//! This story has no `crafting-core` behaviour. Following ADR 0004, only lean
//! text-level checks are made here: a file contains or does not contain a given string,
//! no line starts with a given text. There are no Markdown parsers. Text is compared with
//! normalized whitespace, so line breaks inside step 8 do not matter.
//!
//! Step 8 of the story skill is the text from the line that starts with `8. ` to the end
//! of the file. The four new sentences are copied from criterion 1 of the work item, HTML
//! escapes resolved; the expected full step 8 is the one from the tech lead's plan
//! ("Technischer Plan AB#87" on the story).
//!
//! Red state before the implementation: both tests fail, because the new sentences are
//! missing and the old sentence is still there.
//!
//! All criteria are checkable in the PR; there is no verification after the merge.

use std::fs;
use std::path::{Path, PathBuf};

const STORY_SKILL: &str = ".claude/skills/story/SKILL.md";

/// Start of the line that begins step 8 of the story skill.
const STEP_8_START: &str = "8. ";

/// Sentences that replace the old retrospective sentence in step 8 (AC1).
const NEW_RETRO_SENTENCES: [&str; 4] = [
    "write a short retrospective: what went well and what went wrong.",
    "Propose a change to a skill or instruction only for a problem that actually occurred \
     in this story and cost time, caused a wrong result or needed an avoidable question to \
     the PO. The proposal names that problem.",
    "If nothing needs to change, say so; that is a valid result.",
    "Propose the change; do not apply it yourself.",
];

/// Part of the old retrospective sentence that must be gone from the file (AC1).
const OLD_RETRO_PHRASE: &str = "what went wrong, which skill or instruction should change";

/// Start of a bullet line, after the indentation (AC1: step 8 is running text).
const BULLET_START: &str = "- ";

/// The whole step 8 after the change, with normalized whitespace (AC2).
const EXPECTED_STEP_8: &str = "8. After the PO has merged, write a short retrospective: \
    what went well and what went wrong. Propose a change to a skill or instruction only for \
    a problem that actually occurred in this story and cost time, caused a wrong result or \
    needed an avoidable question to the PO. The proposal names that problem. If nothing \
    needs to change, say so; that is a valid result. Propose the change; do not apply it \
    yourself. Try a proposed change to tools or configuration before proposing it. The \
    proposal names what was tried and the result. If the story has criteria that can only \
    be checked after the merge, its PR says `Related AB#<id>` instead of `Fixes AB#<id>`, \
    so the merge does not close it. The verification task from the plan runs after the \
    merge; set the story to Closed only after the PO has confirmed the result.";

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

/// Text of step 8 of the story skill: from the line that starts with `8. ` to the end of
/// the file.
fn step_8(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let begin = lines
        .iter()
        .position(|line| line.starts_with(STEP_8_START))
        .unwrap_or_else(|| panic!("{STORY_SKILL} must have a step starting with `{STEP_8_START}`"));
    lines[begin..].join("\n")
}

// AC1: „`story`-Skill, Schritt 8: Der Satz "write a short retrospective: what went wrong,
// which skill or instruction should change. Propose the change; do not apply it yourself."
// wird ersetzt durch genau diese Sätze:
//    - write a short retrospective: what went well and what went wrong.
//    - Propose a change to a skill or instruction only for a problem that actually occurred
//      in this story and cost time, caused a wrong result or needed an avoidable question
//      to the PO. The proposal names that problem.
//    - If nothing needs to change, say so; that is a valid result.
//    - Propose the change; do not apply it yourself.
// Die Sätze stehen als Fließtext in Schritt 8, wie bisher; die Spiegelstriche dienen nur der
// Aufzählung in diesem Kriterium."
#[test]
fn ac1_story_step_8_proposes_changes_only_for_real_problems() {
    // Given the story skill
    let skill = read_text(STORY_SKILL);

    // When reading step 8 (retrospective)
    let step = step_8(&skill);
    let normalized_step = normalize_whitespace(&step);
    let missing: Vec<&str> = NEW_RETRO_SENTENCES
        .iter()
        .copied()
        .filter(|sentence| !normalized_step.contains(&normalize_whitespace(sentence)))
        .collect();
    let has_old_phrase = normalize_whitespace(&skill).contains(OLD_RETRO_PHRASE);
    let bullet_lines: Vec<&str> = step
        .lines()
        .filter(|line| line.trim_start().starts_with(BULLET_START))
        .collect();

    // Then step 8 contains the four new sentences
    assert!(
        missing.is_empty(),
        "step 8 of {STORY_SKILL} must contain these sentences: {missing:#?}"
    );
    // And the old sentence is gone from the file
    assert!(
        !has_old_phrase,
        "{STORY_SKILL} must no longer contain {OLD_RETRO_PHRASE:?}"
    );
    // And step 8 is running text, not a bullet list
    assert!(
        bullet_lines.is_empty(),
        "step 8 of {STORY_SKILL} must be running text without bullets, found: \
         {bullet_lines:#?}"
    );
}

// AC2: „Die übrigen Sätze von Schritt 8 bleiben unverändert."
#[test]
fn ac2_story_step_8_keeps_all_other_sentences_unchanged() {
    // Given the story skill
    let skill = read_text(STORY_SKILL);

    // When reading step 8 (retrospective) with normalized whitespace
    let normalized_step = normalize_whitespace(&step_8(&skill));

    // Then it is exactly the old step 8 with only the retrospective sentence replaced
    assert_eq!(
        normalized_step,
        normalize_whitespace(EXPECTED_STEP_8),
        "step 8 of {STORY_SKILL} must be exactly the expected text from the plan"
    );
}
