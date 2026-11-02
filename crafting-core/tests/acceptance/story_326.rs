//! Acceptance tests for AB#326 "Spieltexte nur im Code verbieten, nicht in Kommentaren".
//!
//! The PO's decision (ADR 0011, amendment AB#326): game texts must not appear as text literals
//! in the code of `crafting-core` and `web`; comments and documentation may name them. "Code"
//! is everything in the files under `crafting-core/src` and `web/` (without build output) except
//! comments, each outside string literals:
//! - `.rs`: `//` to the end of the line (also `///` and `//!`) and nested `/* */` (also
//!   `/** */`). String literals are `"…"` with escapes, `r"…"` and `r#"…"#` with any number of
//!   `#`, `b"…"`, `br"…"`, `c"…"`, and character literals such as `'"'` and `'\''`. `'a` is a
//!   lifetime or a label, not a character literal.
//! - `.css`: `/* */` outside `"…"` and `'…'`.
//! - `.html`: `<!-- -->`.
//! - `.toml`: `#` to the end of the line outside `"…"`, `'…'`, `"""…"""` and `'''…'''`.
//! - Every other file counts in full, among them `web/i18n/en.json` and the privacy pages
//!   `web/legal/*.md`, which are content of the website, not documentation.
//!
//! This file holds the one shared helper of the tech lead's plan, written by hand without a new
//! dependency (`syn` or `proc-macro2` would turn `///` into a `#[doc = "…"]` literal):
//! `without_comments`, `code_of`, `files_containing_in_code` and `findings_in_code`. Case
//! matters. A comment is replaced by one space and keeps its line breaks, so the lines of a file
//! stay where they are. The checks that a game text lives only in the game data call it
//! (adapted for AB#326, PO-confirmed): `assert_only_in_game_data` in `story_113`,
//! `story_114`, `story_276`, `story_299`, `story_306` and `story_313`; `story_52` AC2 (the base
//! name); `story_62` AC1 (the currency name) and AC5 (mod names, mod text lines and stat ids);
//! and in `story_186` AC7 the part on class and base names (comments are removed first, then
//! the base ids). Every other text check keeps searching comments too, for example that the
//! page does not use `valid_mods` (AB#248).
//!
//! Criterion 3, the local trial (test designer, 2026-11-02, on the story branch), with the test
//! `story_313::ac2_chaos_orb_exists_with_its_game_name_and_description`:
//! 1. Before the adaptation, the line `// Trial AB#326: Chaos Orb` was added to
//!    `crafting-core/src/craft.rs`. The test failed with "\"Chaos Orb\" must only be in
//!    data/currencies.json, also found in: [...craft.rs]": the old check searched comments.
//! 2. After the adaptation, the same comment line was added again: the test passed.
//! 3. Then the same text as a literal, `const _TRIAL_AB326: &str = "Chaos Orb";`, in the same
//!    place: the test failed with the same message naming `craft.rs`.
//! 4. Both trials were reverted; `crafting-core/src` is unchanged by this commit.
//!
//! The tests below repeat the trial with sample sources per file type, including the edge cases
//! of the plan: a URL with `//` and a game text in the same string, `/*` in a string, a lifetime
//! before a string, raw strings with `#`, character literals with quotes and nested block
//! comments.
//!
//! Red state: the tests in this file are green from the start. They test the helper of this
//! file, and the story changes no product behaviour (criterion 6). The evidence that the
//! adapted checks still catch a literal and no longer catch a comment is the local trial above.
//!
//! No permanent test (`writing-acceptance-tests`): criteria 1, 4 and 5 are wording (ADR 0011,
//! the comments in `crafting-core/src` and `web/src`, the sentence in `writing-acceptance-tests`)
//! and are checked in the review and at the acceptance. Criterion 6 is checked by the whole
//! suite, `cargo test --all`.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

// ---------------------------------------------------------------------------------------
// Values
// ---------------------------------------------------------------------------------------

/// A game text for the samples: the name of a currency in the game data.
const GAME_TEXT: &str = "Chaos Orb";
const GAME_TEXT_ID: &str = "chaos_orb";
const CURRENCIES_JSON: &str = "data/currencies.json";

