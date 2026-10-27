//! The base picker in the browser: the field, the panel below it and the listeners.
//!
//! What the picker shows and what its buttons do is decided in `picker.rs`; this module only
//! builds the elements from that, routes the events to it and moves the focus. The panel
//! listens for clicks once and finds the button by its `data-action` attribute, so no listener
//! is created per render.

use std::rc::Rc;

use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{
    Document, Element, Event, EventTarget, HtmlElement, HtmlInputElement, KeyboardEvent, Node,
};

use crate::i18n::Translations;
use crate::picker::{Action, Body, OptionView, PathPart, PickerView, SlotTile};
use crate::{append, button, element, StartPage};

/// The identifier that connects the label with the field.
const FIELD_ID: &str = "base-picker";

/// The attribute of a button that holds its action (`Action::token`).
const ACTION_ATTRIBUTE: &str = "data-action";

/// The elements of the picker.
#[derive(Clone)]
pub struct PickerElements {
    pub root: Element,
    input: HtmlInputElement,
    panel: Element,
}

/// Builds the label, the field and the empty panel. The panel is hidden until the picker is
/// opened.
pub fn build(document: &Document, translations: &Translations) -> Result<PickerElements, JsValue> {
    let root = element(document, "div", None)?;
    root.set_class_name("picker");
    let label = element(document, "label", Some(translations.text("picker.field")))?;
    label.set_attribute("for", FIELD_ID)?;
    let input = element(document, "input", None)?
        .dyn_into::<HtmlInputElement>()
        .map_err(JsValue::from)?;
    input.set_id(FIELD_ID);
    input.set_type("text");
    input.set_attribute("autocomplete", "off")?;
    let panel = element(document, "div", None)?;
    panel.set_class_name("picker-panel");
    panel.set_attribute("hidden", "")?;
    append(&root, label)?;
    append(&root, input.clone().into())?;
    append(&root, panel.clone())?;
    Ok(PickerElements { root, input, panel })
}

/// Registers the listeners of the picker. They live as long as the page.
pub fn wire(page: &Rc<StartPage>) -> Result<(), JsValue> {
    let elements = page.picker_elements.clone();
    listen(&elements.input, "click", false, page, |page, _| {
        page.picker.borrow_mut().open();
        page.render_picker()
    })?;
    listen(&elements.input, "input", false, page, |page, _| {
        {
            let mut picker = page.picker.borrow_mut();
            picker.open();
            picker.set_query(&page.picker_elements.input.value());
        }
        page.render_picker()
    })?;
    listen(
        &elements.root,
        "keydown",
        false,
        page,
        |page, event| match event.dyn_into::<KeyboardEvent>() {
            Ok(event) => page.on_picker_key(&event),
            Err(_) => Ok(()),
        },
    )?;
    listen(&elements.panel, "click", false, page, |page, event| {
        page.on_panel_click(&event)
    })?;
    // A click outside closes the picker. The listener runs in the capture phase, before the
    // click is handled, so that a button that the click removes from the panel still counts as
    // inside.
    let document = page.document.clone();
    listen(&document, "click", true, page, |page, event| {
        page.on_document_click(&event)
    })
}

/// Runs `handler` on the page whenever `event_name` reaches `target`.
fn listen(
    target: &EventTarget,
    event_name: &str,
    capture: bool,
    page: &Rc<StartPage>,
    handler: impl Fn(&StartPage, Event) -> Result<(), JsValue> + 'static,
) -> Result<(), JsValue> {
    let page = Rc::clone(page);
    let listener = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
        if let Err(error) = handler(&page, event) {
            web_sys::console::error_1(&error);
        }
    });
    target.add_event_listener_with_callback_and_bool(
        event_name,
        listener.as_ref().unchecked_ref(),
        capture,
    )?;
    // The listener must stay alive as long as the page, so the closure is not dropped.
    listener.forget();
    Ok(())
}

impl StartPage {
    /// Shows the field and the panel for the state of the picker.
    pub(crate) fn render_picker(&self) -> Result<(), JsValue> {
        let elements = &self.picker_elements;
        let picker = self.picker.borrow();
        // Setting the value of the field to what it already holds would move the cursor.
        if elements.input.value() != picker.query() {
            elements.input.set_value(picker.query());
        }
        if picker.is_open() {
            let placeholder = self.translations.text("picker.placeholder");
            elements.input.set_placeholder(placeholder);
            let view = picker.view(&self.catalog, &self.translations);
            elements.panel.set_text_content(None);
            append(&elements.panel, panel_content(&self.document, &view)?)?;
            elements.panel.remove_attribute("hidden")
        } else {
            elements.input.set_placeholder("");
            elements.panel.set_attribute("hidden", "")?;
            elements.panel.set_text_content(None);
            Ok(())
        }
    }

