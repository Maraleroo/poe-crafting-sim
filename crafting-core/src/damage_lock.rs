//! The damage type lock of the Breach rings (ADR 0020, rule "Breach rings" in
//! `poe-crafting-rules`).

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::Deserialize;

use crate::data::BASE_ITEMS_JSON;
use crate::model::{ExplicitModDefinition, Item};

/// The five damage types of the PO's rule, as tags of the game data. Other tags, such as
/// `elemental`, `physical_damage` or `elemental_damage`, are not damage types. This is the only
/// place of the list.
const DAMAGE_TYPES: [&str; 5] = ["physical", "lightning", "cold", "fire", "chaos"];

/// The part of an entry of `base_items.json` that holds the lock: the tag of a damage type,
/// absent for a base without a lock.
#[derive(Deserialize)]
struct LockEntry {
    id: String,
    #[serde(default)]
    damage_type_lock: Option<String>,
}

#[derive(Deserialize)]
struct LocksFile {
    base_items: Vec<LockEntry>,
}

/// The mods of `mods` that the damage type lock of the base with the given identifier does not
/// block, in their order. A mod is blocked if one of its tags is one of the four damage types
/// other than the lock's. A base without a lock, or an identifier that is not in the game data,
/// blocks nothing. A blocked mod can never roll on the base.
///
/// `valid_pool` applies the same block when a currency adds a random mod; the currencies need
/// no pool filtered with this function.
///
/// # Panics
/// If the embedded data is invalid, for example a lock that is not one of the five damage
/// types. That is a programming error, caught by the unit tests.
pub fn unblocked_mods(base_id: &str, mods: &[ExplicitModDefinition]) -> Vec<ExplicitModDefinition> {
    let blocked = blocked_tags_of_base(base_id);
    mods.iter()
        .filter(|definition| !is_blocked(&blocked, definition))
        .cloned()
        .collect()
}

/// The tags that an item blocks (ADR 0021): a mod with one of them can never roll on the item.
/// The item collects them from its sources. Today the only source is the damage type lock of
/// its base.
pub(crate) fn blocked_tags(item: &Item) -> Vec<&'static str> {
    blocked_tags_of_base(&item.base.id)
}

/// The tags that the damage type lock of the base blocks: the four damage types other than
/// the lock's, or none for a base without a lock.
fn blocked_tags_of_base(base_id: &str) -> Vec<&'static str> {
    tags_blocked_by(locks().get(base_id).copied())
}

/// The tags that a lock on the given damage type blocks.
fn tags_blocked_by(lock: Option<&str>) -> Vec<&'static str> {
    match lock {
        Some(lock) => DAMAGE_TYPES
            .into_iter()
            .filter(|damage_type| *damage_type != lock)
            .collect(),
        None => Vec::new(),
    }
}

/// The locks of the embedded game data, read once.
///
/// # Panics
/// If the embedded data is invalid.
fn locks() -> &'static BTreeMap<String, &'static str> {
    static LOCKS: OnceLock<BTreeMap<String, &'static str>> = OnceLock::new();
    LOCKS.get_or_init(|| parse_locks(BASE_ITEMS_JSON))
}

/// Whether the mod has one of the blocked tags.
pub(crate) fn is_blocked(blocked: &[&str], definition: &ExplicitModDefinition) -> bool {
    definition
        .definition
        .tags
        .iter()
        .any(|tag| blocked.contains(&tag.as_str()))
}