// ---------------------------------------------------------------------------------------
// The shared helper (tech lead's plan)
// ---------------------------------------------------------------------------------------

/// The content of a file of the given type (extension without the dot) without its comments
/// (ADR 0011, amendment AB#326). Every comment is replaced by one space followed by the line
/// breaks it contained. Files of other types are returned in full.
pub(crate) fn without_comments(extension: &str, content: &str) -> String {
    let chars: Vec<char> = content.chars().collect();
    match extension {
        "rs" => rust_without_comments(&chars),
        "css" => css_without_comments(&chars),
        "html" => html_without_comments(&chars),
        "toml" => toml_without_comments(&chars),
        _ => content.to_owned(),
    }
}

/// The content of the file at `path` without its comments, by the file's extension.
pub(crate) fn code_of(path: &Path) -> String {
    let bytes = fs::read(path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    let content = String::from_utf8_lossy(&bytes);
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("");
    without_comments(extension, &content)
}

/// The files whose code (content without comments) contains `needle`.
pub(crate) fn files_containing_in_code(files: &[PathBuf], needle: &str) -> Vec<String> {
    found_in_code(files, &[needle])
        .into_iter()
        .map(|(file, _)| file)
        .collect()
}

/// "<file> contains <needle>" for every needle found in the code of one of the files. Each
/// file is read and stripped once.
pub(crate) fn findings_in_code(files: &[PathBuf], needles: &[&str]) -> Vec<String> {
    found_in_code(files, needles)
        .into_iter()
        .map(|(file, needle)| format!("{file} contains {needle:?}"))
        .collect()
}

/// Pairs of file and needle for every needle found in the code of one of the files.
fn found_in_code(files: &[PathBuf], needles: &[&str]) -> Vec<(String, String)> {
    let mut found = Vec::new();
    for path in files {
        let code = code_of(path);
        for needle in needles {
            if code.contains(needle) {
                found.push((path.display().to_string(), (*needle).to_owned()));
            }
        }
    }
    found
}

/// One space for the comment, then its line breaks.
fn push_comment(out: &mut String, comment: &[char]) {
    out.push(' ');
    out.extend(comment.iter().filter(|c| **c == '\n'));
}

fn starts_with_at(chars: &[char], at: usize, pattern: &str) -> bool {
    pattern
        .chars()
        .enumerate()
        .all(|(offset, expected)| chars.get(at + offset) == Some(&expected))
}

/// The index just after the first `pattern` at or after `from`, or the end.
fn end_after(chars: &[char], from: usize, pattern: &str) -> usize {
    let length = pattern.chars().count();
    (from..chars.len())
        .find(|index| starts_with_at(chars, *index, pattern))
        .map_or(chars.len(), |index| index + length)
}

/// The index of the line break that ends the line at `from` (not consumed), or the end.
fn line_end(chars: &[char], from: usize) -> usize {
    (from..chars.len())
        .find(|index| chars[*index] == '\n')
        .unwrap_or(chars.len())
}

/// The index just after a string that opens at `start` with `quote`. With `escapes`, a
/// backslash escapes the next character.
fn quoted_end(chars: &[char], start: usize, quote: char, escapes: bool) -> usize {
    let mut index = start + 1;
    while index < chars.len() {
        if escapes && chars[index] == '\\' {
            index += 2;
        } else if chars[index] == quote {
            return index + 1;
        } else {
            index += 1;
        }
    }
    chars.len()
}

fn rust_without_comments(chars: &[char]) -> String {
    let mut out = String::with_capacity(chars.len());
    let mut index = 0;
    while index < chars.len() {
        let c = chars[index];
        let end = if starts_with_at(chars, index, "//") {
            let end = line_end(chars, index);
            push_comment(&mut out, &chars[index..end]);
            index = end;
            continue;
        } else if starts_with_at(chars, index, "/*") {
            let end = rust_block_comment_end(chars, index);
            push_comment(&mut out, &chars[index..end]);
            index = end;
            continue;
        } else if c == '"' {
            quoted_end(chars, index, '"', true)
        } else if c == '\'' {
            rust_char_literal_end(chars, index)
        } else if c.is_alphabetic() || c == '_' {
            let identifier_end = (index..chars.len())
                .find(|i| !(chars[*i].is_alphanumeric() || chars[*i] == '_'))
                .unwrap_or(chars.len());
            let identifier: String = chars[index..identifier_end].iter().collect();
            if matches!(identifier.as_str(), "r" | "br" | "cr") {
                rust_raw_string_end(chars, identifier_end).unwrap_or(identifier_end)
            } else {
                identifier_end
            }
        } else {
            index + 1
        };
        out.extend(&chars[index..end]);
        index = end;
    }
    out
}

/// The index just after a nested block comment that opens at `start`, or the end.
fn rust_block_comment_end(chars: &[char], start: usize) -> usize {
    let mut depth = 0;
    let mut index = start;
    while index < chars.len() {
        if starts_with_at(chars, index, "/*") {
            depth += 1;
            index += 2;
        } else if starts_with_at(chars, index, "*/") {
            depth -= 1;
            index += 2;
            if depth == 0 {
                return index;
            }
        } else {
            index += 1;
        }
    }
    chars.len()
}

/// The index just after a character literal that opens at `start`. A lifetime or a label
/// (`'a`) is not a literal: only its quote is consumed.
fn rust_char_literal_end(chars: &[char], start: usize) -> usize {
    if chars.get(start + 1) == Some(&'\\') {
        // An escape such as '\'', '\\', '\n' or '\u{1F600}'
        return end_after(chars, start + 3, "'");
    }
    if chars.get(start + 2) == Some(&'\'') {
        return start + 3;
    }
    start + 1
}

/// The index just after a raw string whose hashes or quote start at `after_prefix` (after
/// `r`, `br` or `cr`), or `None` if none starts there (for example the raw identifier
/// `r#type`).
fn rust_raw_string_end(chars: &[char], after_prefix: usize) -> Option<usize> {
    let hashes = (after_prefix..chars.len())
        .take_while(|index| chars[*index] == '#')
        .count();
    let quote = after_prefix + hashes;
    if chars.get(quote) != Some(&'"') {
        return None;
    }
    let closing = format!("\"{}", "#".repeat(hashes));
    Some(end_after(chars, quote + 1, &closing))
}

fn css_without_comments(chars: &[char]) -> String {
    let mut out = String::with_capacity(chars.len());
    let mut index = 0;
    while index < chars.len() {
        let c = chars[index];
        let end = if starts_with_at(chars, index, "/*") {
            let end = end_after(chars, index + 2, "*/");
            push_comment(&mut out, &chars[index..end]);
            index = end;
            continue;
        } else if c == '"' || c == '\'' {
            quoted_end(chars, index, c, true)
        } else {
            index + 1
        };
        out.extend(&chars[index..end]);
        index = end;
    }
    out
}

fn html_without_comments(chars: &[char]) -> String {
    let mut out = String::with_capacity(chars.len());
    let mut index = 0;
    while index < chars.len() {
        if starts_with_at(chars, index, "<!--") {
            let end = end_after(chars, index + 4, "-->");
            push_comment(&mut out, &chars[index..end]);
            index = end;
        } else {
            out.push(chars[index]);
            index += 1;
        }
    }
    out
}

fn toml_without_comments(chars: &[char]) -> String {
    let mut out = String::with_capacity(chars.len());
    let mut index = 0;
    while index < chars.len() {
        let c = chars[index];
        let end = if c == '#' {
            let end = line_end(chars, index);
            push_comment(&mut out, &chars[index..end]);
            index = end;
            continue;
        } else if starts_with_at(chars, index, "\"\"\"") {
            toml_multiline_end(chars, index, '"', true)
        } else if starts_with_at(chars, index, "'''") {
            toml_multiline_end(chars, index, '\'', false)
        } else if c == '"' {
            quoted_end(chars, index, '"', true)
        } else if c == '\'' {
            quoted_end(chars, index, '\'', false)
        } else {
            index + 1
        };
        out.extend(&chars[index..end]);
        index = end;
    }
    out
}

/// The index just after a TOML multi-line string that opens at `start` with three `quote`s.
fn toml_multiline_end(chars: &[char], start: usize, quote: char, escapes: bool) -> usize {
    let closing: String = [quote; 3].iter().collect();
    let mut index = start + 3;
    while index < chars.len() {
        if escapes && chars[index] == '\\' {
            index += 2;
        } else if starts_with_at(chars, index, &closing) {
            return index + 3;
        } else {
            index += 1;
        }
    }
    chars.len()
}

// ---------------------------------------------------------------------------------------
// Helpers for the tests
// ---------------------------------------------------------------------------------------

fn repo_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crafting-core must live inside the repository root")
        .join(relative)
}