    /// Escape closes the picker and puts the focus back into the field. Enter and the arrow
    /// down key in the field open the picker, or move to its first button when it is open.
    fn on_picker_key(&self, event: &KeyboardEvent) -> Result<(), JsValue> {
        match event.key().as_str() {
            "Escape" if self.picker.borrow().is_open() => {
                self.close_picker()?;
                self.picker_elements.input.focus()
            }
            "Enter" | "ArrowDown" if self.event_is_in_field(event) => {
                event.prevent_default();
                let opened = !self.picker.borrow().is_open();
                if opened {
                    self.picker.borrow_mut().open();
                    self.render_picker()?;
                }
                self.focus_first_choice();
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn event_is_in_field(&self, event: &Event) -> bool {
        let field: &EventTarget = self.picker_elements.input.as_ref();
        event.target().as_ref() == Some(field)
    }

    /// Does what the clicked button of the panel stands for.
    fn on_panel_click(&self, event: &Event) -> Result<(), JsValue> {
        let Some(target) = event.target().and_then(|t| t.dyn_into::<Element>().ok()) else {
            return Ok(());
        };
        let selector = format!("button[{ACTION_ATTRIBUTE}]");
        let Some(clicked) = target.closest(&selector)? else {
            return Ok(());
        };
        let action = clicked
            .get_attribute(ACTION_ATTRIBUTE)
            .and_then(|token| Action::from_token(&token));
        let Some(action) = action else {
            return Ok(());
        };
        let chosen = self.picker.borrow_mut().apply(&self.catalog, &action);
        self.render_picker()?;
        match chosen {
            Some(base_id) => {
                self.workbench.borrow_mut().select(&base_id);
                self.show(None)
            }
            None => {
                self.focus_first_choice();
                Ok(())
            }
        }
    }

    /// Closes the picker when the click is outside of it.
    fn on_document_click(&self, event: &Event) -> Result<(), JsValue> {
        if !self.picker.borrow().is_open() {
            return Ok(());
        }
        let inside = event
            .target()
            .and_then(|target| target.dyn_into::<Node>().ok())
            .is_some_and(|node| self.picker_elements.root.contains(Some(&node)));
        if inside {
            Ok(())
        } else {
            self.close_picker()
        }
    }

    fn close_picker(&self) -> Result<(), JsValue> {
        self.picker.borrow_mut().close();
        self.render_picker()
    }

    /// Puts the focus on the first button of the main part of the panel, if there is one.
    fn focus_first_choice(&self) {
        let first = self
            .picker_elements
            .panel
            .query_selector(".picker-body button")
            .ok()
            .flatten()
            .and_then(|found| found.dyn_into::<HtmlElement>().ok());
        if let Some(first) = first {
            // A failure to focus changes nothing that the player needs.
            let _ = first.focus();
        }
    }
}

/// The path, the main part and the hint.
fn panel_content(document: &Document, view: &PickerView) -> Result<Element, JsValue> {
    let content = element(document, "div", None)?;
    append(&content, path(document, view)?)?;
    let body = element(document, "div", None)?;
    body.set_class_name("picker-body");
    match &view.body {
        Body::Slots(tiles) => append(&body, slots(document, tiles)?)?,
        Body::Options(options) => append(&body, option_list(document, options)?)?,
        Body::NothingFound(text) => {
            let paragraph = element(document, "p", Some(text))?;
            paragraph.set_class_name("nothing-found");
            append(&body, paragraph)?;
        }
    }
    append(&content, body)?;
    let hint = element(document, "p", Some(&view.hint))?;
    hint.set_class_name("hint");
    append(&content, hint)?;
    Ok(content)
}

/// The path: every part except the last one is a button.
fn path(document: &Document, view: &PickerView) -> Result<Element, JsValue> {
    let path = element(document, "nav", None)?;
    path.set_class_name("path");
    for (index, part) in view.path.iter().enumerate() {
        if index > 0 {
            let separator = element(document, "span", Some(&view.separator))?;
            separator.set_attribute("aria-hidden", "true")?;
            append(&path, separator)?;
        }
        append(&path, path_part(document, part)?)?;
    }
    Ok(path)
}

fn path_part(document: &Document, part: &PathPart) -> Result<Element, JsValue> {
    match &part.action {
        Some(action) => action_button(document, &part.label, action),
        None => {
            let current = element(document, "span", Some(&part.label))?;
            current.set_attribute("aria-current", "location")?;
            Ok(current)
        }
    }
}

/// The slots of the grid. A slot that cannot be chosen is no button.
fn slots(document: &Document, tiles: &[SlotTile]) -> Result<Element, JsValue> {
    let grid = element(document, "div", None)?;
    grid.set_class_name("slots");
    for tile in tiles {
        let slot = match &tile.action {
            Some(action) => action_button(document, &tile.label, action)?,
            None => {
                let slot = element(document, "div", None)?;
                append(&slot, element(document, "span", Some(&tile.label))?)?;
                if let Some(note) = &tile.note {
                    slot.append_with_str_1(" ")?;
                    let note = element(document, "span", Some(note))?;
                    note.set_class_name("note");
                    append(&slot, note)?;
                }
                slot
            }
        };
        slot.set_class_name(if tile.action.is_some() {
            "slot"
        } else {
            "slot unavailable"
        });
        append(&grid, slot)?;
    }
    Ok(grid)
}

/// A list with one button per class or base: its mark, its name and the lines of its implicit.
fn option_list(document: &Document, options: &[OptionView]) -> Result<Element, JsValue> {
    let list = element(document, "ul", None)?;
    list.set_class_name("options");
    for option in options {
        let choice = action_button(document, "", &option.action)?;
        choice.set_class_name("option");
        if let Some(kind) = &option.kind {
            let kind = element(document, "span", Some(kind))?;
            kind.set_class_name("kind");
            append(&choice, kind)?;
            // The space keeps the mark and the name apart for a screen reader.
            choice.append_with_str_1(" ")?;
        }
        let name = element(document, "span", Some(&option.label))?;
        name.set_class_name("option-name");
        append(&choice, name)?;
        for line in &option.implicit {
            let line = element(document, "span", Some(line))?;
            line.set_class_name("implicit");
            append(&choice, line)?;
        }
        let item = element(document, "li", None)?;
        append(&item, choice)?;
        append(&list, item)?;
    }
    Ok(list)
}

/// A button with the given label that stands for an action.
fn action_button(document: &Document, label: &str, action: &Action) -> Result<Element, JsValue> {
    let choice = button(document, label)?;
    choice.set_type("button");
    choice.set_attribute(ACTION_ATTRIBUTE, &action.token())?;
    Ok(choice.into())
}
