//! The game data files, embedded at build time (ADR 0008).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use serde::Deserialize;

use crate::base_effects::parse_effects;
use crate::craft::CraftError;
use crate::currency::CurrencyDefinition;
use crate::damage_lock::parse_locks;
use crate::model::{BaseItem, ExplicitModDefinition, ModDefinition};
use crate::slot::Slot;

pub(crate) const BASE_ITEMS_JSON: &str = include_str!("../../data/base_items.json");
pub(crate) const MODS_JSON: &str = include_str!("../../data/mods.json");
const CURRENCIES_JSON: &str = include_str!("../../data/currencies.json");
const ITEM_SLOTS_JSON: &str = include_str!("../../data/item_slots.json");

#[derive(Deserialize)]
struct BaseItemsFile {
    base_items: Vec<BaseItem>,
}

#[derive(Deserialize)]
struct ModsFile {
    mods: Vec<ExplicitModDefinition>,
}

/// The part of an entry of `base_items.json` that holds the pool: its mods, by `id`.
#[derive(Deserialize)]
struct PoolEntry {
    id: String,
    #[serde(default)]
    mod_pool: Vec<String>,
}

#[derive(Deserialize)]
struct PoolsFile {
    base_items: Vec<PoolEntry>,
}

/// The part of an entry of `mods.json` that identifies the mod.
#[derive(Deserialize)]
struct ModIdEntry {
    id: String,
}

#[derive(Deserialize)]
struct ModIdsFile {
    mods: Vec<ModIdEntry>,
}

/// An entry of `item_slots.json`: the item classes of a slot.
#[derive(Deserialize)]
struct SlotEntry {
    id: String,
    item_classes: Vec<String>,
}

#[derive(Deserialize)]
struct SlotsFile {
    slots: Vec<SlotEntry>,
}

#[derive(Deserialize)]
struct CurrenciesFile {
    currencies: Vec<CurrencyDefinition>,
}

/// The parsed and validated game data. It is read once, at the first access.
struct GameData {
    bases: Vec<BaseItem>,
    mods: Vec<ExplicitModDefinition>,
    /// The position in `mods` of each mod, by the `id` in `mods.json`.
    mod_positions: BTreeMap<String, usize>,
    /// The pool of each base, by base id.
    pools: BTreeMap<String, Pool>,
    currencies: Vec<CurrencyDefinition>,
    item_classes: BTreeMap<String, Vec<String>>,
}

/// The pool of a base: the ids of its mods, in the order of the game data, and the mods
/// themselves, resolved at the first access to this pool.
struct Pool {
    mod_ids: Vec<String>,
    mods: OnceLock<Vec<ExplicitModDefinition>>,
}

impl GameData {
    fn parse() -> GameData {
        let bases = parse_base_items(BASE_ITEMS_JSON);
        let mods = parse_mods(MODS_JSON);
        let mod_positions = mod_positions(MODS_JSON);
        let pools = read_pools(BASE_ITEMS_JSON)
            .into_iter()
            .map(|entry| {
                let pool = Pool {
                    mod_ids: entry.mod_pool,
                    mods: OnceLock::new(),
                };
                (entry.id, pool)
            })
            .collect();
        let item_classes = parse_item_slots(ITEM_SLOTS_JSON, &bases);
        let currencies = parse_currencies(CURRENCIES_JSON);
        GameData {
            bases,
            mods,
            mod_positions,
            pools,
            currencies,
            item_classes,
        }
    }
}

fn game_data() -> &'static GameData {
    static DATA: OnceLock<GameData> = OnceLock::new();
    DATA.get_or_init(GameData::parse)
}

/// All base items of the game data.
///
/// # Panics
/// If the embedded data is invalid. That is a programming error, caught by the unit tests.
pub fn base_items() -> Vec<BaseItem> {
    game_data().bases.clone()
}

/// The base item with the given identifier, if there is one.
pub fn base_item(id: &str) -> Option<BaseItem> {
    all_base_items().iter().find(|base| base.id == id).cloned()
}

/// All prefixes and suffixes of the game data.
///
/// # Panics
/// If the embedded data is invalid. That is a programming error, caught by the unit tests.
pub fn explicit_mods() -> Vec<ExplicitModDefinition> {
    game_data().mods.clone()
}

/// The mods that can roll on the base item with the given identifier, in the order of the
/// base's `mod_pool` in the game data (ADR 0017). `None` if there is no such base.
///
/// # Panics
/// If the embedded data is invalid, for example a pool names a mod that does not exist. That is
/// a programming error, caught by the unit tests.
pub fn mod_pool(base_id: &str) -> Option<Vec<ExplicitModDefinition>> {
    pool_of(base_id).map(<[ExplicitModDefinition]>::to_vec)
}

