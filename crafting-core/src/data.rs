//! The game data files, embedded at build time (ADR 0008).

use std::collections::BTreeSet;

use serde::Deserialize;

use crate::model::{BaseItem, ModDefinition};

const BASE_ITEMS_JSON: &str = include_str!("../../data/base_items.json");

#[derive(Deserialize)]
struct BaseItemsFile {
    base_items: Vec<BaseItem>,
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

fn validate_mod(base_id: &str, definition: &ModDefinition) {
    assert!(
        !definition.stats.is_empty(),
        "{base_id}: a mod needs at least one stat"
    );
    for stat in &definition.stats {
        assert!(
            stat.min <= stat.max,
            "{base_id}: stat {:?} has min above max",
            stat.id
        );
    }
    for (name, stat_id) in &definition.placeholders {
        assert!(
            definition.stats.iter().any(|stat| stat.id == *stat_id),
            "{base_id}: placeholder {name:?} names the unknown stat {stat_id:?}"
        );
        assert!(
            definition.text.contains(&format!("{{{name}}}")),
            "{base_id}: placeholder {name:?} does not occur in the mod text"
        );
    }
    let mut rest = definition.text.as_str();
    while let Some(start) = rest.find('{') {
        let length = rest[start..]
            .find('}')
            .unwrap_or_else(|| panic!("{base_id}: unclosed placeholder in the mod text"));
        let name = &rest[start + 1..start + length];
        assert!(
            definition.placeholders.contains_key(name),
            "{base_id}: the mod text uses the unmapped placeholder {name:?}"
        );
        rest = &rest[start + length..];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = r#"{"base_items": [{
        "id": "a", "name": "A", "item_class": "C", "required_level": 1, "tags": [],
        "implicits": [{
            "family": "F", "tags": [],
            "stats": [{"id": "s", "min": 1, "max": 2, "global": false}],
            "text": "+{v}", "placeholders": {"v": "s"}
        }]
    }]}"#;

    #[test]
    fn embedded_data_is_valid() {
        assert!(!base_items().is_empty());
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
