//! Acceptance tests for AB#335 "Messpunkte für die Kernoperationen".
//!
//! This story has no `crafting-core` behaviour. Following ADR 0004, only lean text-level
//! checks are made here: a file exists, contains or does not contain a given string, a
//! line with a given text exists. There are no YAML, TOML or JSON parsers. No test runs the
//! measuring program or checks a timing, and no test names a version, tag or SHA. The
//! interfaces are the ones named in the tech lead's plan ("Technischer Plan AB#335") and in
//! ADR 0025: `crafting-core/examples/measure.rs`, `.github/workflows/measure.yml`, the
//! operation keys, the command line `--out`, `--compare`, `--summary`, the command in
//! `CLAUDE.md`, and the allow rule in `.claude/settings.json` as decided by the PO on
//! 2026-11-02 (exact command, no trailing `*`).
//!
//! Checks of the program's source text ignore comment lines, so a name in a comment does
//! not count.
//!
//! What is checked outside `cargo test`:
//! - AC1: that the program really measures the mean time per call over many repetitions,
//!   on a suitable item for each currency: in the review against the plan.
//! - AC2: that a measurement takes at most about a minute locally: the developer states the
//!   measured time in the PR description.
//! - AC3 and AC4: the table in the run summary, the yellow warning naming the operation and
//!   the green job (verification task AB#341 after the merge).
//! - AC5 and AC6, the parts about the diff of this story: no change to a release-relevant
//!   file, to `Cargo.toml` or `Cargo.lock`, and no version raise. Later stories change these
//!   files, so the review checks them with `git diff origin/main...HEAD`.
//!
//! Red state before the implementation: every test fails, because
//! `crafting-core/examples/measure.rs` and `.github/workflows/measure.yml` do not exist,
//! `CLAUDE.md` has no measure command and `.claude/settings.json` has no allow rule for it.

use std::fs;
use std::path::{Path, PathBuf};

const MEASURE_PROGRAM: &str = "crafting-core/examples/measure.rs";
const MEASURE_WORKFLOW: &str = ".github/workflows/measure.yml";
const CORE_MANIFEST: &str = "crafting-core/Cargo.toml";
const CLAUDE_MD: &str = "CLAUDE.md";
const SETTINGS: &str = ".claude/settings.json";

/// Key that starts the deny list in `.claude/settings.json`.
const DENY_KEY: &str = "\"deny\"";

/// The documented local command (AC2), as it appears between backticks in `CLAUDE.md`.
const MEASURE_COMMAND: &str = "cargo run --release -p crafting-core --example measure";

/// The allow rule for exactly the documented command (PO decision of 2026-11-02).
const MEASURE_ALLOW_RULE: &str = "\"Bash(cargo run --release -p crafting-core --example measure)\"";

/// Operation keys that are not currencies (ADR 0025), as string literals in the program.
const VIEW_AND_DATA_KEYS: [&str; 3] = [
    "\"default_view_rows\"",
    "\"advanced_view_rows\"",
    "\"first_data_load\"",
];

/// The six currencies of the core; their keys are `Currency::id()` (ADR 0025).
const CURRENCY_VARIANTS: [&str; 6] = [
    "OrbOfTransmutation",
    "OrbOfAlteration",
    "OrbOfAugmentation",
    "RegalOrb",
    "ExaltedOrb",
    "ChaosOrb",
];

/// Command line options of the program, stable from ADR 0025 on (AC3).
const COMMAND_LINE_OPTIONS: [&str; 3] = ["\"--out\"", "\"--compare\"", "\"--summary\""];

/// Path roots the program may import from (AC6): the standard library, the core, and the
/// program's own modules.
const ALLOWED_USE_ROOTS: [&str; 7] = [
    "std",
    "core",
    "alloc",
    "crafting_core",
    "self",
    "super",
    "crate",
];

/// Length of a full git commit SHA in hex characters.
const SHA_LENGTH: usize = 40;

/// Separator between the pinned SHA and the version comment (ADR 0006).
const VERSION_COMMENT: &str = " # v";

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

