//! Acceptance tests for AB#5 "Walking Skeleton: Hello Crafting ist live".
//!
//! Following ADR 0004, criteria with a file- or API-level contract are tested here:
//! the public API of `crafting-core` and the web translation file `web/i18n/en.json`.
//! Criteria 5, 6 and 7 need the CI system, the public URL or a local browser and are
//! checked outside `cargo test` (CI run on the PR, PO in the local preview and on the
//! published page). They have no test in this file on purpose.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

/// Exact heading text from criterion 1.
const TITLE: &str = "Hello Crafting";

/// Exact footer notice from criterion 3 (plain ASCII apostrophe).
const DISCLAIMER: &str =
    "This product isn't affiliated with or endorsed by Grinding Gear Games in any way.";

/// Footer template for the website version (AB#80), with the placeholder syntax of ADR 0002.
const CORE_VERSION_TEMPLATE: &str = "Version {version}";

/// Attributes whose values browsers show to the user (tooltips, alternative text,
/// input hints, screen reader labels).
const VISIBLE_ATTRIBUTES: [&str; 4] = ["alt", "title", "placeholder", "aria-label"];

fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crafting-core must live inside the repository root")
}

fn web_dir() -> PathBuf {
    repo_root().join("web")
}

fn en_json_path() -> PathBuf {
    web_dir().join("i18n").join("en.json")
}

fn index_html_path() -> PathBuf {
    web_dir().join("index.html")
}

fn read_text(path: &Path) -> String {
    let bytes = fs::read(path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Loads `web/i18n/en.json` and checks that it is a flat JSON object of strings.
fn load_english_translations() -> Map<String, Value> {
    let path = en_json_path();
    let content = read_text(&path);
    let value: Value = serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("{} is not valid JSON: {e}", path.display()));
    let Value::Object(map) = value else {
        panic!(
            "{} must contain a JSON object at the top level",
            path.display()
        );
    };
    for (key, value) in &map {
        assert!(
            value.is_string(),
            "{} must be a flat object of strings, but key {key:?} has value {value}",
            path.display()
        );
    }
    map
}

fn translation<'a>(translations: &'a Map<String, Value>, key: &str) -> &'a str {
    translations
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("web/i18n/en.json has no text for key {key:?}"))
}

/// All files under `web/`, except build output in `web/dist` and `web/target`.
fn web_source_files() -> Vec<PathBuf> {
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
    let web = web_dir();
    let mut files = Vec::new();
    collect(&web, &web, &mut files);
    files
}

/// Name of an opening tag in lower case, empty for closing tags, comments and doctype.
fn opening_tag_name(tag: &str) -> String {
    tag.trim_start_matches('<')
        .chars()
        .take_while(char::is_ascii_alphanumeric)
        .collect::<String>()
        .to_ascii_lowercase()
}

/// Non-empty values of visible attributes inside one tag.
fn visible_attribute_values(tag: &str) -> Vec<String> {
    let lower = tag.to_ascii_lowercase();
    let mut found = Vec::new();
    for attribute in VISIBLE_ATTRIBUTES {
        let pattern = format!("{attribute}=");
        let mut from = 0;
        while let Some(offset) = lower[from..].find(&pattern) {
            let at = from + offset;
            from = at + pattern.len();
            let starts_attribute = lower[..at].chars().last().is_some_and(char::is_whitespace);
            if !starts_attribute {
                continue;
            }
            let raw = &tag[from..];
            let value = match raw.chars().next() {
                Some(quote @ ('"' | '\'')) => raw[1..].split(quote).next().unwrap_or(""),
                _ => raw
                    .split(|c: char| c.is_whitespace() || c == '>')
                    .next()
                    .unwrap_or(""),
            };
            if !value.trim().is_empty() {
                found.push(format!("{attribute}=\"{value}\""));
            }
        }
    }
    found
}

