//! Acceptance tests for AB#39 "Datenschutzerklärung auf der Website (DE/EN)".
//!
//! This story has no `crafting-core` behaviour. Following ADR 0004 and ADR 0007, only
//! lean text-level checks are made here: a file exists, contains or does not contain a
//! given string, a line with a given text exists. There are no JSON, TOML or Markdown
//! parsers.
//!
//! The scans cover every file under `web/` except the build output in `web/dist` and
//! `web/target`. This test file lives outside `web/`, so its own strings never satisfy or
//! break a scan.
//!
//! Red state before the implementation: AC1, AC2, AC3 and AC7 fail. AC4, AC5 and AC6
//! guard properties that already hold (no e-mail link, nothing loaded from third-party
//! servers, no browser storage) and may pass from the start.
//!
//! What is checked outside `cargo test`:
//! - The rendering in the browser, the real network requests and the real browser storage
//!   (release build check in AB#46, PO verification after the merge in AB#47).
//! - AC3: that the texts are byte-for-byte the PO's originals. The PO checks this in the
//!   diff of the commit that adds them. Here: only the source note, the binding-version
//!   note and that the web code embeds exactly these files.
//! - AC8: that the privacy page is reachable on GitHub Pages after the merge
//!   (verification task AB#47). There is no test for it in this file on purpose.

use std::fs;
use std::path::{Path, PathBuf};

const EN_JSON: &str = "web/i18n/en.json";
const WEB_CARGO_TOML: &str = "web/Cargo.toml";
const PRIVACY_DE: &str = "web/legal/privacy.de.md";
const PRIVACY_EN: &str = "web/legal/privacy.en.md";
const CLAUDE_MD: &str = "CLAUDE.md";
const REVIEWING_CODE: &str = ".claude/skills/reviewing-code/SKILL.md";

/// Relative links of the two privacy pages (ADR 0007).
const ENGLISH_PAGE_LINK: &str = "?page=privacy";
const GERMAN_PAGE_LINK: &str = "?page=privacy-de";

/// How the web code refers to the PO's files.
const DE_FILE_REFERENCE: &str = "legal/privacy.de.md";
const EN_FILE_REFERENCE: &str = "legal/privacy.en.md";

/// Notes that must survive unchanged (AC3).
const GERMAN_SOURCE_NOTE: &str = "Quelle: https://www.e-recht24.de";
const ENGLISH_SOURCE_NOTE: &str = "Source: https://www.e-recht24.de";
const BINDING_NOTE: &str = "Only the German version is legally binding.";

/// The only allowed form of the e-mail address (AC4).
const OBFUSCATED_ADDRESS: &str = "poe-crafting-sim [at] gmx.de";

/// Forbidden forms of the e-mail address in delivered files (AC4).
const FORBIDDEN_ADDRESS_FORMS: [&str; 2] = ["mailto:", "poe-crafting-sim@"];

/// Markdown and HTML that embeds content instead of linking it (AC5), lower case.
const EMBEDDING_MARKUP: [&str; 11] = [
    "![", "<img", "<iframe", "<script", "<link", "<object", "<embed", "<video", "<audio",
    "<source", "<style",
];

/// Absolute and protocol-relative addresses, which point to other servers (AC5).
const EXTERNAL_ADDRESSES: [&str; 9] = [
    "http://",
    "https://",
    "src=\"//",
    "src='//",
    "href=\"//",
    "href='//",
    "url(//",
    "url(\"//",
    "url('//",
];

/// Browser storage in any spelling of JavaScript or Rust (AC6), lower case.
const BROWSER_STORAGE: [&str; 7] = [
    "cookie",
    "localstorage",
    "sessionstorage",
    "indexeddb",
    "local_storage",
    "session_storage",
    "indexed_db",
];

/// `web-sys` features that give access to browser storage (AC6).
const STORAGE_FEATURES: [&str; 2] = ["\"Storage\"", "\"IdbFactory\""];

/// New hard rule in `CLAUDE.md` (AC7), plain ASCII apostrophe.
const PRIVACY_RULE: &str = "The website loads nothing from third-party servers and stores \
    nothing in the visitor's browser. Any change to that first needs a privacy policy \
    updated by the PO.";

/// New critical criterion in the review checklist (AC7), plain ASCII apostrophe.
const REVIEW_CRITERION: &str = "loads something from a third-party server or stores \
    something in the visitor's browser";

fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crafting-core must live inside the repository root")
}

fn repo_path(relative: &str) -> PathBuf {
    repo_root().join(relative)
}