/// The source text of the measuring program without comment lines (`//`, `///`, `//!`).
fn measure_program_code() -> String {
    read_text(MEASURE_PROGRAM)
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Number of leading spaces of a line.
fn indentation(line: &str) -> usize {
    line.len() - line.trim_start_matches(' ').len()
}

/// Blank lines and comment lines never end or start a block.
fn is_blank_or_comment(line: &str) -> bool {
    let content = line.trim();
    content.is_empty() || content.starts_with('#')
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

/// Names of the jobs: the lines directly below the top-level `jobs:` line that are
/// indented by exactly two spaces, up to the next top-level key.
fn job_lines(text: &str) -> Vec<String> {
    let mut lines = text.lines().skip_while(|line| line.trim_end() != "jobs:");
    assert!(
        lines.next().is_some(),
        "{MEASURE_WORKFLOW} must have a top-level `jobs:` line"
    );
    lines
        .filter(|line| !is_blank_or_comment(line))
        .take_while(|line| indentation(line) > 0)
        .filter(|line| indentation(line) == 2)
        .map(|line| line.trim().to_owned())
        .collect()
}

/// Text of the section `## Commands` in `CLAUDE.md`, up to the next `## ` heading.
fn claude_md_commands_section() -> String {
    let text = read_text(CLAUDE_MD);
    let (_, after) = text
        .split_once("## Commands")
        .unwrap_or_else(|| panic!("{CLAUDE_MD} must have a section `## Commands`"));
    after.split("\n## ").next().unwrap_or_default().to_owned()
}

/// Text of `.claude/settings.json` before the `"deny"` key: the allow list and above.
fn settings_allow_part() -> String {
    let text = read_text(SETTINGS);
    let (before, _) = text
        .split_once(DENY_KEY)
        .unwrap_or_else(|| panic!("{SETTINGS} must contain the key {DENY_KEY}"));
    before.to_owned()
}

/// Crate names of all dependency sections of `crafting-core/Cargo.toml`, with `-` written
/// as `_` as in Rust paths. A dependency line is a line `<name> = ...` below a section
/// header that contains `dependencies`.
fn core_dependency_names() -> Vec<String> {
    let text = read_text(CORE_MANIFEST);
    let mut in_dependencies = false;
    let mut names = Vec::new();
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_dependencies = line.contains("dependencies");
            continue;
        }
        if !in_dependencies || line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((name, _)) = line.split_once('=') {
            names.push(name.trim().replace('-', "_"));
        }
    }
    names
}

/// The path root of a `use` or `extern crate` line, for example `std` for
/// `use std::time::Instant;`. `None` for other lines.
fn use_root(line: &str) -> Option<String> {
    let line = line.trim();
    let line = line.strip_prefix("pub ").unwrap_or(line);
    let path = line
        .strip_prefix("use ")
        .or_else(|| line.strip_prefix("extern crate "))?;
    let path = path.trim_start().trim_start_matches("::");
    let root: String = path
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();
    Some(root)
}

/// All files below a directory, skipping build output (`target`, `dist`).
fn files_below(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return files;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = entry.file_name();
            if name != "target" && name != "dist" {
                files.extend(files_below(&path));
            }
        } else {
            files.push(path);
        }
    }
    files
}