/// Reads the locks of `base_items.json` by base identifier. Bases without a lock are not in the
/// map.
///
/// # Panics
/// If the data cannot be read or a lock is not one of the five damage types.
pub(crate) fn parse_locks(json: &str) -> BTreeMap<String, &'static str> {
    let file: LocksFile = serde_json::from_str(json)
        .unwrap_or_else(|error| panic!("invalid base item data: {error}"));
    let mut locks = BTreeMap::new();
    for base in file.base_items {
        let Some(tag) = base.damage_type_lock else {
            continue;
        };
        let damage_type = DAMAGE_TYPES
            .iter()
            .find(|damage_type| **damage_type == tag)
            .unwrap_or_else(|| {
                panic!(
                    "{}: the damage type lock {tag:?} is not one of {DAMAGE_TYPES:?}",
                    base.id
                )
            });
        locks.insert(base.id, *damage_type);
    }
    locks
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::base_items;
    use crate::model::{AffixType, ModDefinition, StatDefinition};

    const BASES: &str = r#"{"base_items": [
        {"id": "cold", "damage_type_lock": "cold"},
        {"id": "plain"},
        {"id": "chaos", "damage_type_lock": "chaos"}
    ]}"#;

    fn invented_mod(name: &str, tags: &[&str]) -> ExplicitModDefinition {
        ExplicitModDefinition {
            name: name.to_owned(),
            affix_type: AffixType::Prefix,
            weight: 1,
            definition: ModDefinition {
                family: name.to_owned(),
                level: 1,
                tags: tags.iter().map(|tag| (*tag).to_owned()).collect(),
                stats: vec![StatDefinition {
                    id: "stat".to_owned(),
                    min: 1,
                    max: 2,
                }],
                text: "{value}".to_owned(),
                placeholders: BTreeMap::from([("value".to_owned(), "stat".to_owned())]),
                conversions: BTreeMap::new(),
            },
        }
    }

    fn kept(lock: Option<&str>, tags: &[&str]) -> bool {
        !is_blocked(&tags_blocked_by(lock), &invented_mod("m", tags))
    }

    #[test]
    fn locks_are_read_by_base_and_a_base_without_a_lock_is_not_listed() {
        let locks = parse_locks(BASES);
        assert_eq!(locks.len(), 2);
        assert_eq!(locks.get("cold"), Some(&"cold"));
        assert_eq!(locks.get("chaos"), Some(&"chaos"));
        assert_eq!(locks.get("plain"), None);
    }

    #[test]
    #[should_panic(expected = "is not one of")]
    fn a_lock_that_is_not_a_damage_type_is_rejected() {
        parse_locks(r#"{"base_items": [{"id": "a", "damage_type_lock": "elemental"}]}"#);
    }

    #[test]
    #[should_panic(expected = "is not one of")]
    fn a_lock_in_upper_case_is_rejected() {
        parse_locks(r#"{"base_items": [{"id": "a", "damage_type_lock": "Cold"}]}"#);
    }

    #[test]
    fn a_mod_with_a_tag_of_another_damage_type_is_blocked() {
        for other in ["physical", "lightning", "fire", "chaos"] {
            assert!(!kept(Some("cold"), &[other]), "{other}");
            assert!(!kept(Some("cold"), &["damage", other]), "{other}");
            assert!(!kept(Some("cold"), &["cold", other]), "{other}");
            assert!(!kept(Some("cold"), &[other, "cold"]), "{other}");
        }
    }

    #[test]
    fn a_mod_of_the_own_damage_type_or_without_a_damage_type_is_kept() {
        assert!(kept(Some("cold"), &["cold"]));
        assert!(kept(Some("cold"), &["damage", "cold", "attack"]));
        assert!(kept(Some("cold"), &[]));
        for tag in ["elemental", "physical_damage", "elemental_damage", "Fire"] {
            assert!(kept(Some("cold"), &[tag, "resistance"]), "{tag}");
        }
    }

    #[test]
    fn without_a_lock_nothing_is_blocked() {
        for tag in DAMAGE_TYPES {
            assert!(kept(None, &[tag]), "{tag}");
        }
        assert!(kept(None, &["physical", "fire"]));
    }

    #[test]
    fn unblocked_mods_keeps_the_order_and_an_unknown_base_blocks_nothing() {
        let mods = vec![
            invented_mod("a", &["fire"]),
            invented_mod("b", &["cold"]),
            invented_mod("c", &[]),
            invented_mod("d", &["chaos"]),
        ];
        assert_eq!(unblocked_mods("no/such/base", &mods), mods);
        assert!(unblocked_mods("no/such/base", &[]).is_empty());
    }

    #[test]
    fn the_embedded_locks_are_valid_and_name_existing_bases() {
        let locks = parse_locks(BASE_ITEMS_JSON);
        assert!(!locks.is_empty());
        let ids: Vec<String> = base_items().into_iter().map(|base| base.id).collect();
        for id in locks.keys() {
            assert!(ids.contains(id), "{id}");
        }
    }
}