/// All files below `dir`, skipping the given direct subdirectories (build output).
fn files_below(dir: &Path, skipped: &[&str]) -> Vec<PathBuf> {
    fn collect(root: &Path, dir: &Path, skipped: &[&str], out: &mut Vec<PathBuf>) {
        let entries =
            fs::read_dir(dir).unwrap_or_else(|e| panic!("cannot list {}: {e}", dir.display()));
        for entry in entries {
            let path = entry
                .unwrap_or_else(|e| panic!("cannot read entry in {}: {e}", dir.display()))
                .path();
            let relative = path.strip_prefix(root).expect("path lies under its root");
            if skipped.iter().any(|skip| relative == Path::new(skip)) {
                continue;
            }
            if path.is_dir() {
                collect(root, &path, skipped, out);
            } else {
                out.push(path);
            }
        }
    }
    let mut files = Vec::new();
    collect(dir, dir, skipped, &mut files);
    files
}

/// Asserts that the sample's code (without comments) contains the game text.
fn assert_found(extension: &str, sample: &str) {
    assert!(
        without_comments(extension, sample).contains(GAME_TEXT),
        "{GAME_TEXT:?} is in the code of this .{extension} sample and must be found:\n{sample}"
    );
}

/// Asserts that the sample's code (without comments) does not contain the game text.
fn assert_not_found(extension: &str, sample: &str) {
    let code = without_comments(extension, sample);
    assert!(
        !code.contains(GAME_TEXT),
        "{GAME_TEXT:?} is only in a comment of this .{extension} sample and must not be \
         found:\n{sample}\nwithout comments:\n{code}"
    );
}

