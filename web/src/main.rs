//! Entry point of the web page. Trunk compiles this crate to WebAssembly.
//!
//! The page is built at runtime from translated texts. The crate also compiles natively
//! so that `cargo clippy` and `cargo test` cover it; only `main` needs a browser.

mod i18n;
mod legal;
mod page;
mod route;

use i18n::Translations;
use page::PageText;
use route::Route;
use wasm_bindgen::JsValue;
use web_sys::{Document, Element};

fn main() {
    if let Err(error) = render() {
        web_sys::console::error_1(&error);
    }
}

/// Fills the document title and body for the requested page from the translations,
/// the legal texts and `crafting-core`. Every page gets the same footer.
fn render() -> Result<(), JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window available"))?;
    let document = window
        .document()
        .ok_or_else(|| JsValue::from_str("no document available"))?;
    let body = document
        .body()
        .ok_or_else(|| JsValue::from_str("document has no body"))?;
    let route = Route::from_search(&window.location().search()?);
    let text = PageText::new(&Translations::english(), crafting_core::version());

    let main = element(&document, "main", None)?;
    match route {
        Route::Home => {
            document.set_title(&text.title);
            append(&main, element(&document, "h1", Some(&text.title))?)?;
        }
        Route::PrivacyEnglish => {
            document.set_title(&text.privacy_title);
            let other = Route::PrivacyGerman;
            append(
                &main,
                language_link(&document, other, &text.german_version_link, "de")?,
            )?;
            let policy = element(&document, "article", None)?;
            policy.set_inner_html(&legal::privacy_english_html());
            append(&main, policy)?;
        }
        Route::PrivacyGerman => {
            document.set_title(&text.privacy_title);
            let other = Route::PrivacyEnglish;
            append(
                &main,
                language_link(&document, other, &text.english_version_link, "en")?,
            )?;
            let policy = element(&document, "article", None)?;
            policy.set_attribute("lang", "de")?;
            policy.set_inner_html(&legal::privacy_german_html());
            append(&main, policy)?;
        }
    }
    append(&body, main)?;

    let footer = element(&document, "footer", None)?;
    append(&footer, element(&document, "p", Some(&text.core_version))?)?;
    append(&footer, element(&document, "p", Some(&text.disclaimer))?)?;
    let privacy = element(&document, "p", None)?;
    append(
        &privacy,
        link(&document, Route::PrivacyEnglish, &text.privacy_link)?,
    )?;
    append(&footer, privacy)?;
    append(&body, footer)?;
    Ok(())
}

fn element(document: &Document, tag: &str, text: Option<&str>) -> Result<Element, JsValue> {
    let element = document.create_element(tag)?;
    element.set_text_content(text);
    Ok(element)
}

/// A link to a page of the site.
fn link(document: &Document, route: Route, text: &str) -> Result<Element, JsValue> {
    let link = element(document, "a", Some(text))?;
    let href = route
        .href()
        .ok_or_else(|| JsValue::from_str("the page has no link"))?;
    link.set_attribute("href", href)?;
    Ok(link)
}

/// A link to the policy in the other language, inside its own paragraph.
fn language_link(
    document: &Document,
    route: Route,
    text: &str,
    language: &str,
) -> Result<Element, JsValue> {
    let link = link(document, route, text)?;
    link.set_attribute("hreflang", language)?;
    let paragraph = element(document, "p", None)?;
    append(&paragraph, link)?;
    Ok(paragraph)
}

fn append(parent: &Element, child: Element) -> Result<(), JsValue> {
    parent.append_child(&child)?;
    Ok(())
}
