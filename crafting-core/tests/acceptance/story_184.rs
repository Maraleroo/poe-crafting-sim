//! Acceptance tests for AB#184 "Dunkles Theme für die ganze Website".
//!
//! This story has no `crafting-core` behaviour. Following ADR 0004 and ADR 0014, only lean
//! text-level checks of the files under `web/` are made here (without the build output in
//! `web/dist` and `web/target`). Colour values and other contents of the stylesheet are not
//! checked word for word. This test file lives outside `web/`, so its own strings never
//! satisfy or break a scan.
//!
//! Red state before the implementation: none. AC6 says "weiterhin": it guards a property that
//! already holds (system fonts only, nothing from third-party servers), so its test passes
//! before and after the implementation.
//!
//! Criteria without a permanent test, checked by the Scrum Master and the PO in the local
//! preview (task AB#194, desktop and 375 px width), because they need a browser (ADR 0004):
//! - AC1 to AC5: the computed colours of every page, the colours by rarity, red only for
//!   errors, gold for links, hover and keyboard focus, and the contrast of every text.
//! - AC6 in the browser: the network tab shows no request to another server and no font.
//! - AC7: behaviour and layout of the page, buttons that stay in place while crafting.
//! - AC8: no horizontal scrolling at a width of 375 pixels.
//!
//! The existing checks for absolute addresses and `@import` (AB#39 AC5, AB#62 AC6) also
//! cover this story's AC6 and are not repeated here.

use std::fs;
use std::path::{Path, PathBuf};

/// File extensions of web fonts and font files, lower case and with the leading dot.
const FONT_EXTENSIONS: [&str; 5] = [".woff", ".woff2", ".ttf", ".otf", ".eot"];

/// The CSS rule that defines a font of its own instead of a system font, lower case.
const FONT_FACE: &str = "@font-face";

/// The CSS function that loads a resource such as a font or an image, lower case.
const CSS_URL: &str = "url(";

fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crafting-core must live inside the repository root")
}

fn read_path(path: &Path) -> String {
    let bytes = fs::read(path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    String::from_utf8_lossy(&bytes).into_owned()
}

/// All files under `web/`, except the build output in `web/dist` and `web/target`.
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
    let web = repo_root().join("web");
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

/// True if the file's extension is `extension` (without the dot), in any case.
fn has_extension(path: &Path, extension: &str) -> bool {
    path.extension()
        .is_some_and(|e| e.to_string_lossy().eq_ignore_ascii_case(extension))
}

/// For every file whose text contains one of `needles` (compared in lower case), one
/// finding "<file> contains `<needle>`".
fn findings(files: &[PathBuf], needles: &[&str]) -> Vec<String> {
    files
        .iter()
        .flat_map(|path| {
            let text = read_path(path).to_lowercase();
            needles
                .iter()
                .filter(move |needle| text.contains(*needle))
                .map(move |needle| format!("{} contains `{needle}`", path.display()))
                .collect::<Vec<_>>()
        })
        .collect()
}

// AC6: „Die Seite verwendet nur Systemschriften und lädt weiterhin nichts von fremden
// Servern."
#[test]
fn ac6_page_uses_only_system_fonts_and_loads_no_font_or_resource_from_stylesheets() {
    // Given all files under web/, among them the stylesheets and the HTML page
    let files = web_files();
    let stylesheets_and_pages: Vec<PathBuf> = files
        .iter()
        .filter(|path| has_extension(path, "css") || has_extension(path, "html"))
        .cloned()
        .collect();
    assert!(
        files.iter().any(|path| has_extension(path, "css")),
        "web/ must contain a stylesheet; the scan found none"
    );

    // When looking for fonts of their own, resources loaded by a stylesheet and font files
    let font_faces = findings(&files, &[FONT_FACE]);
    let loaded_resources = findings(&stylesheets_and_pages, &[CSS_URL]);
    let font_references = findings(&files, &FONT_EXTENSIONS);
    let font_files: Vec<String> = files
        .iter()
        .filter(|path| {
            let name = path.to_string_lossy().to_lowercase();
            FONT_EXTENSIONS
                .iter()
                .any(|extension| name.ends_with(extension))
        })
        .map(|path| path.display().to_string())
        .collect();

    // Then no file under web/ defines a font of its own
    assert!(
        font_faces.is_empty(),
        "no file under web/ may define a font with `@font-face`; the page uses only \
         system fonts: {font_faces:#?}"
    );
    // And no stylesheet and no HTML page loads a resource with `url(`
    assert!(
        loaded_resources.is_empty(),
        "stylesheets and HTML pages under web/ must not load anything with `url(`: \
         {loaded_resources:#?}"
    );
    // And there is no font file under web/
    assert!(
        font_files.is_empty(),
        "web/ must not contain font files: {font_files:#?}"
    );
    // And no file under web/ refers to a font file, for example one outside web/
    assert!(
        font_references.is_empty(),
        "no file under web/ may refer to a font file: {font_references:#?}"
    );
}