// AC1: „Ein Messprogramm im Repository misst für jede Operation die mittlere Zeit pro Aufruf
// über viele Wiederholungen, nativ im Release-Modus und mit festem Startwert des
// Zufallsgenerators:
// - jede der sechs Currencies auf einem passenden Item (z. B. Alteration auf einem
//   magischen Iron Ring, Chaos Orb auf einem seltenen Bone Ring);
// - das Aufbauen der Mod-Zeilen für die Default- und für die Advanced-Ansicht eines
//   seltenen Items;
// - das erste Einlesen der Spieldaten."
#[test]
fn ac1_measuring_program_covers_every_core_operation_with_a_seeded_rng() {
    // Given the measuring program in the repository
    let code = measure_program_code();

    // When looking for the operations it measures and its random generator
    let missing_keys: Vec<&str> = VIEW_AND_DATA_KEYS
        .into_iter()
        .filter(|key| !code.contains(key))
        .collect();
    let missing_currencies: Vec<&str> = CURRENCY_VARIANTS
        .into_iter()
        .filter(|variant| !code.contains(variant))
        .collect();

    // Then it has the keys of both views and of the first data load
    assert!(
        missing_keys.is_empty(),
        "{MEASURE_PROGRAM} must name the operation keys {VIEW_AND_DATA_KEYS:?} as string \
         literals (ADR 0025), missing: {missing_keys:?}"
    );
    // And it measures each of the six currencies, keyed by `Currency::id()`
    assert!(
        missing_currencies.is_empty(),
        "{MEASURE_PROGRAM} must measure every currency of the core, missing: \
         {missing_currencies:?}"
    );
    assert!(
        code.contains(".id()"),
        "{MEASURE_PROGRAM} must key the currency operations by `Currency::id()` (ADR 0025)"
    );
    // And it uses the seeded random generator of the core
    assert!(
        code.contains("SeededRng"),
        "{MEASURE_PROGRAM} must use `SeededRng` with a fixed seed"
    );
}

// AC2: „Ein einzelner Befehl startet die Messung lokal. Er steht in `CLAUDE.md` im Abschnitt
// „Commands“. Eine Messung dauert lokal höchstens etwa eine Minute."
// Part 1: the command in `CLAUDE.md`.
#[test]
fn ac2_claude_md_commands_section_names_the_single_measure_command() {
    // Given the section `## Commands` of CLAUDE.md
    let section = claude_md_commands_section();

    // When looking for the measure command between backticks
    let quoted = format!("`{MEASURE_COMMAND}`");
    let command_line = section.lines().find(|line| line.contains(&quoted));

    // Then a line of the section names exactly this command, without arguments
    assert!(
        command_line.is_some(),
        "the section `## Commands` of {CLAUDE_MD} must have a line with {quoted}"
    );
    // And the program that the command runs exists
    assert!(
        repo_path(MEASURE_PROGRAM).is_file(),
        "{MEASURE_PROGRAM} must exist, because `--example measure` runs it"
    );
}

// AC2: „Ein einzelner Befehl startet die Messung lokal. Er steht in `CLAUDE.md` im Abschnitt
// „Commands“. Eine Messung dauert lokal höchstens etwa eine Minute."
// Part 2: the documented command runs without a permission prompt (plan AB#340), allowed
// for exactly this command and nothing more (PO decision of 2026-11-02).
#[test]
fn ac2_settings_allow_exactly_the_measure_command() {
    // Given the team's permission settings
    let allow = settings_allow_part();

    // When looking at the allow rules for `cargo run`
    let cargo_run_rules: Vec<&str> = allow
        .lines()
        .map(str::trim)
        .filter(|line| line.contains("Bash(cargo run"))
        .collect();

    // Then the documented command is allowed
    assert!(
        allow.contains(MEASURE_ALLOW_RULE),
        "{SETTINGS} must allow {MEASURE_ALLOW_RULE} (before {DENY_KEY})"
    );
    // And no allow rule for `cargo run` has a wildcard, so no other program and no
    // other arguments run without a prompt
    let wildcard_rules: Vec<&&str> = cargo_run_rules
        .iter()
        .filter(|line| line.contains('*'))
        .collect();
    assert!(
        wildcard_rules.is_empty(),
        "{SETTINGS} must not allow `cargo run` with a wildcard, found: {wildcard_rules:?}"
    );
}

