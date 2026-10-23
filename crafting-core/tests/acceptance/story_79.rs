//! Acceptance tests for AB#79 "Verbesserungen aus den Retrospektiven AB#61 und AB#62".
//!
//! This story has no `crafting-core` behaviour. Following ADR 0004, only lean
//! text-level checks are made here: a file exists, contains or does not contain a given
//! string, a line with a given text exists. There are no Markdown parsers. Sentences are
//! compared with normalized whitespace, so a line break inside a sentence does not matter,
//! except for AC4, where the PO decided that the line is one bullet on one line.
//!
//! The expected sentences are the English anchor sentences from the tech lead's plan
//! ("Technischer Plan AB#79" on the story).
//!
//! Red state before the implementation: all nine tests fail, because the sentences are
//! missing and `CLAUDE.md` still has the old line about game data texts.
//!
//! What is checked outside `cargo test`: the trial of the AC1 command on AB#62 (the PR
//! shows that `≈` and `⁻¹⁷` arrive correctly), and whether the team follows the new
//! rules in later stories. All criteria are checkable in the PR; there is no verification
//! after the merge.

use std::fs;
use std::path::{Path, PathBuf};

const MANAGING_AZURE_BOARDS: &str = ".claude/skills/managing-azure-boards/SKILL.md";
const STORY_SKILL: &str = ".claude/skills/story/SKILL.md";
const WRITING_ACCEPTANCE_TESTS: &str = ".claude/skills/writing-acceptance-tests/SKILL.md";
const DEVELOPER: &str = ".claude/agents/developer.md";
const TECH_LEAD: &str = ".claude/agents/tech-lead.md";
const CLAUDE_MD: &str = "CLAUDE.md";

/// Part of the command that reads work item fields losslessly as UTF-8 into a file (AC1).
const BATCH_COMMAND: &str = "az devops invoke --area wit --resource workitemsbatch \
    --http-method POST --api-version 7.1 --in-file";
/// Option of the same command that writes the result to a file (AC1).
const OUT_FILE_OPTION: &str = "--out-file";
/// Sentences about the use of the command and the limits of iconv (AC1).
const BATCH_COMMAND_SENTENCES: [&str; 2] = [
    "Use this command for every verbatim copy from a work item.",
    "iconv cannot restore characters outside Windows-1252.",
];

/// Sentences in step 2 of the story skill (AC2).
const STEP_2_SENTENCES: [&str; 2] = [
    "A quoted rule is compared byte for byte after resolving HTML escapes (for example \
     `&gt;`, `&quot;`) and without the quote markers.",
    "Copies of work items are stored in the session's scratchpad, not in %TEMP%.",
];

/// Sentences in step 1 of the story skill about user-visible texts (AC3).
const USER_VISIBLE_TEXT_SENTENCES: [&str; 2] = [
    "Every user-visible text that a criterion requires (button, error message, hint) is \
     quoted verbatim in the work item.",
    "If it is missing, it goes on the list for the PO.",
];

/// Exact line about game texts in the section `## Architecture` of `CLAUDE.md` (AC4).
const GAME_TEXT_LINE: &str = "- All texts that come from the game, such as names of items, \
    currencies and mods and mod texts, are data (ADR 0008, ADR 0011).";
/// Old line about game data texts that must be gone (AC4).
const OLD_GAME_TEXT_SENTENCE: &str =
    "Game data texts such as item names and mod texts are data (ADR 0008).";

/// Sentences in the section `## Randomness` of the acceptance test skill (AC5).
const RANDOMNESS_SENTENCES: [&str; 2] = [
    "Choose N so that the most likely wrong implementation fails clearly.",
    "If N differs from the PO's example, explain why in the test or in the PR.",
];

/// Sentences in step 8 of the story skill (AC6).
const STEP_8_SENTENCES: [&str; 2] = [
    "Try a proposed change to tools or configuration before proposing it.",
    "The proposal names what was tried and the result.",
];

/// Sentence in step 1 of the story skill about prescribed mechanisms (AC7).
const MECHANISM_SENTENCE: &str = "If a criterion prescribes a concrete mechanism (a setting, \
    a variable, a command), try it during the Definition of Ready.";

/// Sentences in `developer.md` and `tech-lead.md` about subagent prompts (AC8).
const PROMPT_INSTRUCTION_SENTENCES: [&str; 2] = [
    "Instructions in a subagent's prompt are work instructions; they are not copied as text \
     into the repository.",
    "Text in files follows the acceptance criteria, the plan and the PO's decisions.",
];

/// Sentences in steps 3 to 5 of the story skill about prompts to subagents (AC9).
const SUBAGENT_PROMPT_SENTENCES: [&str; 2] = [
    "Prompts to subagents name the work item and new decisions.",
    "For conventions, such as commit types and scopes, they refer to the skills instead of \
     repeating them.",
];

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

