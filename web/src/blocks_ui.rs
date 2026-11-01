//! The advanced view in the browser: the toggle between the two views and the blocks of the
//! mods (ADR 0024).
//!
//! What a block shows is decided in `crafting-core` and put into texts in `item_view.rs`; this
//! module only builds the elements. The range of a value is an element of its own, so that the
//! stylesheet can set it apart; the text gets no added spaces, so a copy of a line keeps the
//! format of the extended item text.

use wasm_bindgen::{JsCast, JsValue};
use web_sys::{Document, Element, HtmlButtonElement};

use crate::i18n::Translations;
use crate::item_view::BlockView;
use crate::{append, element};
use crafting_core::TextPart;

/// The two buttons of the toggle. The page holds the choice in memory only.
#[derive(Clone)]
pub struct ViewToggle {
    pub root: Element,
    pub default_button: HtmlButtonElement,
    pub advanced_button: HtmlButtonElement,
}

/// Builds the toggle: two buttons next to each other that share one row. Their texts wrap
/// inside the buttons on a narrow page.
pub fn build_toggle(
    document: &Document,
    translations: &Translations,
) -> Result<ViewToggle, JsValue> {
    let root = element(document, "div", None)?;
    root.set_class_name("view-toggle");
    let make = |key: &str| -> Result<HtmlButtonElement, JsValue> {
        let button = element(document, "button", Some(translations.text(key)))?
            .dyn_into::<HtmlButtonElement>()
            .map_err(JsValue::from)?;
        button.set_type("button");
        append(&root, button.clone().into())?;
        Ok(button)
    };
    let default_button = make("view.default")?;
    let advanced_button = make("view.advanced")?;
    Ok(ViewToggle {
        root,
        default_button,
        advanced_button,
    })
}

impl ViewToggle {
    /// Marks the button of the shown view as pressed.
    pub fn show(&self, advanced: bool) -> Result<(), JsValue> {
        for (button, pressed) in [
            (&self.default_button, !advanced),
            (&self.advanced_button, advanced),
        ] {
            button.set_attribute("aria-pressed", if pressed { "true" } else { "false" })?;
        }
        Ok(())
    }
}

/// The list of blocks, one entry per mod.
pub fn blocks_list(document: &Document, blocks: &[BlockView]) -> Result<Element, JsValue> {
    let list = element(document, "ul", None)?;
    list.set_class_name("blocks");
    for block in blocks {
        append(&list, block_item(document, block)?)?;
    }
    Ok(list)
}

/// One block: the short label on the left, the lines, the tier box on the right of the first
/// line, and below the lines the name, the separator, the tags and the mark of an increase.
fn block_item(document: &Document, block: &BlockView) -> Result<Element, JsValue> {
    let item = element(document, "li", None)?;
    item.set_class_name(&format!("block {}", block.kind_class));

    // The name of the kind is the accessible name and the tooltip of the short label.
    let label = element(document, "span", Some(&block.short_label))?;
    label.set_class_name("kind");
    label.set_attribute("role", "img")?;
    label.set_attribute("aria-label", &block.kind_name)?;
    label.set_attribute("title", &block.kind_name)?;
    append(&item, label)?;

    let lines = element(document, "div", None)?;
    lines.set_class_name("lines");
    for line in &block.lines {
        append(&lines, line_element(document, line)?)?;
    }
    append(&item, lines)?;

    if let Some(tier) = &block.tier {
        append(&item, classed(document, "span", tier, "tier")?)?;
    }

    let meta = element(document, "p", None)?;
    meta.set_class_name("meta");
    if let Some(name) = &block.name {
        append(&meta, classed(document, "span", name, "name")?)?;
    }
    let tail = match (&block.separator, block.tags.is_empty()) {
        (Some(separator), false) => Some(format!(" {separator} {}", block.tags)),
        (None, false) => Some(block.tags.clone()),
        _ => None,
    };
    if let Some(tail) = tail {
        meta.insert_adjacent_text("beforeend", &tail)?;
    }
    if let Some(increase) = &block.increase {
        append(&meta, classed(document, "span", increase, "increase")?)?;
    }
    if meta.has_child_nodes() {
        append(&item, meta)?;
    }
    Ok(item)
}

/// One line of a mod: its text, with each range in an element of its own.
fn line_element(document: &Document, line: &[TextPart]) -> Result<Element, JsValue> {
    let element_of_line = element(document, "span", None)?;
    element_of_line.set_class_name("line");
    for part in line {
        match part {
            TextPart::Text(text) => {
                element_of_line.insert_adjacent_text("beforeend", text)?;
            }
            TextPart::Range(range) => {
                append(&element_of_line, classed(document, "span", range, "range")?)?;
            }
        }
    }
    Ok(element_of_line)
}

/// An element with a text and a CSS class.
fn classed(document: &Document, tag: &str, text: &str, class: &str) -> Result<Element, JsValue> {
    let created = element(document, tag, Some(text))?;
    created.set_class_name(class);
    Ok(created)
}
