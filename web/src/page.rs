//! Content of the page, built as plain data so it can be tested without a browser.

use crate::i18n::Translations;
use crafting_core::Currency;

/// All visible texts of the page.
#[derive(Debug, PartialEq, Eq)]
pub struct PageText {
    pub title: String,
    pub core_version: String,
    pub disclaimer: String,
    pub privacy_link: String,
    pub privacy_title: String,
    pub german_version_link: String,
    pub english_version_link: String,
    /// Labels of the currency buttons: the currencies' names, game texts from `crafting-core`.
    pub transmutation_button: String,
    pub alteration_button: String,
    pub augmentation_button: String,
    pub reset_button: String,
}

impl PageText {
    /// Looks up the texts; `core_version` is the version reported by `crafting-core`.
    pub fn new(translations: &Translations, core_version: &str) -> Self {
        Self {
            title: translations.text("app.title").to_owned(),
            core_version: translations.format("footer.core_version", &[("version", core_version)]),
            disclaimer: translations.text("footer.disclaimer").to_owned(),
            privacy_link: translations.text("footer.privacy").to_owned(),
            privacy_title: translations.text("privacy.title").to_owned(),
            german_version_link: translations.text("privacy.german_version").to_owned(),
            english_version_link: translations.text("privacy.english_version").to_owned(),
            transmutation_button: Currency::OrbOfTransmutation.name(),
            alteration_button: Currency::OrbOfAlteration.name(),
            augmentation_button: Currency::OrbOfAugmentation.name(),
            reset_button: translations.text("craft.reset").to_owned(),
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
        assert_eq!(text.privacy_link, t.text("footer.privacy"));
        assert_eq!(text.privacy_title, t.text("privacy.title"));
        assert_eq!(text.german_version_link, t.text("privacy.german_version"));
        assert_eq!(text.english_version_link, t.text("privacy.english_version"));
        assert_eq!(text.reset_button, t.text("craft.reset"));
    }

    #[test]
    fn currency_buttons_are_labelled_with_the_names_of_the_currencies() {
        let text = PageText::new(&Translations::english(), "0.1.0");
        assert_eq!(
            text.transmutation_button,
            Currency::OrbOfTransmutation.name()
        );
        assert_eq!(text.alteration_button, Currency::OrbOfAlteration.name());
        assert_eq!(text.augmentation_button, Currency::OrbOfAugmentation.name());
    }
}
