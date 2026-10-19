//! Text lookup for the user interface.
//!
//! Every visible text comes from a translation file (`web/i18n/<lang>.json`), which is
//! embedded at compile time. Keys are dotted, lower snake case; placeholders look like
//! `{name}`.

use std::collections::HashMap;

const EN_JSON: &str = include_str!("../i18n/en.json");

/// A set of translated texts for one language.
pub struct Translations {
    texts: HashMap<String, String>,
}

impl Translations {
    /// The English translations, which are the default language.
    pub fn english() -> Self {
        Self::from_json(EN_JSON).expect("web/i18n/en.json must be a flat JSON object of strings")
    }

    fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        Ok(Self {
            texts: serde_json::from_str(json)?,
        })
    }

    /// Text for `key`.
    ///
    /// A missing key is a programming error, not a runtime condition: tests check that
    /// every key used by the UI exists, so this panics instead of showing a wrong text.
    pub fn text(&self, key: &str) -> &str {
        self.texts
            .get(key)
            .unwrap_or_else(|| panic!("missing translation key {key:?}"))
    }

    /// Text for `key` with every `{name}` replaced by the matching value in `params`.
    pub fn format(&self, key: &str, params: &[(&str, &str)]) -> String {
        params
            .iter()
            .fold(self.text(key).to_owned(), |text, (name, value)| {
                text.replace(&format!("{{{name}}}"), value)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_replaces_placeholders() {
        let t = Translations::from_json(r#"{"k": "a {x} b {y} {x}"}"#).unwrap();
        assert_eq!(t.format("k", &[("x", "1"), ("y", "2")]), "a 1 b 2 1");
    }

    #[test]
    fn format_without_params_returns_text_unchanged() {
        let t = Translations::from_json(r#"{"k": "plain"}"#).unwrap();
        assert_eq!(t.format("k", &[]), "plain");
    }

    #[test]
    #[should_panic(expected = "missing translation key")]
    fn missing_key_is_a_programming_error() {
        Translations::english().text("does.not_exist");
    }

    #[test]
    fn invalid_json_is_rejected() {
        assert!(Translations::from_json("[1]").is_err());
    }
}
