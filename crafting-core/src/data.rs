//! The game data files, embedded at build time (ADR 0008).

use std::collections::BTreeSet;

use serde::Deserialize;

use crate::craft::CraftError;
use crate::currency::CurrencyDefinition;
use crate::model::{BaseItem, ExplicitModDefinition, ModDefinition};

const BASE_ITEMS_JSON: &str = include_str!("../../data/base_items.json");
const MODS_JSON: &str = include_str!("../../data/mods.json");
const CURRENCIES_JSON: &str = include_str!("../../data/currencies.json");

#[derive(Deserialize)]
struct BaseItemsFile {
    base_items: Vec<BaseItem>,
}

#[derive(Deserialize)]
struct ModsFile {
    mods: Vec<ExplicitModDefinition>,
}

#[derive(Deserialize)]
struct CurrenciesFile {
    currencies: Vec<CurrencyDefinition>,
}

/// All base items of the game data.
///
/// # Panics
/// If the embedded data is invalid. That is a programming error, caught by the unit tests.
pub fn base_items() -> Vec<BaseItem> {
    parse_base_items(BASE_ITEMS_JSON)
}

/// The base item with the given identifier, if there is one.
pub fn base_item(id: &str) -> Option<BaseItem> {
    base_items().into_iter().find(|base| base.id == id)
}

/// All prefixes and suffixes of the game data.
///
/// # Panics
/// If the embedded data is invalid. That is a programming error, caught by the unit tests.
pub fn explicit_mods() -> Vec<ExplicitModDefinition> {
    parse_mods(MODS_JSON)
}

/// All currencies of the game data.
///
/// # Panics
/// If the embedded data is invalid. That is a programming error, caught by the unit tests.
pub fn currencies() -> Vec<CurrencyDefinition> {
    parse_currencies(CURRENCIES_JSON)
}

fn parse_mods(json: &str) -> Vec<ExplicitModDefinition> {
    let file: ModsFile =
        serde_json::from_str(json).unwrap_or_else(|error| panic!("invalid mod data: {error}"));
    for explicit in &file.mods {
        assert!(
            explicit.weight > 0,
            "{}: the weight must be above zero",
            explicit.name
        );
        validate_mod(&explicit.name, &explicit.definition);
    }
    file.mods
}

fn parse_currencies(json: &str) -> Vec<CurrencyDefinition> {
    let file: CurrenciesFile =
        serde_json::from_str(json).unwrap_or_else(|error| panic!("invalid currency data: {error}"));
    let mut ids = BTreeSet::new();
    for currency in &file.currencies {
        assert!(
            ids.insert(&currency.id),
            "duplicate currency {:?}",
            currency.id
        );
        for (code, message) in &currency.messages {
            assert!(
                CraftError::ALL.iter().any(|error| error.code() == code),
                "currency {:?}: {code:?} is not the code of a craft error",
                currency.id
            );
            assert!(
                !message.is_empty(),
                "currency {:?}: the message for {code:?} is empty",
                currency.id
            );
        }
    }
    file.currencies
}

fn parse_base_items(json: &str) -> Vec<BaseItem> {
    let file: BaseItemsFile = serde_json::from_str(json)
        .unwrap_or_else(|error| panic!("invalid base item data: {error}"));
    let mut ids = BTreeSet::new();
    for base in &file.base_items {
        assert!(ids.insert(&base.id), "duplicate base item {:?}", base.id);
        for implicit in &base.implicits {
            validate_mod(&base.id, implicit);
        }
    }
    file.base_items
}

