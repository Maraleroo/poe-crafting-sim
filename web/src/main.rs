//! Entry point of the web page. Trunk compiles this crate to WebAssembly.
//!
//! The page is built at runtime from translated texts. The crate also compiles natively
//! so that `cargo clippy` and `cargo test` cover it; only `main` needs a browser.

mod blocks_ui;
mod i18n;
mod item_level;
mod item_view;
mod legal;
mod level_ui;
mod page;
mod picker;
mod picker_ui;
mod route;
mod workbench;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use blocks_ui::ViewToggle;
use crafting_core::Currency;
use i18n::Translations;
use item_view::ItemView;
use level_ui::LevelElements;
use page::PageText;
use picker::{Catalog, Picker};
use picker_ui::PickerElements;
use route::Route;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{Document, Element, HtmlButtonElement, Window};
use workbench::Workbench;

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
            StartPage::mount(&document, &main, &text, random_seed(&window)?)?;
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

/// The seed of the page's generator: 8 random bytes from the browser's Web Crypto API
/// (ADR 0010). Asking for them sends no request and uses no browser storage.
fn random_seed(window: &Window) -> Result<u64, JsValue> {
    let mut bytes = [0u8; 8];
    window
        .crypto()?
        .get_random_values_with_u8_array(&mut bytes)?;
    Ok(u64::from_le_bytes(bytes))
}

/// The start page: the base picker, the item level field, the buttons that change the item, the message and the item
/// on the workbench.
struct StartPage {
    document: Document,
    translations: Translations,
    workbench: RefCell<Workbench>,
    item_host: Element,
    /// The button of each currency, in the order they are shown.
    currencies: Vec<(Currency, HtmlButtonElement)>,
    message: Element,
    /// The state of the base picker, what it offers and its elements (`picker_ui.rs`).
    picker: RefCell<Picker>,
    catalog: Catalog,
    picker_elements: PickerElements,
    level_elements: LevelElements,
    /// The toggle between the default and the advanced view of the mods. The choice is kept in
    /// memory only: a new page starts with the default view.
    view_toggle: ViewToggle,
    advanced: Cell<bool>,
}

impl StartPage {
    /// Adds the base picker, the buttons, the message area and the item to `parent` and
    /// registers the listeners. The listeners live as long as the page.
    fn mount(
        document: &Document,
        parent: &Element,
        text: &PageText,
        seed: u64,
    ) -> Result<(), JsValue> {
        let item_host = element(document, "div", None)?;
        let picker_elements = picker_ui::build(document, &Translations::english())?;
        let level_elements = level_ui::build(document, &Translations::english())?;
        let fields = element(document, "div", None)?;
        fields.set_class_name("fields");
        append(&fields, picker_elements.root.clone())?;
        append(&fields, level_elements.root.clone())?;
        append(parent, fields)?;
        let currencies = vec![
            (
                Currency::OrbOfTransmutation,
                button(document, &text.transmutation_button)?,
            ),
            (
                Currency::OrbOfAlteration,
                button(document, &text.alteration_button)?,
            ),
            (
                Currency::OrbOfAugmentation,
                button(document, &text.augmentation_button)?,
            ),
        ];
        let reset = button(document, &text.reset_button)?;
        let buttons = element(document, "div", None)?;
        buttons.set_class_name("buttons");
        for (_, orb) in &currencies {
            append(&buttons, orb.clone().into())?;
        }
        append(&buttons, reset.clone().into())?;
        let message = element(document, "p", None)?;
        message.set_class_name("message");
        append(parent, buttons)?;
        append(parent, message.clone())?;
        let view_toggle = blocks_ui::build_toggle(document, &Translations::english())?;
        append(parent, view_toggle.root.clone())?;
        append(parent, item_host.clone())?;

        let page = Rc::new(StartPage {
            document: document.clone(),
            translations: Translations::english(),
            workbench: RefCell::new(Workbench::new(seed)),
            item_host,
            currencies: currencies.clone(),
            message,
            picker: RefCell::new(Picker::new()),
            catalog: Catalog::new(),
            picker_elements,
            level_elements,
            view_toggle: view_toggle.clone(),
            advanced: Cell::new(false),
        });
        picker_ui::wire(&page)?;
        level_ui::wire(&page)?;
        page.render_picker()?;
        page.show(None)?;
        for (currency, orb) in &currencies {
            let currency = *currency;
            listen(orb, &page, move |page| page.apply(currency))?;
        }
        listen(&view_toggle.default_button, &page, |page| {
            page.show_view(false)
        })?;
        listen(&view_toggle.advanced_button, &page, |page| {
            page.show_view(true)
        })?;
        listen(&reset, &page, StartPage::reset)
    }

