//! The field "Item level" in the browser: the number field and its listeners.
//!
//! What the field accepts is decided in `item_level.rs` and what a new level does in
//! `workbench.rs`; this module only reads the field, passes the level on and shows the level of
//! the workbench again. The field is outside the base picker, so the picker's key listener does
//! not see its Enter key.

use std::rc::Rc;

use wasm_bindgen::{JsCast, JsValue};
use web_sys::{Document, Element, Event, HtmlInputElement, KeyboardEvent};

use crate::i18n::Translations;
use crate::item_level;
use crate::picker_ui::listen;
use crate::{append, element, StartPage};

/// The identifier that connects the label with the field.
const FIELD_ID: &str = "item-level";

/// The elements of the field.
#[derive(Clone)]
pub struct LevelElements {
    pub root: Element,
    input: HtmlInputElement,
}

/// Builds the label and the number field with the default item level.
pub fn build(document: &Document, translations: &Translations) -> Result<LevelElements, JsValue> {
    let root = element(document, "div", None)?;
    root.set_class_name("level");
    let label = element(
        document,
        "label",
        Some(translations.text("item_level.field")),
    )?;
    label.set_attribute("for", FIELD_ID)?;
    let input = element(document, "input", None)?
        .dyn_into::<HtmlInputElement>()
        .map_err(JsValue::from)?;
    input.set_id(FIELD_ID);
    input.set_type("number");
    input.set_attribute("min", &item_level::ALLOWED.start().to_string())?;
    input.set_attribute("max", &item_level::ALLOWED.end().to_string())?;
    input.set_attribute("step", "1")?;
    input.set_value(&item_level::DEFAULT.to_string());
    append(&root, label)?;
    append(&root, input.clone().into())?;
    Ok(LevelElements { root, input })
}

/// Registers the listeners of the field. They live as long as the page. A change, leaving the
/// field and the Enter key all take over the text; taking over the same level again changes
/// nothing, so it does not matter that one input can fire several of them.
pub fn wire(page: &Rc<StartPage>) -> Result<(), JsValue> {
    let input = page.level_elements.input.clone();
    for event_name in ["change", "blur"] {
        listen(&input, event_name, false, page, |page, _| {
            page.commit_item_level()
        })?;
    }
    listen(
        &input,
        "keydown",
        false,
        page,
        |page, event: Event| match event.dyn_into::<KeyboardEvent>() {
            Ok(event) if event.key() == "Enter" => page.commit_item_level(),
            _ => Ok(()),
        },
    )
}

impl StartPage {
    /// Takes over the text of the field as the item level if it is valid and different from the
    /// current one, which resets the item. In every case the field shows the level of the
    /// workbench afterwards, so an invalid text is replaced by the last valid level.
    fn commit_item_level(&self) -> Result<(), JsValue> {
        let level = item_level::parse(&self.level_elements.input.value());
        let changed = level.is_some_and(|level| {
            let mut workbench = self.workbench.borrow_mut();
            let changed = level != workbench.item_level();
            workbench.set_item_level(level);
            changed
        });
        if changed {
            self.show(None)
        } else {
            self.show_item_level();
            Ok(())
        }
    }

    /// Shows the item level of the workbench in the field.
    pub(crate) fn show_item_level(&self) {
        let shown = self.workbench.borrow().item_level().to_string();
        let input = &self.level_elements.input;
        // Setting the value of the field to what it already holds would move the cursor.
        if input.value() != shown {
            input.set_value(&shown);
        }
    }
}