fn read_path(path: &Path) -> String {
    let bytes = fs::read(path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    String::from_utf8_lossy(&bytes).into_owned()
}

fn read_text(relative: &str) -> String {
    let path = repo_path(relative);
    assert!(path.is_file(), "{relative} must exist");
    read_path(&path)
}

/// Collapses every run of whitespace, including line breaks, into one space.
fn normalize_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// All files under `web/`, except build output in `web/dist` and `web/target`.
fn web_files() -> Vec<PathBuf> {
    fn collect(web: &Path, dir: &Path, out: &mut Vec<PathBuf>) {
        let entries =
            fs::read_dir(dir).unwrap_or_else(|e| panic!("cannot list {}: {e}", dir.display()));
        for entry in entries {
            let path = entry
                .unwrap_or_else(|e| panic!("cannot read entry in {}: {e}", dir.display()))
                .path();
            let relative = path.strip_prefix(web).expect("path lies under web/");
            if relative == Path::new("dist") || relative == Path::new("target") {
                continue;
            }
            if path.is_dir() {
                collect(web, &path, out);
            } else {
                out.push(path);
            }
        }
    }
    let web = repo_path("web");
    let mut files = Vec::new();
    collect(&web, &web, &mut files);
    assert!(
        files
            .iter()
            .any(|path| path.extension().is_some_and(|e| e == "rs")),
        "web/ must contain Rust source files; the scan found none"
    );
    files
}

/// All files under `web/` outside `web/legal/` (and outside the build output).
fn web_files_outside_legal() -> Vec<PathBuf> {
    let legal = repo_path("web").join("legal");
    web_files()
        .into_iter()
        .filter(|path| !path.starts_with(&legal))
        .collect()
}

/// All files under `web/src/`.
fn web_src_files() -> Vec<PathBuf> {
    let src = repo_path("web").join("src");
    let files: Vec<PathBuf> = web_files()
        .into_iter()
        .filter(|path| path.starts_with(&src))
        .collect();
    assert!(!files.is_empty(), "web/src/ must contain files");
    files
}

/// Files whose text contains `needle`; with `ignore_case`, both are compared in lower case.
fn files_containing(files: &[PathBuf], needle: &str, ignore_case: bool) -> Vec<String> {
    files
        .iter()
        .filter(|path| {
            let text = read_path(path);
            if ignore_case {
                text.to_lowercase().contains(&needle.to_lowercase())
            } else {
                text.contains(needle)
            }
        })
        .map(|path| path.display().to_string())
        .collect()
}

/// True if `text` contains `link` and the next character does not continue it, so that
/// `?page=privacy` is not satisfied by `?page=privacy-de`.
fn contains_whole_link(text: &str, link: &str) -> bool {
    text.match_indices(link).any(|(at, _)| {
        text[at + link.len()..]
            .chars()
            .next()
            .is_none_or(|c| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
    })
}

/// Raw value after the key on its line in `web/i18n/en.json`, including the quotes and
/// without a trailing comma, for example `"Privacy"`.
fn translation_value(key: &str) -> String {
    let text = read_text(EN_JSON);
    let quoted_key = format!("\"{key}\"");
    let line = text
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with(&quoted_key))
        .unwrap_or_else(|| panic!("{EN_JSON} must have a line with the key {quoted_key}"));
    let (_, value) = line[quoted_key.len()..]
        .split_once(':')
        .unwrap_or_else(|| panic!("{EN_JSON}: the key {quoted_key} must have a value: {line}"));
    value.trim().trim_end_matches(',').trim_end().to_owned()
}

// AC1: „Der Footer enthält auf jeder Seite einen Link „Privacy“ zur englischen
// Datenschutzseite."
#[test]
fn ac1_footer_links_privacy_to_the_english_policy() {
    // Given the English translation file and the web code
    let src_files = web_src_files();

    // When looking up the footer link text and the link to the English policy
    let link_text = translation_value("footer.privacy");
    let linking_files: Vec<String> = src_files
        .iter()
        .filter(|path| contains_whole_link(&read_path(path), ENGLISH_PAGE_LINK))
        .map(|path| path.display().to_string())
        .collect();

    // Then the footer link text is exactly "Privacy"
    assert_eq!(
        link_text, "\"Privacy\"",
        "{EN_JSON}: `footer.privacy` must be exactly \"Privacy\""
    );
    // And the web code links to the English policy page
    assert!(
        !linking_files.is_empty(),
        "a file under web/src/ must contain the link `{ENGLISH_PAGE_LINK}` \
         (not only `{GERMAN_PAGE_LINK}`)"
    );
}

// AC2: „Es gibt eine englische und eine deutsche Datenschutzseite. Beide verlinken
// gegenseitig aufeinander."
#[test]
fn ac2_english_and_german_policies_link_to_each_other() {
    // Given the English translation file and the web code
    let src_files = web_src_files();

    // When looking up the cross-link texts and the links to both pages
    let german_link_text = translation_value("privacy.german_version");
    let english_link_text = translation_value("privacy.english_version");
    let links_to_german = files_containing(&src_files, GERMAN_PAGE_LINK, false);
    let links_to_english: Vec<&PathBuf> = src_files
        .iter()
        .filter(|path| contains_whole_link(&read_path(path), ENGLISH_PAGE_LINK))
        .collect();

    // Then both cross-link texts exist and are not empty
    for (key, value) in [
        ("privacy.german_version", &german_link_text),
        ("privacy.english_version", &english_link_text),
    ] {
        assert!(
            value.len() > 2 && value.starts_with('"') && value.ends_with('"'),
            "{EN_JSON}: `{key}` must be a non-empty string, found {value}"
        );
    }
    // And the web code links to the German page
    assert!(
        !links_to_german.is_empty(),
        "a file under web/src/ must contain the link `{GERMAN_PAGE_LINK}`"
    );
    // And the web code links back to the English page
    assert!(
        !links_to_english.is_empty(),
        "a file under web/src/ must contain the link `{ENGLISH_PAGE_LINK}`"
    );
}

// AC3: „Beide Texte werden inhaltlich unverändert aus den Dateien des PO übernommen;
// erlaubt ist nur eine Umwandlung in HTML-Markup. Die Quellenangabe für eRecht24 und der
// Hinweis „Only the German version is legally binding“ bleiben erhalten."
#[test]
fn ac3_policies_are_embedded_unchanged_from_the_po_files() {
    // Given the PO's two policy files and the web code
    let german = read_text(PRIVACY_DE);
    let english = read_text(PRIVACY_EN);
    let src_files = web_src_files();
    let english_path = repo_path(PRIVACY_EN);

    // When looking for the notes, the references to the files and copies of the text
    let embeds_german = files_containing(&src_files, DE_FILE_REFERENCE, false);
    let embeds_english = files_containing(&src_files, EN_FILE_REFERENCE, false);
    let other_files: Vec<PathBuf> = web_files()
        .into_iter()
        .filter(|path| *path != english_path)
        .collect();
    let copies = files_containing(&other_files, BINDING_NOTE, false);

    // Then the German text keeps its eRecht24 source note
    assert!(
        german.contains(GERMAN_SOURCE_NOTE),
        "{PRIVACY_DE} must contain {GERMAN_SOURCE_NOTE:?}"
    );
    // And the English text keeps its source note and the binding-version note
    assert!(
        english.contains(ENGLISH_SOURCE_NOTE),
        "{PRIVACY_EN} must contain {ENGLISH_SOURCE_NOTE:?}"
    );
    assert!(
        english.contains(BINDING_NOTE),
        "{PRIVACY_EN} must contain {BINDING_NOTE:?}"
    );
    // And the web code takes both texts from the PO's files
    assert!(
        !embeds_german.is_empty(),
        "a file under web/src/ must refer to `{DE_FILE_REFERENCE}`"
    );
    assert!(
        !embeds_english.is_empty(),
        "a file under web/src/ must refer to `{EN_FILE_REFERENCE}`"
    );
    // And no other file under web/ holds a copy of the text
    assert!(
        copies.is_empty(),
        "{BINDING_NOTE:?} must appear only in {PRIVACY_EN}, also found in: {copies:#?}"
    );
}

// AC4: „Die E-Mail-Adresse erscheint nur als `poe-crafting-sim [at] gmx.de`. In den
// ausgelieferten Dateien gibt es keinen `mailto:`-Link und die Adresse nicht im Klartext
// mit `@`."
#[test]
fn ac4_email_address_appears_only_obfuscated() {
    // Given the PO's two policy files and all files under web/
    let german = read_text(PRIVACY_DE);
    let english = read_text(PRIVACY_EN);
    let files = web_files();

    // When looking for the address in plain text or as an e-mail link
    let forbidden: Vec<String> = FORBIDDEN_ADDRESS_FORMS
        .iter()
        .flat_map(|form| {
            files_containing(&files, form, true)
                .into_iter()
                .map(move |file| format!("{file} contains `{form}`"))
        })
        .collect();

    // Then both policies give the address in the obfuscated form
    assert!(
        german.contains(OBFUSCATED_ADDRESS),
        "{PRIVACY_DE} must contain {OBFUSCATED_ADDRESS:?}"
    );
    assert!(
        english.contains(OBFUSCATED_ADDRESS),
        "{PRIVACY_EN} must contain {OBFUSCATED_ADDRESS:?}"
    );
    // And no file under web/ has an e-mail link or the address with `@`
    assert!(
        forbidden.is_empty(),
        "no file under web/ may contain an e-mail link or the plain address: {forbidden:#?}"
    );
}

// AC5: „Die Website lädt keine Ressourcen von Drittservern. Externe Adressen im Text sind
// nur normale Links, keine eingebetteten Inhalte."
#[test]
fn ac5_website_loads_no_resources_from_third_party_servers() {
    // Given the PO's two policy files and all other files under web/
    let policies = [
        (PRIVACY_DE, read_text(PRIVACY_DE).to_lowercase()),
        (PRIVACY_EN, read_text(PRIVACY_EN).to_lowercase()),
    ];
    let other_files = web_files_outside_legal();
    let stylesheets: Vec<PathBuf> = web_files()
        .into_iter()
        .filter(|path| path.extension().is_some_and(|e| e == "css"))
        .collect();

    // When looking for embedded content and addresses of other servers
    let embedded: Vec<String> = policies
        .iter()
        .flat_map(|(file, text)| {
            EMBEDDING_MARKUP
                .iter()
                .filter(|markup| text.contains(*markup))
                .map(move |markup| format!("{file} contains `{markup}`"))
        })
        .collect();
    let external: Vec<String> = EXTERNAL_ADDRESSES
        .iter()
        .flat_map(|address| {
            files_containing(&other_files, address, true)
                .into_iter()
                .map(move |file| format!("{file} contains `{address}`"))
        })
        .collect();
    let imports = files_containing(&stylesheets, "@import", true);

    // Then the policies only mention addresses and embed nothing
    assert!(
        embedded.is_empty(),
        "the policies in web/legal/ must not embed images or other content: {embedded:#?}"
    );
    // And no other file under web/ points to another server
    assert!(
        external.is_empty(),
        "outside web/legal/, no file under web/ may contain an absolute or \
         protocol-relative address: {external:#?}"
    );
    // And no stylesheet imports another stylesheet
    assert!(
        imports.is_empty(),
        "stylesheets under web/ must not use `@import`: {imports:#?}"
    );
}

// AC6: „Die Website setzt keine Cookies und nutzt keinen Browser-Speicher: weder Local
// Storage noch Session Storage noch IndexedDB."
#[test]
fn ac6_website_sets_no_cookies_and_uses_no_browser_storage() {
    // Given all files under web/ outside the policy texts, and the web crate manifest
    let files = web_files_outside_legal();
    let manifest = read_text(WEB_CARGO_TOML);

    // When looking for cookies and browser storage APIs
    let storage_uses: Vec<String> = BROWSER_STORAGE
        .iter()
        .flat_map(|api| {
            files_containing(&files, api, true)
                .into_iter()
                .map(move |file| format!("{file} contains `{api}`"))
        })
        .collect();
    let storage_features: Vec<&str> = STORAGE_FEATURES
        .iter()
        .copied()
        .filter(|feature| manifest.contains(feature))
        .collect();

    // Then no file mentions cookies, Local Storage, Session Storage or IndexedDB
    assert!(
        storage_uses.is_empty(),
        "outside web/legal/, no file under web/ may use cookies or browser storage: \
         {storage_uses:#?}"
    );
    // And the web crate does not enable the web-sys storage features
    assert!(
        storage_features.is_empty(),
        "{WEB_CARGO_TOML} must not enable the web-sys features {storage_features:?}"
    );
}

// AC7: „CLAUDE.md erhält eine neue harte Regel: „Die Website lädt nichts von Drittservern
// und speichert nichts im Browser des Besuchers. Jede Änderung daran braucht zuerst eine
// vom PO angepasste Datenschutzerklärung.“ Der Reviewer prüft diese Regel künftig mit."
#[test]
fn ac7_privacy_rule_is_a_hard_rule_and_a_review_criterion() {
    // Given the project instructions and the review checklist
    let claude_md = read_text(CLAUDE_MD);
    let reviewing_code = normalize_whitespace(&read_text(REVIEWING_CODE));

    // When reading the hard rules section
    let (_, after_heading) = claude_md
        .split_once("## Hard rules")
        .unwrap_or_else(|| panic!("{CLAUDE_MD} must have a section `## Hard rules`"));
    let hard_rules = after_heading.split("\n## ").next().unwrap_or(after_heading);
    let hard_rules = normalize_whitespace(hard_rules);
    let rule = normalize_whitespace(PRIVACY_RULE);
    let criterion = normalize_whitespace(REVIEW_CRITERION);

    // Then the privacy rule is a hard rule
    assert!(
        hard_rules.contains(&format!("- {rule}")),
        "{CLAUDE_MD} must have the hard rule `- {rule}` in its section `## Hard rules`"
    );
    // And the reviewer checks it
    assert!(
        reviewing_code.contains(&criterion),
        "{REVIEWING_CODE} must contain the criterion {criterion:?}"
    );
}