/// Asserts that the game text of the samples is a currency name in the game data.
fn assert_game_text_is_game_data() {
    let path = repo_path(CURRENCIES_JSON);
    let content =
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    let data: Value = serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("{CURRENCIES_JSON} is not valid JSON: {e}"));
    let name = data
        .get("currencies")
        .and_then(Value::as_array)
        .and_then(|entries| {
            entries
                .iter()
                .find(|entry| entry.get("id").and_then(Value::as_str) == Some(GAME_TEXT_ID))
        })
        .and_then(|entry| entry.get("name"))
        .and_then(Value::as_str);
    assert_eq!(
        name,
        Some(GAME_TEXT),
        "the samples use the name of {GAME_TEXT_ID:?} in {CURRENCIES_JSON}"
    );
}

// ---------------------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------------------

// AC2: „Die Akzeptanztests, die prüfen, dass ein Spieltext nur in den Spieldaten steht, werten
// Kommentare nicht mehr aus."
// (Here: the shared helper on sample files of every file type under crafting-core/src and web/.
// The adapted checks in the other story files call it.)
#[test]
fn ac2_game_text_checks_search_files_without_their_comments() {
    // Given sample files with the game text only in comments, and files without comment syntax
    assert_game_text_is_game_data();
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("story_326");
    fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("cannot create {}: {e}", dir.display()));
    let only_in_comments = [
        (
            "comments.rs",
            format!(
                "//! The {GAME_TEXT}.\n/// The {GAME_TEXT}.\n// The {GAME_TEXT}.\n\
                 /** The {GAME_TEXT}. */\nfn f() {{ /* The {GAME_TEXT}. */ }}\n"
            ),
        ),
        (
            "comments.css",
            format!(".a {{ /* The {GAME_TEXT}. */ color: red; }}\n"),
        ),
        (
            "comments.html",
            format!("<body><!-- The {GAME_TEXT}. --></body>\n"),
        ),
        (
            "comments.toml",
            format!("[package] # The {GAME_TEXT}.\nname = \"x\"\n"),
        ),
    ];
    let in_full = [
        (
            "translations.json",
            format!("{{ \"x\": \"// {GAME_TEXT}\" }}\n"),
        ),
        ("privacy.md", format!("<!-- {GAME_TEXT} -->\n")),
    ];
    let mut commented: Vec<PathBuf> = Vec::new();
    for (name, content) in &only_in_comments {
        let path = dir.join(name);
        fs::write(&path, content).unwrap_or_else(|e| panic!("cannot write {name}: {e}"));
        commented.push(path);
    }
    let mut full: Vec<PathBuf> = Vec::new();
    for (name, content) in &in_full {
        let path = dir.join(name);
        fs::write(&path, content).unwrap_or_else(|e| panic!("cannot write {name}: {e}"));
        full.push(path);
    }

    // When the check searches the files for the game text
    let in_comments = files_containing_in_code(&commented, GAME_TEXT);
    let in_other_files = files_containing_in_code(&full, GAME_TEXT);
    let findings = findings_in_code(&[commented, full.clone()].concat(), &[GAME_TEXT]);

    // Then no file is found for a comment
    assert!(
        in_comments.is_empty(),
        "comments must not be searched: {in_comments:#?}"
    );
    // And files without comment syntax count in full
    assert_eq!(
        in_other_files.len(),
        in_full.len(),
        "files without comment syntax count in full: {in_other_files:#?}"
    );
    // And the findings name exactly those files with the game text
    let expected: Vec<String> = full
        .iter()
        .map(|path| format!("{} contains {GAME_TEXT:?}", path.display()))
        .collect();
    assert_eq!(findings, expected);
}