/// All currencies of the game data.
///
/// # Panics
/// If the embedded data is invalid. That is a programming error, caught by the unit tests.
pub fn currencies() -> Vec<CurrencyDefinition> {
    currency_definitions().to_vec()
}

/// All base items, read from the game data once and not copied.
pub(crate) fn all_base_items() -> &'static [BaseItem] {
    &game_data().bases
}

/// Like [`mod_pool`], read from the game data once and not copied.
pub(crate) fn pool_of(base_id: &str) -> Option<&'static [ExplicitModDefinition]> {
    let data = game_data();
    let pool = data.pools.get(base_id)?;
    let mods = pool.mods.get_or_init(|| {
        pool.mod_ids
            .iter()
            .map(|id| resolve_mod(base_id, id, &data.mod_positions, &data.mods))
            .collect()
    });
    Some(mods)
}

/// Like [`currencies`], read from the game data once and not copied.
pub(crate) fn currency_definitions() -> &'static [CurrencyDefinition] {
    &game_data().currencies
}

/// The item classes of the slot with the given identifier (`Slot::id`): those of its entry in
/// `item_slots.json`, none if it has no entry.
///
/// # Panics
/// If the embedded data is invalid. That is a programming error, caught by the unit tests.
pub(crate) fn slot_item_classes(slot_id: &str) -> &'static [String] {
    game_data()
        .item_classes
        .get(slot_id)
        .map_or(&[], Vec::as_slice)
}