// AC3: „CI misst bei jedem PR den Stand des PR und den Stand von `main` im selben Lauf auf
// demselben Rechner. Die Zusammenfassung des Laufs zeigt eine Tabelle mit Operation, Zeit
// auf `main`, Zeit im PR und Änderung in Prozent."
// Part 1: the workflow runs on every pull request, safely.
#[test]
fn ac3_measure_workflow_runs_on_pull_requests_with_read_only_rights_and_pinned_actions() {
    // Given the measure workflow
    let text = read_text(MEASURE_WORKFLOW);

    // When reading its trigger, permissions and actions
    let has_pull_request_trigger = text.lines().map(str::trim).any(|line| {
        line == "pull_request:" || (line.starts_with("on:") && line.contains("pull_request"))
    });
    let write_permissions: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|line| line.ends_with(": write") || line.contains("write-all"))
        .collect();
    let actions = uses_lines(&text);
    let unpinned: Vec<&String> = actions
        .iter()
        .filter(|line| !is_pinned_to_sha_with_version(line))
        .collect();

    // Then it runs on pull requests, never on pull_request_target
    assert!(
        has_pull_request_trigger,
        "{MEASURE_WORKFLOW} must be triggered by `pull_request`"
    );
    assert!(
        !text.contains("pull_request_target"),
        "{MEASURE_WORKFLOW} must not mention pull_request_target"
    );
    // And it may only read the repository contents
    assert!(
        text.contains("contents: read"),
        "{MEASURE_WORKFLOW} must declare `contents: read`"
    );
    assert!(
        write_permissions.is_empty(),
        "{MEASURE_WORKFLOW} must not grant write permissions, found: {write_permissions:?}"
    );
    // And every action is pinned to a full commit SHA with the version as a comment
    assert!(
        !actions.is_empty(),
        "{MEASURE_WORKFLOW} must contain at least one `uses:` line"
    );
    assert!(
        unpinned.is_empty(),
        "{MEASURE_WORKFLOW}: every `uses:` line must have the form \
         `uses: owner/repo@<40 hex characters> # vX.Y.Z` (ADR 0006), these do not: {unpinned:?}"
    );
}

// AC3: „CI misst bei jedem PR den Stand des PR und den Stand von `main` im selben Lauf auf
// demselben Rechner. Die Zusammenfassung des Laufs zeigt eine Tabelle mit Operation, Zeit
// auf `main`, Zeit im PR und Änderung in Prozent."
// Part 2: both states are measured in one job and compared into the run summary.
#[test]
fn ac3_measure_workflow_measures_pr_and_main_in_one_job_and_writes_the_summary() {
    // Given the measure workflow and the measuring program
    let text = read_text(MEASURE_WORKFLOW);
    let code = measure_program_code();

    // When reading its jobs, the commands it runs and the program's command line
    let jobs = job_lines(&text);
    let runs_release_program = text
        .lines()
        .any(|line| line.contains("--example measure") && line.contains("--release"));
    let missing_options: Vec<&str> = COMMAND_LINE_OPTIONS
        .into_iter()
        .filter(|option| !code.contains(option))
        .collect();

    // Then a single job measures both states, so they run on the same machine
    assert_eq!(
        jobs.len(),
        1,
        "{MEASURE_WORKFLOW} must have exactly one job, found: {jobs:?}"
    );
    // And the state of `main` is checked out next to the PR state
    assert!(
        text.contains("git worktree"),
        "{MEASURE_WORKFLOW} must check out the state of `main` with `git worktree`"
    );
    // And it builds the measuring program in release mode
    assert!(
        runs_release_program,
        "{MEASURE_WORKFLOW} must have a line with `--example measure` and `--release`"
    );
    // And it writes the times to files and compares them into the run summary
    for required in ["--out", "--compare", "--summary", "GITHUB_STEP_SUMMARY"] {
        assert!(
            text.contains(required),
            "{MEASURE_WORKFLOW} must contain `{required}`"
        );
    }
    // And the program offers the stable command line of ADR 0025
    assert!(
        missing_options.is_empty(),
        "{MEASURE_PROGRAM} must accept the options {COMMAND_LINE_OPTIONS:?} (ADR 0025), \
         missing: {missing_options:?}"
    );
}