/// Everything in an HTML document that a browser would show to the user:
/// text outside tags (without comments, scripts and styles, but including the
/// document title) and the values of visible attributes.
fn visible_html_content(html: &str) -> Vec<String> {
    let mut visible = Vec::new();
    let mut text = String::new();
    let mut rest = html;
    while let Some(start) = rest.find('<') {
        text.push_str(&rest[..start]);
        let markup = &rest[start..];
        if markup.starts_with("<!--") {
            let end = markup.find("-->").map_or(markup.len(), |i| i + 3);
            rest = &markup[end..];
            continue;
        }
        let end = markup.find('>').map_or(markup.len(), |i| i + 1);
        let tag = &markup[..end];
        rest = &markup[end..];
        visible.extend(visible_attribute_values(tag));
        let name = opening_tag_name(tag);
        let self_closing = tag.ends_with("/>");
        if (name == "script" || name == "style") && !self_closing {
            let closing = format!("</{name}");
            let raw_end = rest
                .to_ascii_lowercase()
                .find(&closing)
                .unwrap_or(rest.len());
            rest = &rest[raw_end..];
        }
    }
    text.push_str(rest);
    let text = text.trim();
    if !text.is_empty() {
        visible.push(text.to_owned());
    }
    visible
}

// AC1: „Gegeben der aktuelle Stand auf main, wenn ich die GitHub-Pages-Adresse des
// Projekts öffne, dann sehe ich eine Seite mit der Überschrift „Hello Crafting“."
#[test]
fn ac1_english_translation_defines_hello_crafting_heading() {
    // Given the English translation file of the web page
    let translations = load_english_translations();

    // When the page looks up its heading
    let heading = translation(&translations, "app.title");

    // Then the heading is "Hello Crafting"
    assert_eq!(heading, TITLE);
}

// AC2: „Gegeben die geöffnete Seite, wenn sie geladen ist, dann zeigt sie im Footer
// die Versionsnummer von crafting-core, die im Browser über WebAssembly abgefragt
// wird und nicht fest im HTML steht."
#[test]
fn ac2_core_reports_its_version_for_the_footer() {
    // Given crafting-core, the English translation file and the page's HTML
    let translations = load_english_translations();
    let index_html = read_text(&index_html_path());

    // When the page asks crafting-core for its version
    let version: &'static str = crafting_core::version();

    // Then crafting-core reports its own package version
    assert!(
        !version.is_empty(),
        "crafting-core version must not be empty"
    );
    assert_eq!(version, env!("CARGO_PKG_VERSION"));
    // And the footer text is a template that receives the version at runtime
    assert_eq!(
        translation(&translations, "footer.core_version"),
        CORE_VERSION_TEMPLATE
    );
    // And the version number is not written into the HTML
    assert!(
        !index_html.contains(version),
        "web/index.html must not contain the version {version:?}; it is queried at runtime"
    );
}

// AC3: „Gegeben die geöffnete Seite, wenn ich nach unten scrolle, dann sehe ich im
// Footer den Hinweis „This product isn't affiliated with or endorsed by Grinding Gear
// Games in any way.“"
#[test]
fn ac3_english_translation_defines_ggg_disclaimer() {
    // Given the English translation file of the web page
    let translations = load_english_translations();

    // When the page looks up the footer notice
    let disclaimer = translation(&translations, "footer.disclaimer");

    // Then it is exactly the required disclaimer
    assert_eq!(disclaimer, DISCLAIMER);
}

// AC4: „Gegeben der Code des Webteils, wenn ein Text sichtbar ist, dann stammt er aus
// einer englischen Übersetzungsdatei und steht nicht fest im Code."
#[test]
fn ac4_visible_texts_come_only_from_english_translation_file() {
    // Given the web part: its HTML page, its English translation file and all its files
    let index_html = read_text(&index_html_path());
    let translations = load_english_translations();
    let en_json = en_json_path();
    let files: Vec<PathBuf> = web_source_files()
        .into_iter()
        .filter(|path| *path != en_json)
        .collect();
    assert!(
        !files.is_empty(),
        "web/ must contain code besides the translation file"
    );

    // When looking for visible texts outside the translation file
    let visible_in_html = visible_html_content(&index_html);
    let required_texts = [
        translation(&translations, "app.title"),
        translation(&translations, "footer.disclaimer"),
    ];
    let mut hard_coded = Vec::new();
    for path in &files {
        let content = read_text(path);
        for text in [TITLE, DISCLAIMER] {
            if content.contains(text) {
                hard_coded.push(format!("{} contains {text:?}", path.display()));
            }
        }
    }

    // Then the required texts are in the translation file
    assert_eq!(required_texts, [TITLE, DISCLAIMER]);
    // And the HTML page shows no text of its own
    assert!(
        visible_in_html.is_empty(),
        "web/index.html must contain no visible text, found: {visible_in_html:?}"
    );
    // And no other file under web/ hard-codes a visible text
    assert!(
        hard_coded.is_empty(),
        "visible texts must come from web/i18n/en.json only: {hard_coded:#?}"
    );
}