// AC2 (see above): removing the comments keeps every line of the real files in place, so the
// scanner reads all of crafting-core/src and web/ to the end.
#[test]
fn ac2_real_sources_keep_their_lines_without_comments() {
    // Given every file under crafting-core/src and web/ (without build output)
    let files = [
        files_below(&repo_path("crafting-core").join("src"), &[]),
        files_below(&repo_path("web"), &["dist", "target"]),
    ]
    .concat();
    assert!(
        files
            .iter()
            .any(|path| path.extension().is_some_and(|e| e == "rs")),
        "the scan must find Rust files"
    );

    for path in &files {
        // When its comments are removed
        let bytes =
            fs::read(path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        let content = String::from_utf8_lossy(&bytes);
        let code = code_of(path);

        // Then it keeps its number of lines and gets no longer
        assert_eq!(
            code.matches('\n').count(),
            content.matches('\n').count(),
            "{} must keep its lines",
            path.display()
        );
        assert!(
            code.chars().count() <= content.chars().count(),
            "{} must not grow",
            path.display()
        );
    }
}

// AC3: „Nachweis: Ein Spieltext als Text-Literal im Code lässt die Prüfung weiterhin
// fehlschlagen, derselbe Text in einem Kommentar nicht. Beides ist lokal ausprobiert und im Kopf
// der Testdatei beschrieben."
// (The local trial is described in the head of this file. Here: Rust samples.)
#[test]
fn ac3_rust_literal_is_found_and_the_same_text_in_a_comment_is_not() {
    assert_game_text_is_game_data();

    // Given a game text as a literal, Then it is found
    assert_found("rs", &format!("const NAME: &str = \"{GAME_TEXT}\";"));
    assert_found("rs", &format!("let name = b\"{GAME_TEXT}\";"));
    assert_found("rs", &format!("let name = c\"{GAME_TEXT}\";"));
    assert_found("rs", &format!("let name = r\"{GAME_TEXT}\";"));
    assert_found("rs", &format!("let name = br#\"{GAME_TEXT}\"#;"));
    assert_found("rs", &format!("let r#type = \"{GAME_TEXT}\";"));

    // Given the same text in a comment, Then it is not found
    assert_not_found("rs", &format!("// The {GAME_TEXT}.\nfn f() {{}}"));
    assert_not_found("rs", &format!("/// Applies the {GAME_TEXT}.\nfn f() {{}}"));
    assert_not_found("rs", &format!("//! The {GAME_TEXT}.\nfn f() {{}}"));
    assert_not_found("rs", &format!("fn f() {{}} /* The {GAME_TEXT}. */"));
    assert_not_found("rs", &format!("/** The\n{GAME_TEXT}.\n*/\nfn f() {{}}"));
    assert_not_found("rs", &format!("let r#type = 1; // The {GAME_TEXT}."));
    // And a nested block comment ends only at its last `*/`
    assert_not_found(
        "rs",
        &format!("/* outer /* inner */ The {GAME_TEXT}. */ fn f() {{}}"),
    );

    // And case matters
    assert!(!without_comments("rs", "let name = \"chaos orb\";").contains(GAME_TEXT));

    // And a comment becomes one space and keeps its line breaks
    assert_eq!(
        without_comments("rs", "a /* x\ny */ b // z\nc"),
        "a  \n b  \nc"
    );
}

// AC3 (see above): comment markers inside literals start no comment, and quotes inside
// comments and character literals start no string.
#[test]
fn ac3_rust_comment_markers_inside_literals_hide_no_code() {
    // Given a URL with `//` and the game text in the same string, Then it is found
    assert_found(
        "rs",
        &format!("let url = \"https://example.com/{GAME_TEXT}\";"),
    );
    // Given `/*` in a string before a literal, Then the literal is found
    assert_found(
        "rs",
        &format!("let a = \"/*\"; let b = \"{GAME_TEXT}\"; let c = \"*/\";"),
    );
    assert_found("rs", &format!("let a = \"/* {GAME_TEXT} */\";"));
    // Given escaped quotes in a string, Then the string goes on to its real end
    assert_found("rs", &format!("let a = \"say \\\"// {GAME_TEXT}\\\"\";"));
    assert_not_found("rs", &format!("let a = \"back\\\\\"; // {GAME_TEXT}"));
    // Given a lifetime before a string, Then the string is found
    assert_found(
        "rs",
        &format!("fn f<'a>(x: &'a str) -> &'a str {{ \"{GAME_TEXT}\" }}"),
    );
    assert_found("rs", &format!("let name: &'static str = \"{GAME_TEXT}\";"));
    assert_found(
        "rs",
        &format!("'outer: loop {{ break 'outer; }} let a = \"{GAME_TEXT}\";"),
    );
    // And a comment after a lifetime is still a comment
    assert_not_found("rs", &format!("struct S<'a>(&'a str); // {GAME_TEXT}"));
    assert_not_found(
        "rs",
        &format!("fn f(x: &'static str) -> &str {{ x }} // {GAME_TEXT}"),
    );
    // Given raw strings with `#` that contain quotes and comment markers, Then they are found
    assert_found("rs", &format!("let a = r#\"say \"hi\" // {GAME_TEXT}\"#;"));
    assert_found(
        "rs",
        &format!("let a = r#\"a \"// \"#; let b = \"{GAME_TEXT}\";"),
    );
    assert_found(
        "rs",
        &format!("let a = r##\"x \"# /* \"##; let b = \"{GAME_TEXT}\";"),
    );
    assert_found("rs", &format!("let a = br#\"/* {GAME_TEXT}\"#;"));
    assert_found("rs", &format!("let a = b\"// {GAME_TEXT}\";"));
    // And a comment after a raw string is still a comment
    assert_not_found("rs", &format!("let a = r#\"// \"#; // {GAME_TEXT}"));
    // Given character literals with quotes, Then they open no string
    assert_not_found("rs", &format!("let q = '\"'; // {GAME_TEXT}"));
    assert_found("rs", &format!("let q = '\"'; let a = \"{GAME_TEXT}\";"));
    assert_found("rs", &format!("let q = '\\''; let a = \"{GAME_TEXT}\";"));
    assert_not_found("rs", &format!("let q = b'\"'; // {GAME_TEXT}"));
    // Given a block comment between two literals, Then the second literal is found
    assert_found(
        "rs",
        &format!("let a = \"x\"; /* a /* b */ c */ let b = \"{GAME_TEXT}\";"),
    );
    // Given a quote in a comment, Then it opens no string
    assert_found("rs", &format!("// say \"\nlet a = \"{GAME_TEXT}\";"));
}

// AC3 (see above): CSS samples.
#[test]
fn ac3_css_literal_is_found_and_the_same_text_in_a_comment_is_not() {
    // Given a game text in the code, Then it is found
    assert_found("css", &format!(".a::after {{ content: \"{GAME_TEXT}\"; }}"));
    assert_found("css", &format!(".a::after {{ content: '{GAME_TEXT}'; }}"));
    // And `//` is no comment in CSS
    assert_found("css", &format!(".a {{ color: red; }} // {GAME_TEXT}"));
    // And `/*` in a string starts no comment
    assert_found(
        "css",
        &format!(".a {{ content: \"/*\"; }} .b {{ content: \"{GAME_TEXT}\"; }}"),
    );
    assert_found(
        "css",
        &format!(".a {{ content: '\\'/*'; }} .b {{ content: \"{GAME_TEXT}\"; }}"),
    );

    // Given the same text in a comment, Then it is not found
    assert_not_found(
        "css",
        &format!("/* The {GAME_TEXT}. */\n.a {{ color: red; }}"),
    );
    assert_not_found(
        "css",
        &format!(".a {{\n  /* The\n  {GAME_TEXT}. */\n  color: red;\n}}"),
    );
    assert_not_found(
        "css",
        &format!(".a {{ content: \"*/\"; }} /* {GAME_TEXT} */"),
    );
}

// AC3 (see above): HTML samples.
#[test]
fn ac3_html_literal_is_found_and_the_same_text_in_a_comment_is_not() {
    // Given a game text in the code, Then it is found
    assert_found("html", &format!("<title>{GAME_TEXT}</title>"));
    assert_found("html", &format!("<p>// {GAME_TEXT}</p>"));
    assert_found("html", &format!("<!-- a --><p>{GAME_TEXT}</p>"));

    // Given the same text in a comment, Then it is not found
    assert_not_found("html", &format!("<body><!-- The {GAME_TEXT}. --></body>"));
    assert_not_found(
        "html",
        &format!("<!--\n  The {GAME_TEXT}.\n-->\n<body></body>"),
    );
}

// AC3 (see above): TOML samples.
#[test]
fn ac3_toml_literal_is_found_and_the_same_text_in_a_comment_is_not() {
    // Given a game text in the code, Then it is found
    assert_found("toml", &format!("description = \"{GAME_TEXT}\""));
    assert_found("toml", &format!("description = '{GAME_TEXT}'"));
    // And `#` in a string starts no comment
    assert_found("toml", &format!("description = \"# {GAME_TEXT}\""));
    assert_found("toml", &format!("description = 'C:\\# {GAME_TEXT}'"));
    assert_found(
        "toml",
        &format!("description = \"\"\"\n# {GAME_TEXT}\n\"\"\""),
    );
    assert_found("toml", &format!("description = '''\n# {GAME_TEXT}\n'''"));
    assert_found("toml", &format!("a = \"say \\\" # \"\nb = \"{GAME_TEXT}\""));

    // Given the same text in a comment, Then it is not found
    assert_not_found("toml", &format!("# The {GAME_TEXT}.\n[package]"));
    assert_not_found("toml", &format!("name = \"x\" # The {GAME_TEXT}."));
    assert_not_found("toml", &format!("name = 'a # b' # The {GAME_TEXT}."));
    assert_not_found(
        "toml",
        &format!("name = \"esc \\\" # \" # The {GAME_TEXT}."),
    );
    assert_not_found("toml", &format!("a = '''x'''\n# The {GAME_TEXT}."));
}

// AC3 (see above): every other file counts in full.
#[test]
fn ac3_files_without_comment_syntax_count_in_full() {
    // Given texts that look like comments in other file types, Then they are found
    assert_found("json", &format!("{{ \"x\": \"// {GAME_TEXT}\" }}"));
    assert_found("md", &format!("<!-- {GAME_TEXT} -->"));
    assert_found("md", &format!("# {GAME_TEXT}"));
    assert_found("", &format!("/* {GAME_TEXT} */"));
}