    /// Switches between the default and the advanced view of the mods.
    fn show_view(&self, advanced: bool) -> Result<(), JsValue> {
        self.advanced.set(advanced);
        self.show_item()
    }

    /// Renders the item in the chosen view and marks the pressed button of the toggle.
    fn show_item(&self) -> Result<(), JsValue> {
        let workbench = self.workbench.borrow();
        let view = ItemView::new(&self.translations, workbench.item());
        self.item_host.set_text_content(None);
        append(
            &self.item_host,
            item_section(&self.document, &view, self.advanced.get())?,
        )?;
        self.view_toggle.show(self.advanced.get())
    }

    /// Renders the item again and updates the currency buttons and the message.
    fn show(&self, message: Option<&str>) -> Result<(), JsValue> {
        self.show_item()?;
        let workbench = self.workbench.borrow();
        // A currency stays clickable when it does not apply: the click shows the reason. The
        // HTML attribute `disabled` would swallow the click, so the state is `aria-disabled`.
        for (currency, orb) in &self.currencies {
            let enabled = workbench.applicable(*currency);
            orb.set_attribute("aria-disabled", if enabled { "false" } else { "true" })?;
            orb.set_class_name(if enabled { "" } else { "inapplicable" });
            orb.set_title(&workbench.tooltip(*currency));
        }
        self.message.set_text_content(message);
        self.show_item_level();
        Ok(())
    }

    fn apply(&self, currency: Currency) -> Result<(), JsValue> {
        let result = self.workbench.borrow_mut().apply(currency);
        let message = result
            .err()
            .map(|error| Workbench::failure_message(currency, error));
        self.show(message.as_deref())
    }

    fn reset(&self) -> Result<(), JsValue> {
        self.workbench.borrow_mut().reset();
        self.show(None)
    }
}

/// A button with the given label.
fn button(document: &Document, label: &str) -> Result<HtmlButtonElement, JsValue> {
    element(document, "button", Some(label))?
        .dyn_into::<HtmlButtonElement>()
        .map_err(JsValue::from)
}

/// Runs `action` on the page whenever `button` is clicked.
fn listen(
    button: &HtmlButtonElement,
    page: &Rc<StartPage>,
    action: impl Fn(&StartPage) -> Result<(), JsValue> + 'static,
) -> Result<(), JsValue> {
    let page = Rc::clone(page);
    let on_click = Closure::<dyn FnMut()>::new(move || {
        if let Err(error) = action(&page) {
            web_sys::console::error_1(&error);
        }
    });
    button.add_event_listener_with_callback("click", on_click.as_ref().unchecked_ref())?;
    // The listener must stay alive as long as the page, so the closure is not dropped.
    on_click.forget();
    Ok(())
}

/// The item as a section: name, class, rarity, level requirement and its mods, in the default
/// view (implicit mods and the merged lines of the prefixes and suffixes) or in the advanced
/// view (one block per mod).
fn item_section(document: &Document, item: &ItemView, advanced: bool) -> Result<Element, JsValue> {
    let section = element(document, "section", None)?;
    section.set_class_name(&format!("item {}", item.rarity_class));
    append(&section, element(document, "h2", Some(&item.name))?)?;
    append(&section, element(document, "p", Some(&item.item_class))?)?;
    append(&section, element(document, "p", Some(&item.rarity))?)?;
    if let Some(requires_level) = &item.requires_level {
        append(&section, element(document, "p", Some(requires_level))?)?;
    }
    if advanced {
        append(&section, blocks_ui::blocks_list(document, &item.blocks)?)?;
    } else {
        append(&section, mod_list(document, &item.implicits)?)?;
        if !item.explicits.is_empty() {
            append(&section, mod_list(document, &item.explicits)?)?;
        }
    }
    Ok(section)
}

/// A list with one entry per line of mod text.
fn mod_list(document: &Document, lines: &[String]) -> Result<Element, JsValue> {
    let list = element(document, "ul", None)?;
    for line in lines {
        append(&list, element(document, "li", Some(line))?)?;
    }
    Ok(list)
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
