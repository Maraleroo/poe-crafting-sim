//! Acceptance tests for AB#61 "Verbesserungen aus der Retrospektive AB#52".
//!
//! This story has no `crafting-core` behaviour. Following ADR 0004, only lean
//! text-level checks are made here: a file exists, contains or does not contain a given
//! string, a line with a given text exists. There are no Markdown parsers. Sentences are
//! compared with normalized whitespace, so a line break inside a sentence does not matter.
//!
//! The expected sentences are the English anchor sentences from the tech lead's plan
//! (Task AB#64). For AC5, the hard rule follows the PO's decision recorded on AB#61:
//! the line reads exactly `- Never hard-code UI text. Add a translation key instead.`
//!
//! Red state before the implementation: all six tests fail, because the sentences are
//! missing, ADR 0005 to 0007 still say `proposed` and `CLAUDE.md` still has the old rule.
//!
//! What is checked outside `cargo test`: that the team actually follows the new rules.
//! All criteria are checkable in the PR; there is no verification after the merge.

use std::fs;
use std::path::{Path, PathBuf};

const TECH_LEAD: &str = ".claude/agents/tech-lead.md";
const MANAGING_AZURE_BOARDS: &str = ".claude/skills/managing-azure-boards/SKILL.md";
const STORY_SKILL: &str = ".claude/skills/story/SKILL.md";
const REVIEWING_CODE: &str = ".claude/skills/reviewing-code/SKILL.md";
const CLAUDE_MD: &str = "CLAUDE.md";

/// ADRs that must be accepted (AC2).
const ADRS: [&str; 3] = [
    "docs/adr/0005-ai-review-in-ci.md",
    "docs/adr/0006-pinned-ci-dependencies.md",
    "docs/adr/0007-legal-pages-and-privacy-constraints.md",
];
const STATUS_ACCEPTED: &str = "- Status: accepted";
const STATUS_PROPOSED: &str = "- Status: proposed";

/// New rule for the tech lead (AC1), plain ASCII apostrophes.
const NEW_ADR_RULE: &str = "New ADRs are written in the story's PR with status `accepted`; \
    the PO's merge is the acceptance.";

/// Sentences about the Azure CLI output on Windows (AC3).
const AZURE_CLI_SENTENCES: [&str; 4] = [
    "On Windows, the Azure CLI prints text in Windows-1252, not UTF-8",
    "`az.cmd` starts Python in isolated mode, so environment variables have no effect",
    "A wrongly displayed character in the console does not mean that the stored data is \
     broken.",
    "Verbatim copies from work items are checked byte for byte against the work item.",
];

/// Sentence in step 1 of the story skill (AC4).
const QUOTED_RULE_COUNTS: &str =
    "A rule that the PO quotes verbatim in an acceptance criterion counts as present.";
/// Sentence part in step 2 of the story skill (AC4).
const QUOTED_RULE_COMMITTED: &str = "commit it unchanged to `poe-crafting-rules`, checked \
    byte for byte against the work item";

/// Sentences in the architecture part of `CLAUDE.md` (AC5).
const UI_TEXT_SENTENCES: [&str; 3] = [
    "All UI text comes from translation files",
    "`crafting-core` returns structured results and reason codes, never UI text.",
    "All texts that come from the game, such as names of items, currencies and mods and mod texts, are data (ADR 0008, ADR 0011).",
];
/// Exact hard rule line (AC5, wording decided by the PO on AB#61).
const UI_TEXT_RULE: &str = "- Never hard-code UI text. Add a translation key instead.";
/// Old hard rule that must be gone (AC5).
const OLD_RULE: &str = "Never hard-code user-facing text";

/// New rule for review findings (AC6).
const FINDING_RULE: &str = "A finding names a problem; plain observations belong in the summary.";

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

/// Text of the numbered step `number` in the story skill: from the line that starts with
/// `<number>. ` up to the line that starts with `<number + 1>. `.
fn story_step(text: &str, number: u32) -> String {
    let start = format!("{number}. ");
    let next = format!("{}. ", number + 1);
    let lines: Vec<&str> = text.lines().collect();
    let first = lines
        .iter()
        .position(|line| line.starts_with(&start))
        .unwrap_or_else(|| panic!("{STORY_SKILL} must have a step starting with `{start}`"));
    let end = lines[first + 1..]
        .iter()
        .position(|line| line.starts_with(&next))
        .map_or(lines.len(), |offset| first + 1 + offset);
    lines[first..end].join("\n")
}

/// Text of the section `## Hard rules` in `CLAUDE.md`, up to the next `## ` heading.
fn hard_rules_section(claude_md: &str) -> &str {
    let (_, after_heading) = claude_md
        .split_once("## Hard rules")
        .unwrap_or_else(|| panic!("{CLAUDE_MD} must have a section `## Hard rules`"));
    after_heading.split("\n## ").next().unwrap_or(after_heading)
}

// AC1: „`tech-lead.md`: Neue ADRs werden im PR der Story mit Status `accepted`
// geschrieben; der Merge durch den PO ist die Annahme."
#[test]
fn ac1_tech_lead_writes_new_adrs_as_accepted() {
    // Given the tech lead's agent definition
    let tech_lead = read_text(TECH_LEAD);

    // When looking for the rule about the status of new ADRs
    let missing = missing_sentences(&tech_lead, &[NEW_ADR_RULE]);

    // Then the rule is there
    assert!(
        missing.is_empty(),
        "{TECH_LEAD} must contain the sentence {missing:?}"
    );
}