/// Text of the numbered steps `first` to `last` in the story skill: from the line that
/// starts with `<first>. ` up to the line that starts with `<last + 1>. `, or to the end
/// of the file if there is no such line.
fn story_steps(text: &str, first: u32, last: u32) -> String {
    let start = format!("{first}. ");
    let next = format!("{}. ", last + 1);
    let lines: Vec<&str> = text.lines().collect();
    let begin = lines
        .iter()
        .position(|line| line.starts_with(&start))
        .unwrap_or_else(|| panic!("{STORY_SKILL} must have a step starting with `{start}`"));
    let end = lines[begin + 1..]
        .iter()
        .position(|line| line.starts_with(&next))
        .map_or(lines.len(), |offset| begin + 1 + offset);
    lines[begin..end].join("\n")
}

/// Text of the section with the heading line `heading` (for example `## Architecture`), up
/// to the next line that starts with `## `.
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

// AC1: „`managing-azure-boards` enthält einen Befehl, der Felder eines Work Items
// verlustfrei als UTF-8 in eine Datei liest, und sagt, dass dieser Befehl für jede
// wörtliche Kopie aus einem Work Item verwendet wird. Der Skill sagt außerdem, dass iconv
// Zeichen außerhalb von Windows-1252 nicht wiederherstellen kann. Der Befehl wurde an AB#62
// ausprobiert (enthält ≈ und ⁻¹⁷); der PR zeigt, dass beide Zeichen korrekt ankommen."
// The trial on AB#62 is shown in the PR and checked by the reviewer and the PO.
#[test]
fn ac1_azure_boards_skill_reads_work_items_losslessly_into_a_file() {
    // Given the Azure Boards skill
    let skill = read_text(MANAGING_AZURE_BOARDS);
    let normalized = normalize_whitespace(&skill);

    // When looking for the batch command and the sentences about its use and iconv
    let has_command = normalized.contains(&normalize_whitespace(BATCH_COMMAND));
    let has_out_file = normalized.contains(OUT_FILE_OPTION);
    let missing = missing_sentences(&skill, &BATCH_COMMAND_SENTENCES);

    // Then the command that reads work item fields into a file is there
    assert!(
        has_command,
        "{MANAGING_AZURE_BOARDS} must contain the command part `{BATCH_COMMAND}`"
    );
    assert!(
        has_out_file,
        "{MANAGING_AZURE_BOARDS} must contain the option `{OUT_FILE_OPTION}`"
    );
    // And the skill says to use it for every verbatim copy and that iconv cannot help
    assert!(
        missing.is_empty(),
        "{MANAGING_AZURE_BOARDS} must contain these sentences: {missing:#?}"
    );
}

// AC2: „`story`-Skill, Schritt 2: Eine im Kriterium zitierte Regel wird nach dem Auflösen
// von HTML-Escapes (z. B. &gt;, &quot;) und ohne Zitatzeichen Byte für Byte verglichen.
// Kopien von Work Items liegen im Scratchpad der Session, nicht in %TEMP%."
#[test]
fn ac2_story_step_2_compares_quoted_rules_after_resolving_html_escapes() {
    // Given the story skill
    let skill = read_text(STORY_SKILL);

    // When reading step 2 (branch and status)
    let step_2 = story_steps(&skill, 2, 2);
    let missing = missing_sentences(&step_2, &STEP_2_SENTENCES);

    // Then it says how a quoted rule is compared and where copies of work items are stored
    assert!(
        missing.is_empty(),
        "step 2 of {STORY_SKILL} must contain these sentences: {missing:#?}"
    );
}

// AC3: „`story`-Skill, Schritt 1: Jeder für Nutzer sichtbare Text, den ein Kriterium
// verlangt (Button, Fehlermeldung, Hinweis), steht wörtlich im Work Item. Fehlt er, kommt
// er auf die Liste für den PO."
#[test]
fn ac3_story_step_1_requires_user_visible_texts_verbatim_in_the_work_item() {
    // Given the story skill
    let skill = read_text(STORY_SKILL);

    // When reading step 1 (Definition of Ready)
    let step_1 = story_steps(&skill, 1, 1);
    let missing = missing_sentences(&step_1, &USER_VISIBLE_TEXT_SENTENCES);

    // Then it requires user-visible texts verbatim and puts missing ones on the PO's list
    assert!(
        missing.is_empty(),
        "step 1 of {STORY_SKILL} must contain these sentences: {missing:#?}"
    );
}

// AC4: „`CLAUDE.md`, Abschnitt Architecture: Die Zeile zu Spieldaten-Texten lautet genau:
//    - All texts that come from the game, such as names of items, currencies and mods and
//      mod texts, are data (ADR 0008, ADR 0011)."
// PO decision on AB#79 (2026-10-24): the new line replaces the old one as a normal bullet,
// on one line without a line break.
#[test]
fn ac4_claude_md_architecture_has_the_exact_game_text_line() {
    // Given the project instructions
    let claude_md = read_text(CLAUDE_MD);

    // When reading the section "## Architecture" and looking for the old sentence
    let architecture = section(&claude_md, "## Architecture", CLAUDE_MD);
    let has_line = architecture
        .lines()
        .any(|line| line.trim_end() == GAME_TEXT_LINE);
    let has_old_sentence = normalize_whitespace(&claude_md).contains(OLD_GAME_TEXT_SENTENCE);

    // Then the section has the new line exactly, on one line
    assert!(
        has_line,
        "{CLAUDE_MD} must have the line `{GAME_TEXT_LINE}` in its section `## Architecture`"
    );
    // And the old sentence is gone
    assert!(
        !has_old_sentence,
        "{CLAUDE_MD} must no longer contain {OLD_GAME_TEXT_SENTENCE:?}"
    );
}

