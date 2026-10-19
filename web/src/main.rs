//! Entry point of the web page. Trunk compiles this crate to WebAssembly.
//!
//! The page is built at runtime from translated texts. The crate also compiles natively
//! so that `cargo clippy` and `cargo test` cover it; only `main` needs a browser.

mod i18n;
mod page;

use i18n::Translations;
use page::PageText;
use wasm_bindgen::JsValue;
use web_sys::{Document, Node};

fn main() {
    if let Err(error) = render() {
        web_sys::console::error_1(&error);
    }
}

/// Fills the document title and body from the translations and `crafting-core`.
fn render() -> Result<(), JsValue> {
    let document = web_sys::window()
        .and_then(|window| window.document())
        .ok_or_else(|| JsValue::from_str("no document available"))?;
    let body = document
        .body()
        .ok_or_else(|| JsValue::from_str("document has no body"))?;
    let text = PageText::new(&Translations::english(), crafting_core::version());

    document.set_title(&text.title);

    let main = element(&document, "main", None)?;
    main.append_child(&element(&document, "h1", Some(&text.title))?)?;
    body.append_child(&main)?;

    let footer = element(&document, "footer", None)?;
    footer.append_child(&element(&document, "p", Some(&text.core_version))?)?;
    footer.append_child(&element(&document, "p", Some(&text.disclaimer))?)?;
    body.append_child(&footer)?;
    Ok(())
}

fn element(document: &Document, tag: &str, text: Option<&str>) -> Result<Node, JsValue> {
    let element = document.create_element(tag)?;
    element.set_text_content(text);
    Ok(element.into())
}
