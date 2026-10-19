//! Content of the page, built as plain data so it can be tested without a browser.

use crate::i18n::Translations;

/// All visible texts of the page.
#[derive(Debug, PartialEq, Eq)]
pub struct PageText {
    pub title: String,
    pub core_version: String,
    pub disclaimer: String,
}

impl PageText {
    /// Looks up the texts; `core_version` is the version reported by `crafting-core`.
    pub fn new(translations: &Translations, core_version: &str) -> Self {
        Self {
            title: translations.text("app.title").to_owned(),
            core_version: translations.format("footer.core_version", &[("version", core_version)]),
            disclaimer: translations.text("footer.disclaimer").to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn footer_shows_the_given_core_version() {
        let text = PageText::new(&Translations::english(), "1.2.3");
        assert!(text.core_version.contains("1.2.3"));
        assert!(!text.core_version.contains("{version}"));
    }

    #[test]
    fn page_text_comes_from_translations_and_all_keys_exist() {
        let t = Translations::english();
        let text = PageText::new(&t, "0.1.0");
        assert_eq!(text.title, t.text("app.title"));
        assert_eq!(text.disclaimer, t.text("footer.disclaimer"));
    }
}