// AC2: „ADR 0005, 0006 und 0007 haben den Status `accepted`."
#[test]
fn ac2_adrs_0005_to_0007_are_accepted() {
    // Given ADR 0005, 0006 and 0007
    let adrs: Vec<(&str, String)> = ADRS.iter().map(|adr| (*adr, read_text(adr))).collect();

    // When reading their status lines
    let not_accepted: Vec<&str> = adrs
        .iter()
        .filter(|(_, text)| !text.lines().any(|line| line.trim() == STATUS_ACCEPTED))
        .map(|(adr, _)| *adr)
        .collect();
    let still_proposed: Vec<&str> = adrs
        .iter()
        .filter(|(_, text)| {
            text.lines()
                .any(|line| line.trim().starts_with(STATUS_PROPOSED))
        })
        .map(|(adr, _)| *adr)
        .collect();

    // Then each one has the line "- Status: accepted"
    assert!(
        not_accepted.is_empty(),
        "these ADRs must have the line `{STATUS_ACCEPTED}`: {not_accepted:?}"
    );
    // And none of them has a line "- Status: proposed" left
    assert!(
        still_proposed.is_empty(),
        "these ADRs must not have a line `{STATUS_PROPOSED}`: {still_proposed:?}"
    );
}

// AC3: „`managing-azure-boards` sagt: Unter Windows gibt die Azure CLI Text in
// Windows-1252 aus, nicht in UTF-8 (`az.cmd` startet Python im isolierten Modus,
// Umgebungsvariablen wirken nicht). Ein falsch dargestelltes Zeichen in der Konsole heißt
// nicht, dass die gespeicherten Daten kaputt sind. Wörtliche Kopien aus Work Items werden
// Byte für Byte gegen das Work Item geprüft."
#[test]
fn ac3_azure_boards_skill_explains_windows_1252_output_and_byte_checks() {
    // Given the Azure Boards skill
    let skill = read_text(MANAGING_AZURE_BOARDS);

    // When looking for the sentences about the console encoding and verbatim copies
    let missing = missing_sentences(&skill, &AZURE_CLI_SENTENCES);

    // Then all of them are there
    assert!(
        missing.is_empty(),
        "{MANAGING_AZURE_BOARDS} must contain these sentences: {missing:#?}"
    );
}

// AC4: „`story`-Skill: Eine Regel, die der PO in einem Akzeptanzkriterium wörtlich
// zitiert, zählt für die Definition of Ready als vorhanden. In Schritt 2 wird sie
// unverändert in `poe-crafting-rules` committet, Byte für Byte gegen das Work Item
// geprüft."
#[test]
fn ac4_story_skill_accepts_and_commits_rules_quoted_by_the_po() {
    // Given the story skill
    let skill = read_text(STORY_SKILL);

    // When reading step 1 (Definition of Ready) and step 2 (branch and status)
    let step_1 = story_step(&skill, 1);
    let step_2 = story_step(&skill, 2);
    let missing_in_step_1 = missing_sentences(&step_1, &[QUOTED_RULE_COUNTS]);
    let missing_in_step_2 = missing_sentences(&step_2, &[QUOTED_RULE_COMMITTED]);

    // Then step 1 counts a rule quoted verbatim by the PO as present
    assert!(
        missing_in_step_1.is_empty(),
        "step 1 of {STORY_SKILL} must contain the sentence {missing_in_step_1:?}"
    );
    // And step 2 commits it unchanged to the rules skill, checked byte for byte
    assert!(
        missing_in_step_2.is_empty(),
        "step 2 of {STORY_SKILL} must contain {missing_in_step_2:?}"
    );
}

// AC5: „`CLAUDE.md`: Alle UI-Texte kommen aus Übersetzungsdateien. `crafting-core`
// liefert strukturierte Ergebnisse und Begründungs-Codes, nie UI-Text. Spieldaten-Texte
// wie Item-Namen und Mod-Texte sind Daten (ADR 0008). Die harte Regel lautet: "Never
// hard-code UI text.""
// PO decision on AB#61: the hard rule keeps its second sentence and reads exactly
// "- Never hard-code UI text. Add a translation key instead."
#[test]
fn ac5_claude_md_separates_ui_text_from_game_data_text() {
    // Given the project instructions
    let claude_md = read_text(CLAUDE_MD);

    // When looking for the UI text sentences, the hard rule and the old rule
    let missing = missing_sentences(&claude_md, &UI_TEXT_SENTENCES);
    let has_rule_line = hard_rules_section(&claude_md)
        .lines()
        .any(|line| line.trim_end() == UI_TEXT_RULE);
    let has_old_rule = normalize_whitespace(&claude_md).contains(OLD_RULE);

    // Then the sentences about UI text and game data text are there
    assert!(
        missing.is_empty(),
        "{CLAUDE_MD} must contain these sentences: {missing:#?}"
    );
    // And the section "## Hard rules" has the line for UI text
    assert!(
        has_rule_line,
        "{CLAUDE_MD} must have the line `{UI_TEXT_RULE}` in its section `## Hard rules`"
    );
    // And the old wording of the rule is gone
    assert!(
        !has_old_rule,
        "{CLAUDE_MD} must no longer contain {OLD_RULE:?}"
    );
}

// AC6: „`reviewing-code`: Ein Befund nennt ein Problem; reine Feststellungen gehören in
// die Zusammenfassung."
#[test]
fn ac6_review_findings_name_problems_and_observations_go_to_the_summary() {
    // Given the review checklist
    let skill = read_text(REVIEWING_CODE);

    // When looking for the rule about findings
    let missing = missing_sentences(&skill, &[FINDING_RULE]);

    // Then the rule is there
    assert!(
        missing.is_empty(),
        "{REVIEWING_CODE} must contain the sentence {missing:?}"
    );
}
