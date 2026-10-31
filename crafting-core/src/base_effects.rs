//! What the implicit lines of a base do to the prefixes and suffixes of its items (ADR 0022,
//! rules "Affix limits" and the paragraph on modifier magnitudes in `poe-crafting-rules`): the
//! change of the affix limits and the increase of the modifier magnitudes, per side. Like the
//! damage type lock (ADR 0020), both belong to the base and are read from `base_items.json`.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::Deserialize;

use crate::data::BASE_ITEMS_JSON;
use crate::model::AffixType;

/// A value for each side of an item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Sides<T> {
    pub prefixes: T,
    pub suffixes: T,
}

impl<T: Copy> Sides<T> {
    /// The value of the given side.
    pub fn of(&self, affix_type: AffixType) -> T {
        match affix_type {
            AffixType::Prefix => self.prefixes,
            AffixType::Suffix => self.suffixes,
        }
    }
}

/// The part of an entry of `base_items.json` that holds the two fields.
#[derive(Deserialize)]
struct EffectsEntry {
    id: String,
    #[serde(default)]
    affix_limit_changes: Option<Sides<i32>>,
    #[serde(default)]
    magnitude_increases: Option<Sides<u32>>,
}

#[derive(Deserialize)]
struct EffectsFile {
    base_items: Vec<EffectsEntry>,
}

/// The effects of one base. A side without an effect has 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BaseEffects {
    /// Added to the limit of each side (negative values reduce it).
    pub affix_limit_changes: Sides<i32>,
    /// Percent by which the values of the mods of each side are increased.
    pub magnitude_increases: Sides<u32>,
}

const NONE: BaseEffects = BaseEffects {
    affix_limit_changes: Sides {
        prefixes: 0,
        suffixes: 0,
    },
    magnitude_increases: Sides {
        prefixes: 0,
        suffixes: 0,
    },
};

/// The effects of the base with the given identifier: none for a base without the fields, or
/// for an identifier that is not in the game data.
///
/// # Panics
/// If the embedded data is invalid. That is a programming error, caught by the unit tests.
pub(crate) fn effects_of(base_id: &str) -> BaseEffects {
    static EFFECTS: OnceLock<BTreeMap<String, BaseEffects>> = OnceLock::new();
    EFFECTS
        .get_or_init(|| parse_effects(BASE_ITEMS_JSON))
        .get(base_id)
        .copied()
        .unwrap_or(NONE)
}

/// Reads the effects of `base_items.json` by base identifier. Bases without either field are
/// not in the map.
///
/// # Panics
/// If the data cannot be read, for example a field that does not have the form
/// `{ "prefixes": n, "suffixes": n }`.
pub(crate) fn parse_effects(json: &str) -> BTreeMap<String, BaseEffects> {
    let file: EffectsFile = serde_json::from_str(json)
        .unwrap_or_else(|error| panic!("invalid base item data: {error}"));
    file.base_items
        .into_iter()
        .filter(|base| base.affix_limit_changes.is_some() || base.magnitude_increases.is_some())
        .map(|base| {
            let effects = BaseEffects {
                affix_limit_changes: base.affix_limit_changes.unwrap_or(NONE.affix_limit_changes),
                magnitude_increases: base.magnitude_increases.unwrap_or(NONE.magnitude_increases),
            };
            (base.id, effects)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::base_items;

    const BASES: &str = r#"{"base_items": [
        {"id": "both",
         "affix_limit_changes": {"prefixes": -1, "suffixes": 2},
         "magnitude_increases": {"prefixes": 0, "suffixes": 25}},
        {"id": "plain"},
        {"id": "limits", "affix_limit_changes": {"prefixes": 3, "suffixes": -3}},
        {"id": "magnitudes", "magnitude_increases": {"prefixes": 50, "suffixes": 0}}
    ]}"#;

    #[test]
    fn effects_are_read_by_base_and_a_base_without_fields_is_not_listed() {
        let effects = parse_effects(BASES);
        assert_eq!(effects.len(), 3);
        assert_eq!(effects["both"].affix_limit_changes.prefixes, -1);
        assert_eq!(effects["both"].affix_limit_changes.suffixes, 2);
        assert_eq!(effects["both"].magnitude_increases.suffixes, 25);
        assert_eq!(effects.get("plain"), None);
    }

    #[test]
    fn a_missing_field_means_no_change_and_no_increase() {
        let effects = parse_effects(BASES);
        assert_eq!(
            effects["limits"].magnitude_increases,
            NONE.magnitude_increases
        );
        assert_eq!(
            effects["magnitudes"].affix_limit_changes,
            NONE.affix_limit_changes
        );
        assert_eq!(
            effects["magnitudes"]
                .magnitude_increases
                .of(AffixType::Prefix),
            50
        );
        assert_eq!(
            effects["magnitudes"]
                .magnitude_increases
                .of(AffixType::Suffix),
            0
        );
    }

    #[test]
    #[should_panic(expected = "invalid base item data")]
    fn a_field_without_a_side_is_rejected() {
        parse_effects(r#"{"base_items": [{"id": "a", "affix_limit_changes": {"prefixes": 1}}]}"#);
    }

    #[test]
    #[should_panic(expected = "invalid base item data")]
    fn a_field_with_another_key_is_rejected() {
        parse_effects(
            r#"{"base_items": [{"id": "a",
                "magnitude_increases": {"prefixes": 1, "suffixes": 1, "explicit": 1}}]}"#,
        );
    }

    #[test]
    #[should_panic(expected = "invalid base item data")]
    fn a_negative_increase_is_rejected() {
        parse_effects(
            r#"{"base_items": [{"id": "a", "magnitude_increases": {"prefixes": -1, "suffixes": 1}}]}"#,
        );
    }

    #[test]
    fn an_unknown_base_has_no_effects() {
        assert_eq!(effects_of("no/such/base"), NONE);
    }

    #[test]
    fn the_embedded_effects_are_valid_and_name_existing_bases() {
        let effects = parse_effects(BASE_ITEMS_JSON);
        assert!(!effects.is_empty());
        let ids: Vec<String> = base_items().into_iter().map(|base| base.id).collect();
        for id in effects.keys() {
            assert!(ids.contains(id), "{id}");
        }
    }
}