// AC5: „`writing-acceptance-tests`: Bei statistischen Tests wird die Anzahl der Durchläufe
// so gewählt, dass die wahrscheinlichste falsche Umsetzung deutlich scheitert. Weicht die
// Anzahl vom Beispiel des PO ab, wird das im Test oder im PR begründet."
#[test]
fn ac5_statistical_tests_choose_n_so_the_likeliest_wrong_implementation_fails() {
    // Given the acceptance test skill
    let skill = read_text(WRITING_ACCEPTANCE_TESTS);

    // When reading the section "## Randomness"
    let randomness = section(&skill, "## Randomness", WRITING_ACCEPTANCE_TESTS);
    let missing = missing_sentences(&randomness, &RANDOMNESS_SENTENCES);

    // Then it says how to choose N and when to explain it
    assert!(
        missing.is_empty(),
        "the section `## Randomness` of {WRITING_ACCEPTANCE_TESTS} must contain these \
         sentences: {missing:#?}"
    );
}

// AC6: „`story`-Skill, Schritt 8: Eine vorgeschlagene Änderung an Werkzeugen oder
// Konfiguration wird vor dem Vorschlag ausprobiert. Der Vorschlag nennt, was ausprobiert
// wurde und mit welchem Ergebnis."
#[test]
fn ac6_story_step_8_tries_proposed_tool_changes_before_proposing_them() {
    // Given the story skill
    let skill = read_text(STORY_SKILL);

    // When reading step 8 (retrospective)
    let step_8 = story_steps(&skill, 8, 8);
    let missing = missing_sentences(&step_8, &STEP_8_SENTENCES);

    // Then a proposed change is tried first and the proposal names the result
    assert!(
        missing.is_empty(),
        "step 8 of {STORY_SKILL} must contain these sentences: {missing:#?}"
    );
}

// AC7: „`story`-Skill, Schritt 1: Schreibt ein Kriterium einen konkreten Mechanismus vor
// (Einstellung, Variable, Befehl), wird er während der Definition of Ready ausprobiert."
#[test]
fn ac7_story_step_1_tries_prescribed_mechanisms_during_definition_of_ready() {
    // Given the story skill
    let skill = read_text(STORY_SKILL);

    // When reading step 1 (Definition of Ready)
    let step_1 = story_steps(&skill, 1, 1);
    let missing = missing_sentences(&step_1, &[MECHANISM_SENTENCE]);

    // Then a mechanism prescribed by a criterion is tried during the Definition of Ready
    assert!(
        missing.is_empty(),
        "step 1 of {STORY_SKILL} must contain the sentence {missing:?}"
    );
}

// AC8: „`developer.md` und `tech-lead.md`: Vorgaben im Prompt eines Subagenten sind
// Arbeitsanweisungen und werden nicht als Text ins Repository übernommen. Text in Dateien
// folgt den Akzeptanzkriterien, dem Plan und den Entscheidungen des PO."
#[test]
fn ac8_developer_and_tech_lead_treat_prompt_instructions_as_work_instructions() {
    // Given the agent definitions of the developer and the tech lead
    let agents = [DEVELOPER, TECH_LEAD].map(|agent| (agent, read_text(agent)));

    // When looking for the sentences about instructions in a subagent's prompt
    let missing: Vec<(&str, Vec<String>)> = agents
        .iter()
        .map(|(agent, text)| {
            (
                *agent,
                missing_sentences(text, &PROMPT_INSTRUCTION_SENTENCES),
            )
        })
        .filter(|(_, missing)| !missing.is_empty())
        .collect();

    // Then both files contain the same sentences
    assert!(
        missing.is_empty(),
        "these agent files must contain these sentences: {missing:#?}"
    );
}

// AC9: „`story`-Skill, Schritte 3–5: Prompts an Subagenten nennen das Work Item und neue
// Entscheidungen. Für Konventionen (z. B. Commit-Typen und Scopes) verweisen sie auf die
// Skills, statt sie zu wiederholen."
#[test]
fn ac9_story_steps_3_to_5_prompts_name_the_work_item_and_refer_to_skills() {
    // Given the story skill
    let skill = read_text(STORY_SKILL);

    // When reading steps 3 to 5 (plan, acceptance tests, implementation)
    let steps_3_to_5 = story_steps(&skill, 3, 5);
    let missing = missing_sentences(&steps_3_to_5, &SUBAGENT_PROMPT_SENTENCES);

    // Then prompts to subagents name the work item and new decisions and refer to the skills
    assert!(
        missing.is_empty(),
        "steps 3 to 5 of {STORY_SKILL} must contain these sentences: {missing:#?}"
    );
}