/// Reads `item_slots.json` and checks it against the bases: every slot id is known and listed
/// once, no item class belongs to two slots or is unknown to the bases, and the item class of
/// every base belongs to a slot.
fn parse_item_slots(json: &str, bases: &[BaseItem]) -> BTreeMap<String, Vec<String>> {
    let file: SlotsFile = serde_json::from_str(json)
        .unwrap_or_else(|error| panic!("invalid item slot data: {error}"));
    let mut by_slot: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut slot_of_class: BTreeMap<String, String> = BTreeMap::new();
    for entry in file.slots {
        assert!(
            Slot::ALL.iter().any(|slot| slot.id() == entry.id),
            "item slots: {:?} is not a slot",
            entry.id
        );
        for class in &entry.item_classes {
            assert!(
                bases.iter().any(|base| base.item_class == *class),
                "item slots: no base item has the item class {class:?}"
            );
            if let Some(other) = slot_of_class.insert(class.clone(), entry.id.clone()) {
                panic!(
                    "item slots: the item class {class:?} is listed for {other:?} and {:?}",
                    entry.id
                );
            }
        }
        assert!(
            by_slot
                .insert(entry.id.clone(), entry.item_classes)
                .is_none(),
            "item slots: the slot {:?} is listed twice",
            entry.id
        );
    }
    for base in bases {
        assert!(
            slot_of_class.contains_key(&base.item_class),
            "item slots: the item class {:?} of {:?} belongs to no slot",
            base.item_class,
            base.id
        );
    }
    by_slot
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

/// Reads the pool entries of `base_items.json`.
fn read_pools(base_items_json: &str) -> Vec<PoolEntry> {
    let pools: PoolsFile = serde_json::from_str(base_items_json)
        .unwrap_or_else(|error| panic!("invalid base item data: {error}"));
    pools.base_items
}

/// The position of each mod of `mods.json` by its `id`, the same as in [`parse_mods`].
fn mod_positions(mods_json: &str) -> BTreeMap<String, usize> {
    let ids: ModIdsFile =
        serde_json::from_str(mods_json).unwrap_or_else(|error| panic!("invalid mod data: {error}"));
    let mut positions = BTreeMap::new();
    for (position, entry) in ids.mods.into_iter().enumerate() {
        assert!(
            !positions.contains_key(&entry.id),
            "duplicate mod id {:?} in the mod data",
            entry.id
        );
        positions.insert(entry.id, position);
    }
    positions
}

/// The mod with the given id, for the pool of `base_id`.
fn resolve_mod(
    base_id: &str,
    id: &str,
    positions: &BTreeMap<String, usize>,
    mods: &[ExplicitModDefinition],
) -> ExplicitModDefinition {
    positions
        .get(id)
        .map(|position| mods[*position].clone())
        .unwrap_or_else(|| {
            panic!(
                "the pool of {base_id:?} names the mod {id:?}, \
                 which the mod data does not have"
            )
        })
}

#[cfg(test)]
fn parse_mod_pool(
    base_items_json: &str,
    mods_json: &str,
    base_id: &str,
) -> Option<Vec<ExplicitModDefinition>> {
    let pool = read_pools(base_items_json)
        .into_iter()
        .find(|base| base.id == base_id)?;
    let mods = parse_mods(mods_json);
    let positions = mod_positions(mods_json);
    Some(
        pool.mod_pool
            .iter()
            .map(|id| resolve_mod(base_id, id, &positions, &mods))
            .collect(),
    )
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
    // Checks the damage type locks (ADR 0020).
    parse_locks(json);
    // Checks the affix limit changes and magnitude increases (ADR 0022).
    parse_effects(json);
    let mut ids = BTreeSet::new();
    for base in &file.base_items {
        assert!(ids.insert(&base.id), "duplicate base item {:?}", base.id);
        for implicit in &base.implicits {
            validate_mod(&base.id, implicit);
            // The text of a base is shown without rolled values (ADR 0018). The rule does not
            // define every range, so the data must not hold one it does not define.
            if let Err(message) = implicit.try_range_text() {
                panic!("{}: {message}", base.id);
            }
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
    for (stat_id, conversion) in &definition.conversions {
        assert!(
            definition.stats.iter().any(|stat| stat.id == *stat_id),
            "{owner}: the conversion names the unknown stat {stat_id:?}"
        );
        if let Err(message) = conversion.check() {
            panic!("{owner}: invalid conversion of stat {stat_id:?}: {message}");
        }
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
    use crate::conversion::Conversion;
    use crate::model::AffixType;

    const VALID: &str = r#"{"base_items": [{
        "id": "a", "name": "A", "item_class": "C", "required_level": 1, "tags": [],
        "implicits": [{
            "family": "F", "level": 2, "tags": [],
            "stats": [{"id": "s", "min": 1, "max": 2}],
            "text": "+{v}", "placeholders": {"v": "s"}, "conversions": {}
        }]
    }]}"#;

    const VALID_MODS: &str = r#"{"mods": [{
        "name": "Invented", "affix_type": "suffix", "weight": 10,
        "family": "F", "level": 1, "tags": [],
        "stats": [{"id": "s", "min": 1, "max": 2}],
        "text": "+{v}", "placeholders": {"v": "s"}, "conversions": {}
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
    fn conversions_are_read_with_and_without_a_precision() {
        let json = VALID_MODS.replace(
            r#""conversions": {}"#,
            r#""conversions": {"s": {"divisor": 60, "precision": 1}}"#,
        );
        let mods = parse_mods(&json);
        assert_eq!(
            mods[0].definition.conversions["s"],
            Conversion {
                divisor: 60,
                precision: Some(1)
            }
        );
        let json = VALID_MODS.replace(
            r#""conversions": {}"#,
            r#""conversions": {"s": {"divisor": 100, "precision": null}}"#,
        );
        assert_eq!(
            parse_mods(&json)[0].definition.conversions["s"].precision,
            None
        );
    }

    #[test]
    #[should_panic(expected = "unknown stat")]
    fn conversion_of_an_unknown_stat_is_rejected() {
        parse_mods(&VALID_MODS.replace(
            r#""conversions": {}"#,
            r#""conversions": {"other": {"divisor": 100, "precision": null}}"#,
        ));
    }

    #[test]
    #[should_panic(expected = "invalid conversion")]
    fn conversion_without_an_exact_decimal_form_is_rejected() {
        parse_mods(&VALID_MODS.replace(
            r#""conversions": {}"#,
            r#""conversions": {"s": {"divisor": 60, "precision": null}}"#,
        ));
    }

    #[test]
    #[should_panic(expected = "invalid mod data")]
    fn mod_without_a_level_is_rejected() {
        parse_mods(&VALID_MODS.replace(r#""level": 1, "#, ""));
    }

    #[test]
    fn every_embedded_mod_has_a_positive_weight_and_a_unique_name() {
        let mods = explicit_mods();
        let names: BTreeSet<&str> = mods.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names.len(), mods.len());
        assert!(mods.iter().all(|m| m.weight > 0));
    }

    const POOL_BASES: &str = r#"{"base_items": [
        {"id": "a", "mod_pool": ["m2", "m1"]},
        {"id": "b", "mod_pool": []},
        {"id": "c", "mod_pool": ["missing"]}
    ]}"#;

    fn two_mods(second_id: &str) -> String {
        let entry = |id: &str, name: &str| {
            format!(
                r#"{{"id": "{id}", "name": "{name}", "affix_type": "suffix", "weight": 10,
                "family": "F", "level": 1, "tags": [],
                "stats": [{{"id": "s", "min": 1, "max": 2}}],
                "text": "+{{v}}", "placeholders": {{"v": "s"}}, "conversions": {{}}}}"#
            )
        };
        format!(
            r#"{{"mods": [{}, {}]}}"#,
            entry("m1", "One"),
            entry(second_id, "Two")
        )
    }

    fn pool_names(base_id: &str, mods_json: &str) -> Option<Vec<String>> {
        parse_mod_pool(POOL_BASES, mods_json, base_id)
            .map(|pool| pool.into_iter().map(|m| m.name).collect())
    }

    #[test]
    fn a_pool_lists_its_mods_in_the_order_of_the_base() {
        assert_eq!(
            pool_names("a", &two_mods("m2")),
            Some(vec!["Two".to_owned(), "One".to_owned()])
        );
        assert_eq!(pool_names("b", &two_mods("m2")), Some(Vec::new()));
    }

    #[test]
    fn an_unknown_base_has_no_pool() {
        assert_eq!(pool_names("unknown", &two_mods("m2")), None);
        assert_eq!(mod_pool("does/not/exist"), None);
    }

    #[test]
    #[should_panic(expected = "which the mod data does not have")]
    fn a_pool_with_an_unknown_mod_is_rejected() {
        pool_names("c", &two_mods("m2"));
    }

    #[test]
    #[should_panic(expected = "duplicate mod id")]
    fn two_mods_with_the_same_id_are_rejected() {
        pool_names("a", &two_mods("m1"));
    }

    #[test]
    fn every_embedded_pool_resolves_and_holds_each_mod_once() {
        for base in base_items() {
            let pool = mod_pool(&base.id).expect("the base exists");
            assert!(!pool.is_empty(), "{}", base.id);
            let names: BTreeSet<&str> = pool.iter().map(|m| m.name.as_str()).collect();
            assert_eq!(names.len(), pool.len(), "{}", base.id);
        }
    }

    fn invented_base(id: &str, item_class: &str) -> BaseItem {
        BaseItem {
            id: id.to_owned(),
            name: id.to_owned(),
            item_class: item_class.to_owned(),
            required_level: 0,
            tags: Vec::new(),
            implicits: Vec::new(),
        }
    }

    fn invented_bases() -> Vec<BaseItem> {
        vec![invented_base("a", "Rings"), invented_base("b", "Amulets")]
    }

    #[test]
    fn item_slots_assign_item_classes_to_slots() {
        let json = r#"{"slots": [
            {"id": "ring", "item_classes": ["Rings"]},
            {"id": "amulet", "item_classes": ["Amulets"]}
        ]}"#;
        let by_slot = parse_item_slots(json, &invented_bases());
        assert_eq!(by_slot["ring"], vec!["Rings".to_owned()]);
        assert_eq!(by_slot["amulet"], vec!["Amulets".to_owned()]);
        assert!(!by_slot.contains_key("boots"));
    }

    #[test]
    #[should_panic(expected = "is not a slot")]
    fn an_unknown_slot_is_rejected() {
        let json = r#"{"slots": [{"id": "tail", "item_classes": []}]}"#;
        parse_item_slots(json, &[]);
    }

    #[test]
    #[should_panic(expected = "listed twice")]
    fn a_repeated_slot_is_rejected() {
        let json = r#"{"slots": [
            {"id": "ring", "item_classes": ["Rings"]},
            {"id": "ring", "item_classes": []}
        ]}"#;
        parse_item_slots(json, &invented_bases());
    }

    #[test]
    #[should_panic(expected = "is listed for")]
    fn an_item_class_of_two_slots_is_rejected() {
        let json = r#"{"slots": [
            {"id": "ring", "item_classes": ["Rings"]},
            {"id": "amulet", "item_classes": ["Rings", "Amulets"]}
        ]}"#;
        parse_item_slots(json, &invented_bases());
    }

    #[test]
    #[should_panic(expected = "no base item has the item class")]
    fn an_item_class_without_a_base_is_rejected() {
        let json = r#"{"slots": [{"id": "belt", "item_classes": ["Belts"]}]}"#;
        parse_item_slots(json, &[]);
    }

    #[test]
    #[should_panic(expected = "belongs to no slot")]
    fn a_base_whose_item_class_belongs_to_no_slot_is_rejected() {
        let json = r#"{"slots": [{"id": "ring", "item_classes": ["Rings"]}]}"#;
        parse_item_slots(json, &invented_bases());
    }

    #[test]
    fn the_embedded_item_slots_are_valid() {
        assert_eq!(slot_item_classes("ring"), vec!["Rings".to_owned()]);
        assert!(slot_item_classes("belt").is_empty());
    }

    #[test]
    #[should_panic(expected = "not defined by poe-crafting-rules")]
    fn a_base_whose_implicit_has_an_undefined_range_is_rejected() {
        parse_base_items(&VALID.replace(
            r#""conversions": {}"#,
            r#""conversions": {"s": {"divisor": 100, "precision": null}}"#,
        ));
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
        parse_base_items(&VALID.replace(r#"[{"id": "s", "min": 1, "max": 2}]"#, "[]"));
    }
}