// AC4: „Ist eine Operation mehr als 50 % langsamer als auf `main`, zeigt CI einen gelben
// Hinweis mit dem Namen der Operation. Der Build schlägt wegen Zeitmessungen nie fehl."
#[test]
fn ac4_slowdowns_are_warnings_and_the_job_is_not_fail_open() {
    // Given the measuring program and the measure workflow
    let code = measure_program_code();
    let text = read_text(MEASURE_WORKFLOW);

    // When looking for the warning annotation and for ways to ignore errors
    let emits_warning = code.contains("::warning");
    let ignored_errors: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|line| line.contains("continue-on-error") || line.contains("|| true"))
        .collect();

    // Then a slowdown is reported as a GitHub warning annotation, not as an error
    assert!(
        emits_warning,
        "{MEASURE_PROGRAM} must print a GitHub warning annotation `::warning`"
    );
    // And the workflow ignores no errors: timings never fail the job, because the program
    // always ends with 0, while a build or run error of the program still does
    assert!(
        ignored_errors.is_empty(),
        "{MEASURE_WORKFLOW} must not use `continue-on-error` or `|| true`, found: \
         {ignored_errors:?}"
    );
}

// AC5: „Das Messprogramm wird nicht in die Website kompiliert. Am Produktcode und an der
// Website ändert sich nichts, es ist kein Release."
#[test]
fn ac5_measuring_program_is_a_cargo_example_that_the_website_never_compiles() {
    // Given the measuring program, the core manifest, the core library and the website
    let program = repo_path(MEASURE_PROGRAM);
    let manifest = read_text(CORE_MANIFEST);
    let product_files: Vec<PathBuf> = files_below(&repo_path("crafting-core/src"))
        .into_iter()
        .chain(files_below(&repo_path("web")))
        .collect();

    // When looking for places that would build the program as part of the product
    let manifest_entries: Vec<&str> = manifest
        .lines()
        .map(str::trim)
        .filter(|line| line.contains("measure"))
        .collect();
    let references: Vec<String> = product_files
        .iter()
        .filter(|path| {
            path.file_name().is_some_and(|name| name == "measure.rs")
                || fs::read(path)
                    .is_ok_and(|bytes| String::from_utf8_lossy(&bytes).contains("measure.rs"))
        })
        .map(|path| path.display().to_string())
        .collect();

    // Then the program is a Cargo example of crafting-core, which Trunk never compiles
    assert!(
        program.is_file(),
        "{MEASURE_PROGRAM} must exist as a Cargo example"
    );
    // And the manifest has no entry that turns it into another kind of target
    assert!(
        manifest_entries.is_empty(),
        "{CORE_MANIFEST} must not mention the measuring program, found: {manifest_entries:?}"
    );
    // And neither the core library nor the website includes it
    assert!(
        references.is_empty(),
        "no file under crafting-core/src or web may be or include `measure.rs`, found: \
         {references:?}"
    );
}

// AC6: „Das Messprogramm kommt ohne neue Abhängigkeiten aus. Es nutzt nur die
// Standardbibliothek von Rust und die öffentlichen Funktionen des Kerns."
#[test]
fn ac6_measuring_program_uses_only_std_and_crafting_core() {
    // Given the measuring program and the dependencies an example of crafting-core could use
    let code = measure_program_code();
    let dependencies = core_dependency_names();

    // When reading its imports and the paths into those dependencies
    let foreign_imports: Vec<&str> = code
        .lines()
        .filter(|line| {
            use_root(line).is_some_and(|root| !ALLOWED_USE_ROOTS.contains(&root.as_str()))
        })
        .map(str::trim)
        .collect();
    let used_dependencies: Vec<&String> = dependencies
        .iter()
        .filter(|name| code.contains(&format!("{name}::")))
        .collect();

    // Then it uses the core
    assert!(
        code.contains("crafting_core"),
        "{MEASURE_PROGRAM} must use `crafting_core`"
    );
    // And it imports only from the standard library, the core and its own modules
    assert!(
        foreign_imports.is_empty(),
        "{MEASURE_PROGRAM} may only import from {ALLOWED_USE_ROOTS:?}, found: \
         {foreign_imports:?}"
    );
    // And it reaches into none of the core's own dependencies
    assert!(
        !dependencies.is_empty(),
        "{CORE_MANIFEST} must list its dependencies"
    );
    assert!(
        used_dependencies.is_empty(),
        "{MEASURE_PROGRAM} must not use the dependencies {used_dependencies:?} of \
         crafting-core directly"
    );
}