/// Checks a mod; `owner` names the base item or mod it belongs to, for the messages.
fn validate_mod(owner: &str, definition: &ModDefinition) {
    assert!(
        !definition.stats.is_empty(),
        "{owner}: a mod needs at least one stat"
    );
    for stat in &definition.stats {
        assert!(
            stat.min <= stat.max,
            "{owner}: stat {:?} has min above max",
            stat.id
        );
    }
    for (name, stat_id) in &definition.placeholders {
        assert!(
            definition.stats.iter().any(|stat| stat.id == *stat_id),
            "{owner}: placeholder {name:?} names the unknown stat {stat_id:?}"
        );
        assert!(
            definition.text.contains(&format!("{{{name}}}")),
            "{owner}: placeholder {name:?} does not occur in the mod text"
        );
    }
    let mut rest = definition.text.as_str();
    while let Some(start) = rest.find('{') {
        let length = rest[start..]
            .find('}')
            .unwrap_or_else(|| panic!("{owner}: unclosed placeholder in the mod text"));
        let name = &rest[start + 1..start + length];
        assert!(
            definition.placeholders.contains_key(name),
            "{owner}: the mod text uses the unmapped placeholder {name:?}"
        );
        rest = &rest[start + length..];
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::AffixType;

    const VALID: &str = r#"{"base_items": [{
        "id": "a", "name": "A", "item_class": "C", "required_level": 1, "tags": [],
        "implicits": [{
            "family": "F", "tags": [],
            "stats": [{"id": "s", "min": 1, "max": 2, "global": false}],
            "text": "+{v}", "placeholders": {"v": "s"}
        }]
    }]}"#;

    const VALID_MODS: &str = r#"{"mods": [{
        "name": "Invented", "affix_type": "suffix", "level": 1, "weight": 10,
        "family": "F", "tags": [],
        "stats": [{"id": "s", "min": 1, "max": 2, "global": false}],
        "text": "+{v}", "placeholders": {"v": "s"}
    }]}"#;

    const VALID_CURRENCIES: &str = r#"{"currencies": [{
        "id": "a", "name": "A", "description": "D",
        "messages": {"item_not_normal": "M"}
    }]}"#;

    #[test]
    fn embedded_data_is_valid() {
        assert!(!base_items().is_empty());
        assert!(!explicit_mods().is_empty());
        assert!(!currencies().is_empty());
    }

    #[test]
    fn valid_mods_are_accepted() {
        let mods = parse_mods(VALID_MODS);
        assert_eq!(mods.len(), 1);
        assert_eq!(mods[0].affix_type, AffixType::Suffix);
        assert_eq!(mods[0].weight, 10);
        assert_eq!(mods[0].definition.family, "F");
    }

    #[test]
    #[should_panic(expected = "weight must be above zero")]
    fn mod_with_weight_zero_is_rejected() {
        parse_mods(&VALID_MODS.replace(r#""weight": 10"#, r#""weight": 0"#));
    }

    #[test]
    #[should_panic(expected = "invalid mod data")]
    fn mod_with_an_unknown_affix_type_is_rejected() {
        parse_mods(&VALID_MODS.replace("suffix", "implicit"));
    }

    #[test]
    #[should_panic(expected = "unknown stat")]
    fn mod_placeholder_of_an_unknown_stat_is_rejected() {
        parse_mods(&VALID_MODS.replace(r#""v": "s""#, r#""v": "other""#));
    }

    #[test]
    fn every_embedded_mod_has_a_positive_weight_and_a_unique_name() {
        let mods = explicit_mods();
        let names: BTreeSet<&str> = mods.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names.len(), mods.len());
        assert!(mods.iter().all(|m| m.weight > 0));
    }

    #[test]
    fn valid_currencies_are_accepted() {
        assert_eq!(parse_currencies(VALID_CURRENCIES).len(), 1);
    }

    #[test]
    #[should_panic(expected = "duplicate currency")]
    fn duplicate_currency_ids_are_rejected() {
        let entry = r#"{"id": "a", "name": "A", "description": "D", "messages": {}}"#;
        parse_currencies(&format!(r#"{{"currencies": [{entry}, {entry}]}}"#));
    }

    #[test]
    #[should_panic(expected = "is not the code of a craft error")]
    fn a_message_for_an_unknown_reason_is_rejected() {
        parse_currencies(&VALID_CURRENCIES.replace("item_not_normal", "invented_reason"));
    }

    #[test]
    #[should_panic(expected = "is empty")]
    fn an_empty_message_is_rejected() {
        parse_currencies(&VALID_CURRENCIES.replace(r#""M""#, r#""""#));
    }

    #[test]
    #[should_panic(expected = "invalid currency data")]
    fn a_currency_without_a_description_is_rejected() {
        parse_currencies(&VALID_CURRENCIES.replace(r#""description": "D","#, ""));
    }

    #[test]
    fn every_embedded_base_item_can_be_found_by_id() {
        for base in base_items() {
            assert_eq!(base_item(&base.id), Some(base));
        }
    }

    #[test]
    fn unknown_id_gives_none() {
        assert_eq!(base_item("does/not/exist"), None);
    }

    #[test]
    fn valid_data_is_accepted() {
        assert_eq!(parse_base_items(VALID).len(), 1);
    }

    #[test]
    #[should_panic(expected = "unknown stat")]
    fn placeholder_of_an_unknown_stat_is_rejected() {
        parse_base_items(&VALID.replace(r#""v": "s""#, r#""v": "other""#));
    }

    #[test]
    #[should_panic(expected = "unmapped placeholder")]
    fn text_placeholder_without_mapping_is_rejected() {
        parse_base_items(&VALID.replace("+{v}", "+{v} {w}"));
    }

    #[test]
    #[should_panic(expected = "min above max")]
    fn inverted_range_is_rejected() {
        parse_base_items(&VALID.replace(r#""min": 1, "max": 2"#, r#""min": 3, "max": 2"#));
    }

    #[test]
    #[should_panic(expected = "duplicate base item")]
    fn duplicate_ids_are_rejected() {
        let entry = &VALID[VALID.find('[').unwrap() + 1..VALID.rfind(']').unwrap()];
        parse_base_items(&format!(r#"{{"base_items": [{entry}, {entry}]}}"#));
    }

    #[test]
    #[should_panic(expected = "at least one stat")]
    fn mod_without_stats_is_rejected() {
        parse_base_items(&VALID.replace(
            r#"[{"id": "s", "min": 1, "max": 2, "global": false}]"#,
            "[]",
        ));
    }
}
