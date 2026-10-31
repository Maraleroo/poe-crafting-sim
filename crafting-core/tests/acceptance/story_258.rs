//! Acceptance tests for AB#258 "Normale Ansicht: Mod-Zeilen sortieren und zusammenfassen".
//!
//! The tests use the public interface from the tech lead's plan on AB#258 and ADR 0023:
//! - `Item::explicit_texts(&self) -> Vec<String>`: the explicit lines of the item in the normal
//!   view, one entry per line, sorted by the position of their stats and with the lines of
//!   different mods that name the same stats merged; implicit lines are not part of it.
//! - `data/mods.json`: the new top-level object `stat_order` `{ <stat id>: <position> }`, the
//!   0-based position of the entry of `stat_translations.json` that shows the stat; one key for
//!   every stat that a line of a mod in the file shows.
//! - `data-import` (dev-dependency of `crafting-core`): the unchanged `convert(raw, base_ids)`,
//!   which writes `stat_order`.
//! - unchanged: `Item::explicit_text`, `Mod::display_text`, `Item::name`, `add_random_mod`,
//!   `mod_pool`, `base_item`, `Slot::Ring.base_items()`.
//!
//! The PO's rules are `poe-crafting-rules`, "Item basics": the paragraph "In the normal view, the
//! explicit mod lines ..." (criterion 1) and the paragraph on "X% increased ... Modifier
//! magnitudes" (AB#189). The game's order of the stats (`STAT_ORDER`) was taken by a program from
//! the PO's RePoE export 3.29.3.3 (the raw files of `data/README.md`), not from `data/mods.json`;
//! the raw files are not in the repository. The tests use only the order of the entries, not
//! their positions, so that an export that only inserts other entries keeps them green.
//!
//! Criterion 1 gives the PO's wording for `poe-crafting-rules`. That wording is checked byte for
//! byte at the acceptance (step 2 of the `story` skill), not here; the tests check the behaviour
//! of the rule.
//!
//! Criterion 2 replaces criterion 6 of AB#189 (PO in the story). The test
//! `story_189::ac6_each_added_mod_comes_last_on_its_side` stays unchanged: it checks the order of
//! the lists `prefixes` and `suffixes`, which the item name still uses; only the view sorts.
//!
//! Every random test uses a fixed seed and depends only on invariants, never on the order in
//! which the generator is used.
//!
//! Red state before the implementation: the module does not compile, because
//! `Item::explicit_texts` does not exist yet.
//!
//! What is checked outside `cargo test`, by the PO in the local preview (tech lead's plan):
//! - AC2, AC3: the order and the merged line as the page shows them (here: the page uses
//!   `explicit_texts`).
//! - AC4: the implicits above the explicit lines on the page (also a unit test in `web`).
//!
//! Not testable: the part "a fractured mod is not merged and keeps its own line" of criterion 1,
//! because the simulator has no fractured mods (tech lead's plan).

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crafting_core::{
    add_random_mod, base_item, mod_pool, AffixType, BaseItem, ExplicitMod, ExplicitModDefinition,
    Item, Mod, Rarity, SeededRng, Slot,
};
use data_import::{convert, RawFiles};
use serde_json::{json, Value};

// ---------------------------------------------------------------------------------------
// Values from the story, the PO's rules and the RePoE export 3.29.3.3
// ---------------------------------------------------------------------------------------

/// The entries of `stat_translations.json` that show a line of a mod in `data/mods.json`, in the
/// order of that file, each with its stats. The comment gives the entry's position in the export
/// 3.29.3.3; every stat is in exactly one entry there. 41 entries with 45 stats.
const STAT_ORDER: [&[&str]; 41] = [
    // entry 167
    &["local_socketed_gem_level_+"],
    // entry 1201
    &["additional_all_attributes"],
    // entry 1202
    &["additional_strength"],
    // entry 1203
    &["additional_dexterity"],
    // entry 1204
    &["additional_intelligence"],
    // entry 1292
    &[
        "attack_minimum_added_physical_damage",
        "attack_maximum_added_physical_damage",
    ],
    // entry 1383
    &["fire_damage_+%"],
    // entry 1386
    &[
        "attack_minimum_added_fire_damage",
        "attack_maximum_added_fire_damage",
    ],
    // entry 1392
    &["cold_damage_+%"],
    // entry 1395
    &[
        "attack_minimum_added_cold_damage",
        "attack_maximum_added_cold_damage",
    ],
    // entry 1403
    &["lightning_damage_+%"],
    // entry 1406
    &[
        "attack_minimum_added_lightning_damage",
        "attack_maximum_added_lightning_damage",
    ],
    // entry 1436
    &["attack_speed_+%"],
    // entry 1459
    &["accuracy_rating"],
    // entry 1460
    &["accuracy_rating_+%"],
    // entry 1472
    &["base_cast_speed_+%"],
    // entry 1568
    &["base_evasion_rating"],
    // entry 1582
    &["base_maximum_energy_shield"],
    // entry 1593
    &["base_maximum_life"],
    // entry 1598
    &["base_life_regeneration_rate_per_minute"],
    // entry 1604
    &["base_maximum_mana"],
    // entry 1609
    &["mana_regeneration_rate_+%"],
    // entry 1621
    &["base_item_found_rarity_+%"],
    // entry 1644
    &["base_resist_all_elements_%"],
    // entry 1650
    &["base_fire_damage_resistance_%"],
    // entry 1656
    &["base_cold_damage_resistance_%"],
    // entry 1661
    &["base_lightning_damage_resistance_%"],
    // entry 1666
    &["base_chaos_damage_resistance_%"],
    // entry 1674
    &["life_leech_from_physical_attack_damage_permyriad"],
    // entry 1724
    &["mana_leech_from_physical_attack_damage_permyriad"],
    // entry 1765
    &["base_life_gain_per_target"],
    // entry 1773
    &["base_life_gained_on_enemy_death"],
    // entry 1788
    &["base_mana_gained_on_enemy_death"],
    // entry 1791
    &["minion_maximum_life_+%"],
    // entry 1794
    &["minion_movement_speed_+%"],
    // entry 1996
    &["minion_damage_+%"],
    // entry 2526
    &["light_radius_+%"],
    // entry 2942
    &["minion_elemental_resistance_%"],
    // entry 6190
    &["damage_taken_goes_to_life_over_4_seconds_%"],
    // entry 6412
    &["elemental_damage_with_attack_skills_+%"],
    // entry 9404
    &["minion_attack_and_cast_speed_+%"],
];

const IRON_RING_ID: &str = "Metadata/Items/Rings/Ring1";
/// The ring of the PO's evidence "Sanguine Sapphire Ring of Haast": its implicit is Cold
/// Resistance.
const SAPPHIRE_RING_ID: &str = "Metadata/Items/Rings/Ring6";
/// Heist rings with "25% increased Suffix Modifier magnitudes" and "25% increased Prefix Modifier
/// magnitudes" (AB#189).
const COGWORK_RING_ID: &str = "Metadata/Items/Rings/RingE1";
const GEODESIC_RING_ID: &str = "Metadata/Items/Rings/RingE2";

/// The increases of the Heist rings by side, as (ring, prefix %, suffix %), from the RePoE export
/// (AB#189). Every other ring base has none.
const MAGNITUDE_INCREASES: [(&str, u32, u32); 4] = [
    (COGWORK_RING_ID, 0, 25),
    (GEODESIC_RING_ID, 25, 0),
    ("Metadata/Items/Rings/RingE4", 50, 0),
    ("Metadata/Items/Rings/RingE6", 0, 50),
];

const LIFE_STAT: &str = "base_maximum_life";
const STRENGTH_STAT: &str = "additional_strength";
const RARITY_STAT: &str = "base_item_found_rarity_+%";
const COLD_RESISTANCE_STAT: &str = "base_cold_damage_resistance_%";

/// The rarity mods of the Iron Ring's pool: the prefixes (family
/// `ItemFoundRarityIncreasePrefix`) and the suffixes (family `ItemFoundRarityIncrease`).
const RARITY_PREFIXES: [&str; 4] = ["Magpie's", "Pirate's", "Dragon's", "Perandus'"];
const RARITY_SUFFIXES: [&str; 4] = [
    "of Plunder",
    "of Raiding",
    "of Archaeology",
    "of Excavation",
];

/// A fixed seed, so every run is identical.
const SEED: u64 = 258;
/// Rare items per ring base filled with random mods (AC1). 43 bases give 1290 items with up to
/// six mods each, enough for lines of many stats in every order and for several items with both
/// rarity mods (each has weight 1000 in the pool).
const FILLED_ITEMS: u64 = 30;
/// Magic items per combination of rarity prefix and suffix and order of adding (AC3).
const RARITY_ITEMS: u64 = 5;

// ---------------------------------------------------------------------------------------
// Helpers: files
// ---------------------------------------------------------------------------------------

fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crafting-core must live inside the repository root")
}

fn load_json(relative: &str) -> Value {
    let path = repo_root().join(relative);
    let text =
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{relative} is not valid JSON: {e}"))
}

/// The object `stat_order` of a `mods.json`, as stat identifier to position.
fn stat_order_of(mods_file: &Value, what: &str) -> BTreeMap<String, u64> {
    let object = mods_file
        .get("stat_order")
        .and_then(Value::as_object)
        .unwrap_or_else(|| panic!("{what} must have the top-level object `stat_order`"));
    object
        .iter()
        .map(|(stat, position)| {
            let position = position.as_u64().unwrap_or_else(|| {
                panic!("{what}: the position of {stat:?} must be a whole number of at least 0")
            });
            (stat.clone(), position)
        })
        .collect()
}

/// All files under `web/src/`.
fn web_src_files() -> Vec<PathBuf> {
    fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
        let entries =
            fs::read_dir(dir).unwrap_or_else(|e| panic!("cannot list {}: {e}", dir.display()));
        for entry in entries {
            let path = entry
                .unwrap_or_else(|e| panic!("cannot read entry in {}: {e}", dir.display()))
                .path();
            if path.is_dir() {
                collect(&path, out);
            } else {
                out.push(path);
            }
        }
    }
    let mut files = Vec::new();
    collect(&repo_root().join("web").join("src"), &mut files);
    assert!(!files.is_empty(), "web/src/ must contain files");
    files
}

fn assert_web_src_uses(needle: &str, why: &str) {
    let used = web_src_files().iter().any(|path| {
        let bytes = fs::read(path).unwrap_or_else(|e| panic!("cannot read {path:?}: {e}"));
        String::from_utf8_lossy(&bytes).contains(needle)
    });
    assert!(used, "a file under web/src/ must use `{needle}`: {why}");
}

// ---------------------------------------------------------------------------------------
// Helpers: model
// ---------------------------------------------------------------------------------------

fn base(id: &str) -> BaseItem {
    base_item(id).unwrap_or_else(|| panic!("crafting-core must offer the base item {id:?}"))
}

fn pool_of(id: &str) -> Vec<ExplicitModDefinition> {
    mod_pool(id).unwrap_or_else(|| panic!("crafting-core must offer the pool of {id:?}"))
}

fn find<'a>(pool: &'a [ExplicitModDefinition], name: &str) -> &'a ExplicitModDefinition {
    pool.iter()
        .find(|definition| definition.name == name)
        .unwrap_or_else(|| panic!("the pool must contain the mod {name:?}"))
}

/// A new item of `base` with the given rarity and without prefixes and suffixes.
fn item_of(base: &BaseItem, rarity: Rarity, rng: &mut SeededRng) -> Item {
    let mut item = Item::new(base, rng);
    item.rarity = rarity;
    item
}

/// The mod `name` of `pool`, rolled and then given the raw value `value` for its only stat
/// `stat`, which must lie in its range.
fn explicit_with(
    pool: &[ExplicitModDefinition],
    name: &str,
    stat: &str,
    value: i32,
    rng: &mut SeededRng,
) -> ExplicitMod {
    let definition = find(pool, name);
    let mut modifier = Mod::roll(&definition.definition, rng);
    set_value(&mut modifier, stat, value, name);
    ExplicitMod {
        name: definition.name.clone(),
        affix_type: definition.affix_type,
        modifier,
    }
}

/// Sets the value of the only stat of `modifier`, which must be `stat` with `value` in its range.
fn set_value(modifier: &mut Mod, stat: &str, value: i32, what: &str) {
    assert_eq!(modifier.stats.len(), 1, "{what}: a mod with one stat");
    let rolled = &mut modifier.stats[0];
    assert_eq!(rolled.id, stat, "{what}: the stat of the example");
    assert!(
        (rolled.min..=rolled.max).contains(&value),
        "{what}: {value} lies outside {}..={} of {stat:?}",
        rolled.min,
        rolled.max
    );
    rolled.value = value;
}

/// Adds the mod `name` of `pool` with `add_random_mod`, as the only mod it can choose.
fn add_mod(item: &mut Item, pool: &[ExplicitModDefinition], name: &str, rng: &mut SeededRng) {
    let only = std::slice::from_ref(find(pool, name));
    assert!(add_random_mod(item, only, rng), "{name:?} can be added");
}

/// The prefixes and suffixes of an item.
fn explicits(item: &Item) -> Vec<&ExplicitMod> {
    item.prefixes.iter().chain(item.suffixes.iter()).collect()
}

fn outcome(item: &Item) -> Vec<String> {
    explicits(item)
        .iter()
        .map(|explicit| explicit.name.clone())
        .collect()
}

/// The placeholder names of a line, in their order.
fn placeholder_names(line: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut rest = line;
    while let Some(start) = rest.find('{') {
        let end = start
            + rest[start..]
                .find('}')
                .unwrap_or_else(|| panic!("unclosed placeholder in {line:?}"));
        names.push(rest[start + 1..end].to_owned());
        rest = &rest[end + 1..];
    }
    names
}

// ---------------------------------------------------------------------------------------
// Helpers: the PO's rules
// ---------------------------------------------------------------------------------------

/// The index of the stat's entry in the game's order (`STAT_ORDER`).
fn rank_of(stat: &str) -> usize {
    STAT_ORDER
        .iter()
        .position(|entry| entry.contains(&stat))
        .unwrap_or_else(|| panic!("the stat {stat:?} is not shown by a mod of the export 3.29.3.3"))
}

/// The increase of the ring for a side, 0 for every other base ("X% increased ... Modifier
/// magnitudes").
fn increase_of(base_id: &str, affix_type: AffixType) -> u32 {
    MAGNITUDE_INCREASES
        .iter()
        .find(|(id, _, _)| *id == base_id)
        .map_or(0, |(_, prefixes, suffixes)| match affix_type {
            AffixType::Prefix => *prefixes,
            AffixType::Suffix => *suffixes,
        })
}

/// A value multiplied by (1 + X/100) and rounded down to a whole number.
fn scaled(value: i32, increase: u32, what: &str) -> i32 {
    assert!(
        increase == 0 || value >= 0,
        "{what}: rounding down a scaled negative value is not evidenced (ADR 0022)"
    );
    let value = i64::from(value) * i64::from(100 + increase) / 100;
    i32::try_from(value).expect("a scaled value fits")
}

/// One line of the normal view: the stats it names and the line as a mod of its own, so that
/// `Mod::display_text` (protected by the tests of AB#185) shows it as usual.
struct ExpectedLine {
    stats: BTreeSet<String>,
    line: Mod,
}

/// The explicit lines of the item in the normal view by the PO's rule, and how many lines were
/// merged:
/// - each line of each prefix and suffix, with its values as the normal view shows them
///   (scaled by the ring's increase for the mod's side and rounded down, AB#189);
/// - lines of different mods with the same stats are one line with their values added up;
/// - the lines sorted by the game's order of their stats.
///
/// Implicit mods are not part of it. Panics where the rules do not say what to show.
fn expected_explicit_texts(item: &Item) -> (Vec<String>, usize) {
    let mut lines: Vec<ExpectedLine> = Vec::new();
    let mut merged = 0;
    for explicit in explicits(item) {
        let what = format!("{} on the {}", explicit.name, item.base.name);
        let modifier = &explicit.modifier;
        let increase = increase_of(&item.base.id, explicit.affix_type);
        for text in modifier.text.split('\n') {
            let mut placeholders = BTreeMap::new();
            for name in placeholder_names(text) {
                let stat = modifier
                    .placeholders
                    .get(&name)
                    .unwrap_or_else(|| panic!("{what}: the placeholder {name:?} names no stat"));
                placeholders.insert(name, stat.clone());
            }
            let stats: BTreeSet<String> = placeholders.values().cloned().collect();
            assert!(
                !stats.is_empty(),
                "{what}: the line {text:?} names no stat, so the rule gives it no place"
            );
            let mut line = modifier.clone();
            line.text = text.to_owned();
            line.stats.retain(|stat| stats.contains(&stat.id));
            for stat in &mut line.stats {
                stat.value = scaled(stat.value, increase, &what);
            }
            line.placeholders = placeholders;
            line.conversions.retain(|stat, _| stats.contains(stat));
            match lines.iter_mut().find(|known| known.stats == stats) {
                Some(known) => {
                    merged += 1;
                    assert_eq!(
                        known.line.text, line.text,
                        "{what}: lines with the same stats have the same text"
                    );
                    assert!(
                        known.line.conversions.is_empty() && line.conversions.is_empty(),
                        "{what}: how converted values are added up is not in poe-crafting-rules"
                    );
                    for stat in &mut known.line.stats {
                        let other = line
                            .stats
                            .iter()
                            .find(|other| other.id == stat.id)
                            .expect("the same stats");
                        stat.value += other.value;
                    }
                }
                None => lines.push(ExpectedLine { stats, line }),
            }
        }
    }
    let mut ranked: Vec<(usize, String)> = lines
        .iter()
        .map(|expected| {
            let ranks: BTreeSet<usize> = expected.stats.iter().map(|s| rank_of(s)).collect();
            assert_eq!(
                ranks.len(),
                1,
                "the stats {:?} of one line come from one entry of the game",
                expected.stats
            );
            (
                ranks.into_iter().next().unwrap(),
                expected.line.display_text(),
            )
        })
        .collect();
    ranked.sort_by_key(|(rank, _)| *rank);
    for pair in ranked.windows(2) {
        assert!(
            pair[0].0 < pair[1].0,
            "{:?}: two lines of different stats share an entry, the rule gives them no order",
            outcome(item)
        );
    }
    (ranked.into_iter().map(|(_, text)| text).collect(), merged)
}

// ---------------------------------------------------------------------------------------
// Helpers: invented raw files in RePoE's format (AC1)
// ---------------------------------------------------------------------------------------

const TEST_RING: &str = "Metadata/Items/Test/TestRing";
const TEST_UNUSED_STAT: &str = "test_unused_+%";
const TEST_LIGHT_RADIUS_STAT: &str = "test_light_radius_+%";
const TEST_STRENGTH_STAT: &str = "test_strength";
const TEST_IMPLICIT_STAT: &str = "test_implicit_maximum_mana";
const TEST_MINIMUM_DAMAGE_STAT: &str = "test_attack_minimum_added_damage";
const TEST_MAXIMUM_DAMAGE_STAT: &str = "test_attack_maximum_added_damage";
const TEST_LIFE_STAT: &str = "test_maximum_life";
const TEST_ACCURACY_STAT: &str = "test_accuracy_rating_+%";

fn null_properties() -> Value {
    let keys = [
        "armour",
        "energy_shield",
        "evasion",
        "ward",
        "movement_speed",
        "block",
        "description",
        "directions",
        "stack_size",
        "stack_size_currency_tab",
        "full_stack_turns_into",
        "charges_max",
        "charges_per_use",
        "duration",
        "life_per_use",
        "mana_per_use",
        "attack_time",
        "critical_strike_chance",
        "physical_damage_max",
        "physical_damage_min",
        "range",
        "mana_burn_ms",
        "cooldown_ms",
        "monster_id",
        "monster_ability_text",
        "monster_category",
    ];
    Value::Object(
        keys.iter()
            .map(|key| ((*key).to_owned(), Value::Null))
            .collect(),
    )
}

fn raw_base(name: &str, implicits: &[&str]) -> Value {
    json!({
        "domain": "item",
        "drop_level": 30,
        "implicits": implicits,
        "inventory_height": 1,
        "inventory_width": 1,
        "inherits_from": "Metadata/Items/Rings/AbstractRing",
        "item_class": "TestRing",
        "name": name,
        "properties": null_properties(),
        "release_state": "released",
        "tags": ["testring", "default"],
        "visual_identity": { "dds_file": "Art/2DItems/Test.dds", "id": "Test" },
        "requirements": null,
        "grants_buff": null,
        "skills_granted": null
    })
}

fn raw_mod(
    name: &str,
    generation_type: &str,
    group: &str,
    level: u32,
    stats: Value,
    text: &str,
) -> Value {
    json!({
        "adds_tags": [],
        "domain": "item",
        "generation_type": generation_type,
        "generation_weights": [],
        "grants_effects": [],
        "groups": [group],
        "implicit_tags": [],
        "is_essence_only": false,
        "name": name,
        "required_level": level,
        "spawn_weights": [
            { "tag": "testring", "weight": 1000 },
            { "tag": "default", "weight": 0 }
        ],
        "stats": stats,
        "text": text,
        "type": group,
        "gold_value": null
    })
}

/// One English translation of an entry, with one condition, format and handler list per stat.
fn translation(stats: usize, format: &str, string: &str) -> Value {
    let conditions: Vec<Value> = (0..stats)
        .map(|_| json!({ "min": 1, "max": null, "negated": null }))
        .collect();
    let formats: Vec<&str> = (0..stats).map(|_| format).collect();
    let handlers: Vec<Value> = (0..stats).map(|_| json!([])).collect();
    json!({
        "condition": conditions,
        "format": formats,
        "index_handlers": handlers,
        "string": string,
        "reminder_text": null,
        "is_markup": null
    })
}

fn translation_entry(ids: &[&str], format: &str, string: &str) -> Value {
    json!({
        "English": [translation(ids.len(), format, string)],
        "ids": ids,
        "trade_stats": [],
        "hidden": null,
        "French": null,
        "German": null,
        "Japanese": null,
        "Korean": null,
        "Portuguese": null,
        "Russian": null,
        "Spanish": null,
        "Thai": null,
        "Traditional Chinese": null
    })
}

/// Small invented raw files in RePoE's format: a ring with an implicit and a pool of two prefixes
/// (life; added damage with two stats in one entry) and two suffixes (strength; a mod with two
/// lines from two entries, light radius and accuracy, listed in the mod in the other order). The
/// entries of `stat_translations.json` are in another order than the mods, and the first entry is
/// used by no mod.
fn invented_raw_files() -> RawFiles {
    let base_items = json!({
        TEST_RING: raw_base("Test Ring", &["TestImplicitMana1"]),
    });
    let mods = json!({
        "TestImplicitMana1": raw_mod("", "unique", "TestImplicitMana", 2,
            json!([{ "id": TEST_IMPLICIT_STAT, "max": 20, "min": 20 }]),
            "+20 to maximum Test Mana"),
        "TestLife1": raw_mod("Testy", "prefix", "TestLife", 1,
            json!([{ "id": TEST_LIFE_STAT, "max": 9, "min": 3 }]),
            "+(3-9) to maximum Test Life"),
        "TestAddedDamage1": raw_mod("Testing", "prefix", "TestAddedDamage", 1,
            json!([
                { "id": TEST_MINIMUM_DAMAGE_STAT, "max": 2, "min": 1 },
                { "id": TEST_MAXIMUM_DAMAGE_STAT, "max": 5, "min": 4 }
            ]),
            "Adds (1-2) to (4-5) Test Damage to Attacks"),
        "TestStrength1": raw_mod("of the Test", "suffix", "TestStrength", 1,
            json!([{ "id": TEST_STRENGTH_STAT, "max": 12, "min": 8 }]),
            "+(8-12) to Test Strength"),
        "TestShining1": raw_mod("of Testing", "suffix", "TestShining", 1,
            json!([
                { "id": TEST_LIGHT_RADIUS_STAT, "max": 5, "min": 5 },
                { "id": TEST_ACCURACY_STAT, "max": 11, "min": 9 }
            ]),
            "5% increased Test Light Radius\n(9-11)% increased Test Accuracy Rating"),
    });
    let mods_by_base = json!({
        "Test Rings": {
            "testring,default": {
                "bases": [TEST_RING],
                "mods": {
                    "prefix": {
                        "TestLife": { "TestLife1": 1000 },
                        "TestAddedDamage": { "TestAddedDamage1": 1000 }
                    },
                    "suffix": {
                        "TestStrength": { "TestStrength1": 1000 },
                        "TestShining": { "TestShining1": 1000 }
                    }
                },
                "conditional_mods": null
            }
        }
    });
    let stat_translations = json!([
        translation_entry(&[TEST_UNUSED_STAT], "#", "{0}% increased Test Unused"),
        translation_entry(
            &[TEST_LIGHT_RADIUS_STAT],
            "#",
            "{0}% increased Test Light Radius"
        ),
        translation_entry(&[TEST_STRENGTH_STAT], "+#", "{0} to Test Strength"),
        translation_entry(&[TEST_IMPLICIT_STAT], "+#", "{0} to maximum Test Mana"),
        translation_entry(
            &[TEST_MINIMUM_DAMAGE_STAT, TEST_MAXIMUM_DAMAGE_STAT],
            "#",
            "Adds {0} to {1} Test Damage to Attacks"
        ),
        translation_entry(&[TEST_LIFE_STAT], "+#", "{0} to maximum Test Life"),
        translation_entry(
            &[TEST_ACCURACY_STAT],
            "#",
            "{0}% increased Test Accuracy Rating"
        ),
    ]);
    let stat_value_handlers = json!({
        "tq_noop": { "type": "noop" },
    });
    let item_classes = json!({
        "TestRing": { "category": "Ring", "category_id": "Ring", "name": "Test Rings",
            "influence_tags": [] },
    });
    RawFiles {
        base_items: base_items.to_string(),
        mods: mods.to_string(),
        mods_by_base: mods_by_base.to_string(),
        stat_translations: stat_translations.to_string(),
        stat_value_handlers: stat_value_handlers.to_string(),
        item_classes: item_classes.to_string(),
    }
}

// ---------------------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------------------

// AC1: „Regel des PO, Wortlaut für poe-crafting-rules (verbindlich): In the normal view, the
// explicit mod lines of an item are sorted by the order of their stats in the game's stat
// descriptions (RePoE stat_translations). Lines of different mods with the same stat are shown as
// one line with their values added up; a fractured mod is not merged and keeps its own line.
// Implicit mods are shown above the explicit mods and are never merged with them. Evidence: normal
// view and Alt view of seven items read by the PO on 2026-10-31 (Sapphire Polished Composite Ring,
// Burning Stout Composite Ring, Cogwork Ring, rare Steel Ring, rare Sapphire Ring, rare Wyrmscale
// Boots, Sanguine Sapphire Ring of Haast): all 23 explicit lines follow the stat order. Wyrmscale
// Boots: "+92 to maximum Life" (Rotund) and "+18 to maximum Life" (Flea's) are shown as "+110 to
// maximum Life". Sanguine Sapphire Ring of Haast: the implicit "+26% to Cold Resistance" and the
// suffix "+48% to Cold Resistance" are shown as two lines. Fractured mods: the PO's knowledge of
// the game."
// (The wording is checked at the acceptance. Tech lead's plan: the converter writes the positions
// of the entries as `stat_order` (checked with invented raw files, and in `data/mods.json` against
// the order of the PO's export); `explicit_texts` of rare items of all ring bases filled with
// random mods follows the rule, with the order of the stats as a constant from the PO's export.
// The Wyrmscale Boots are no ring base; the merge is checked with the rarity mods, the only stat
// that mods of two families show in today's data (ADR 0023), also on Heist rings whose increase
// applies to one of the two mods. Fractured mods are not modelled and not testable.)
#[test]
fn ac1_explicit_lines_are_sorted_by_stat_order_and_lines_of_the_same_stat_are_merged() {
    // --- The converter writes the positions of the entries ---
    // Given invented raw files whose entries are in another order than the mods
    let raw = invented_raw_files();
    // When converting them
    let converted = convert(&raw, &[TEST_RING]).expect("the invented raw files convert");
    let mods_file: Value =
        serde_json::from_str(&converted.mods_json).expect("the converted mods.json is valid JSON");
    // Then `stat_order` maps every stat that a line of a mod shows to the 0-based position of its
    // entry, and holds no stat that no line of a mod shows
    let expected: BTreeMap<String, u64> = [
        (TEST_LIGHT_RADIUS_STAT, 1),
        (TEST_STRENGTH_STAT, 2),
        (TEST_MINIMUM_DAMAGE_STAT, 4),
        (TEST_MAXIMUM_DAMAGE_STAT, 4),
        (TEST_LIFE_STAT, 5),
        (TEST_ACCURACY_STAT, 6),
    ]
    .into_iter()
    .map(|(stat, position)| (stat.to_owned(), position))
    .collect();
    assert_eq!(
        stat_order_of(&mods_file, "the converted mods.json"),
        expected,
        "the converted stat_order"
    );

    // --- The data file has the order of the PO's export ---
    // Given data/mods.json
    let data = stat_order_of(&load_json("data/mods.json"), "data/mods.json");
    // Then it holds exactly the stats of the export that the mods show
    let all_stats: BTreeSet<String> = STAT_ORDER
        .iter()
        .flat_map(|entry| entry.iter().map(|stat| (*stat).to_owned()))
        .collect();
    assert_eq!(
        data.keys().cloned().collect::<BTreeSet<String>>(),
        all_stats,
        "the stats of stat_order in data/mods.json"
    );
    // And the stats of one entry share a position, and the entries follow the game's order
    let mut previous: Option<(u64, &[&str])> = None;
    for entry in &STAT_ORDER {
        let positions: BTreeSet<u64> = entry.iter().map(|stat| data[*stat]).collect();
        assert_eq!(positions.len(), 1, "{entry:?}: one position for one entry");
        let position = positions.into_iter().next().unwrap();
        if let Some((before, before_entry)) = previous {
            assert!(
                before < position,
                "{before_entry:?} ({before}) comes before {entry:?} ({position}) in the game"
            );
        }
        previous = Some((position, entry));
    }

    // --- The examples: a merged rarity line on rings with and without an increase ---
    // (ring, prefix and raw value, suffix and raw value, the merged line)
    let examples = [
        (IRON_RING_ID, ("Pirate's", 15), ("of Raiding", 13), 28),
        // The suffix is increased by 25 %: 13 × 1.25 = 16.25, shown as 16; 15 + 16 = 31.
        (COGWORK_RING_ID, ("Pirate's", 15), ("of Raiding", 13), 31),
        // The prefix is increased by 25 %: 11 × 1.25 = 13.75, shown as 13; 13 + 7 = 20.
        (GEODESIC_RING_ID, ("Magpie's", 11), ("of Plunder", 7), 20),
    ];
    let mut rng = SeededRng::new(SEED);
    for (ring_id, (prefix, prefix_value), (suffix, suffix_value), sum) in examples {
        let ring = base(ring_id);
        let pool = pool_of(ring_id);
        // Given a rare ring with a life prefix, a rarity prefix, a rarity suffix and a strength
        // suffix
        let mut item = item_of(&ring, Rarity::Rare, &mut rng);
        item.prefixes
            .push(explicit_with(&pool, "Stout", LIFE_STAT, 57, &mut rng));
        item.prefixes.push(explicit_with(
            &pool,
            prefix,
            RARITY_STAT,
            prefix_value,
            &mut rng,
        ));
        item.suffixes.push(explicit_with(
            &pool,
            suffix,
            RARITY_STAT,
            suffix_value,
            &mut rng,
        ));
        item.suffixes.push(explicit_with(
            &pool,
            "of the Brute",
            STRENGTH_STAT,
            10,
            &mut rng,
        ));
        let life = item.explicit_text(&item.prefixes[0]);
        let strength = item.explicit_text(&item.suffixes[1]);
        // When the item shows its explicit lines
        let shown = item.explicit_texts();
        // Then strength, life and rarity follow the stat order, and both rarity mods are one line
        // with the sum of the values the normal view shows
        assert_eq!(
            shown,
            vec![
                strength,
                life,
                format!("{sum}% increased Rarity of Items found"),
            ],
            "{}: {:?}",
            ring.name,
            outcome(&item)
        );
        assert_eq!(
            expected_explicit_texts(&item).0,
            shown,
            "{}: the test's rule",
            ring.name
        );
    }

    // --- Rare items of all ring bases ---
    let mut merged = 0;
    for ring in Slot::Ring.base_items() {
        let pool = pool_of(&ring.id);
        for trial in 0..FILLED_ITEMS {
            // Given a rare item of the ring filled with random mods
            let mut item = item_of(&ring, Rarity::Rare, &mut rng);
            while add_random_mod(&mut item, &pool, &mut rng) {}
            // When the item shows its explicit lines
            let shown = item.explicit_texts();
            // Then they are the lines of its prefixes and suffixes, merged and sorted by the rule
            let (expected, merged_lines) = expected_explicit_texts(&item);
            assert_eq!(
                shown,
                expected,
                "{} {}, item {trial}: {:?}",
                ring.name,
                ring.id,
                outcome(&item)
            );
            merged += merged_lines;
        }
    }
    // And some of the items had lines to merge, so the merge was checked on random items too
    assert!(merged > 0, "no random item had two lines of the same stat");
}

// AC2: „Der Simulator zeigt die Explicit-Mods eines Items nach Kriterium 1 sortiert. Beispiel:
// Ein magischer Ring mit einem Life-Prefix und einem Strength-Suffix zeigt die Strength-Zeile vor
// der Life-Zeile. (Ersetzt Kriterium 6 von AB#189.)"
// (Tech lead's plan: a magic Iron Ring with a life prefix and a strength suffix, added in both
// orders: the strength line comes before the life line. The page shows the lines of
// `explicit_texts`, a lean text check (ADR 0004); the page itself is checked in the local
// preview. AB#189 criterion 6 is replaced; its test checks the order of the lists, which stays.)
#[test]
fn ac2_a_magic_ring_shows_the_strength_suffix_before_the_life_prefix() {
    let mut rng = SeededRng::new(SEED);
    let iron = base(IRON_RING_ID);
    let pool = pool_of(IRON_RING_ID);

    // --- The example with fixed values ---
    // Given a magic Iron Ring with "Stout" (+57 to maximum Life) and "of the Brute" (+10 to
    // Strength)
    let mut item = item_of(&iron, Rarity::Magic, &mut rng);
    item.prefixes
        .push(explicit_with(&pool, "Stout", LIFE_STAT, 57, &mut rng));
    item.suffixes.push(explicit_with(
        &pool,
        "of the Brute",
        STRENGTH_STAT,
        10,
        &mut rng,
    ));
    // When the item shows its explicit lines
    // Then the strength line comes before the life line
    assert_eq!(
        item.explicit_texts(),
        vec!["+10 to Strength", "+57 to maximum Life"]
    );

    // --- Every life prefix and strength suffix, added in both orders ---
    let lives: Vec<&ExplicitModDefinition> = pool
        .iter()
        .filter(|definition| {
            definition.affix_type == AffixType::Prefix
                && definition.definition.stats.len() == 1
                && definition.definition.stats[0].id == LIFE_STAT
        })
        .collect();
    let strengths: Vec<&ExplicitModDefinition> = pool
        .iter()
        .filter(|definition| {
            definition.affix_type == AffixType::Suffix
                && definition.definition.stats.len() == 1
                && definition.definition.stats[0].id == STRENGTH_STAT
        })
        .collect();
    assert!(!lives.is_empty(), "the Iron Ring's pool has life prefixes");
    assert!(
        !strengths.is_empty(),
        "the Iron Ring's pool has strength suffixes"
    );
    for life in &lives {
        for strength in &strengths {
            for life_first in [true, false] {
                // Given a magic Iron Ring to which both mods are added, in one of both orders
                let mut item = item_of(&iron, Rarity::Magic, &mut rng);
                let order = if life_first {
                    [&life.name, &strength.name]
                } else {
                    [&strength.name, &life.name]
                };
                for name in order {
                    add_mod(&mut item, &pool, name, &mut rng);
                }
                // When the item shows its explicit lines
                // Then the strength line comes first, then the life line
                assert_eq!(
                    item.explicit_texts(),
                    vec![
                        item.explicit_text(&item.suffixes[0]),
                        item.explicit_text(&item.prefixes[0]),
                    ],
                    "{:?}, life added first: {life_first}",
                    outcome(&item)
                );
            }
        }
    }

    // --- The page ---
    // Then the page shows the explicit lines of crafting-core
    assert_web_src_uses(
        "explicit_texts",
        "the item on the page shows its explicit lines sorted and merged",
    );
}

// AC3: „Ein magischer Ring mit Rarity-Prefix und Rarity-Suffix zeigt eine einzige Zeile „X%
// increased Rarity of Items found“ mit der Summe beider Werte."
// (Tech lead's plan: a magic ring with a rarity prefix and a rarity suffix with fixed values:
// exactly one line with the sum. Here every rarity prefix with every rarity suffix of the Iron
// Ring's pool, added in both orders, first with fixed values, then with rolled values.)
#[test]
fn ac3_rarity_prefix_and_rarity_suffix_show_one_line_with_the_sum() {
    let mut rng = SeededRng::new(SEED);
    let iron = base(IRON_RING_ID);
    let pool = pool_of(IRON_RING_ID);

    // --- Fixed values ---
    // (prefix and raw value, suffix and raw value, the line)
    let examples = [
        (
            ("Magpie's", 10),
            ("of Plunder", 7),
            "17% increased Rarity of Items found",
        ),
        (
            ("Perandus'", 28),
            ("of Excavation", 26),
            "54% increased Rarity of Items found",
        ),
    ];
    for ((prefix, prefix_value), (suffix, suffix_value), line) in examples {
        for prefix_first in [true, false] {
            // Given a magic Iron Ring to which the rarity prefix and suffix are added
            let mut item = item_of(&iron, Rarity::Magic, &mut rng);
            let order = if prefix_first {
                [prefix, suffix]
            } else {
                [suffix, prefix]
            };
            for name in order {
                add_mod(&mut item, &pool, name, &mut rng);
            }
            set_value(
                &mut item.prefixes[0].modifier,
                RARITY_STAT,
                prefix_value,
                prefix,
            );
            set_value(
                &mut item.suffixes[0].modifier,
                RARITY_STAT,
                suffix_value,
                suffix,
            );
            // When the item shows its explicit lines
            // Then it shows one single line with the sum of both values
            assert_eq!(
                item.explicit_texts(),
                vec![line],
                "{:?}, prefix added first: {prefix_first}",
                outcome(&item)
            );
        }
    }

    // --- Every rarity prefix with every rarity suffix, rolled values ---
    for prefix in RARITY_PREFIXES {
        for suffix in RARITY_SUFFIXES {
            for prefix_first in [true, false] {
                for _ in 0..RARITY_ITEMS {
                    // Given a magic Iron Ring to which both mods are added
                    let mut item = item_of(&iron, Rarity::Magic, &mut rng);
                    let order = if prefix_first {
                        [prefix, suffix]
                    } else {
                        [suffix, prefix]
                    };
                    for name in order {
                        add_mod(&mut item, &pool, name, &mut rng);
                    }
                    let value = |explicit: &ExplicitMod| {
                        assert_eq!(explicit.modifier.stats.len(), 1, "{}", explicit.name);
                        assert_eq!(explicit.modifier.stats[0].id, RARITY_STAT);
                        explicit.modifier.stats[0].value
                    };
                    let sum = value(&item.prefixes[0]) + value(&item.suffixes[0]);
                    // When the item shows its explicit lines
                    // Then it shows one single line with the sum of both values
                    assert_eq!(
                        item.explicit_texts(),
                        vec![format!("{sum}% increased Rarity of Items found")],
                        "{:?}, prefix added first: {prefix_first}",
                        outcome(&item)
                    );
                }
            }
        }
    }
}

// AC4: „Implicits stehen weiterhin über den Explicit-Mods und werden nicht mit ihnen
// zusammengefasst."
// (Tech lead's plan: a magic Sapphire Ring with a Cold Resistance suffix: `explicit_texts` shows
// the suffix line with its own value and no implicit line; the implicit stays unchanged. The
// order on the page is checked by a unit test in `web` and in the local preview. Here also the
// PO's evidence "Sanguine Sapphire Ring of Haast" and, for every ring base, every mod of its pool
// that shows a stat of one of its implicits.)
#[test]
fn ac4_implicits_are_not_merged_with_explicit_mods() {
    let mut rng = SeededRng::new(SEED);

    // --- The PO's evidence: Sanguine Sapphire Ring of Haast ---
    let sapphire = base(SAPPHIRE_RING_ID);
    let pool = pool_of(SAPPHIRE_RING_ID);
    // Given a magic Sapphire Ring with the implicit "+26% to Cold Resistance", the prefix
    // "Sanguine" and the suffix "of Haast" with +48% to Cold Resistance
    let mut item = item_of(&sapphire, Rarity::Magic, &mut rng);
    assert_eq!(
        item.implicits.len(),
        1,
        "the Sapphire Ring has one implicit"
    );
    set_value(
        &mut item.implicits[0],
        COLD_RESISTANCE_STAT,
        26,
        "the Sapphire Ring's implicit",
    );
    item.prefixes
        .push(explicit_with(&pool, "Sanguine", LIFE_STAT, 30, &mut rng));
    item.suffixes.push(explicit_with(
        &pool,
        "of Haast",
        COLD_RESISTANCE_STAT,
        48,
        &mut rng,
    ));
    let implicits_before = item.implicits.clone();
    // When the item shows its explicit lines
    let shown = item.explicit_texts();
    // Then the suffix keeps its own value and no line of the implicit is among them
    assert_eq!(
        shown,
        vec!["+30 to maximum Life", "+48% to Cold Resistance"]
    );
    // And the implicit keeps its own line and value
    assert_eq!(
        item.implicits, implicits_before,
        "the implicit is unchanged"
    );
    assert_eq!(item.implicits[0].display_text(), "+26% to Cold Resistance");

    // --- Every ring base with a mod that shows a stat of an implicit ---
    let mut checked = 0;
    for ring in Slot::Ring.base_items() {
        let implicit_stats: BTreeSet<String> = ring
            .implicits
            .iter()
            .flat_map(|implicit| implicit.placeholders.values().cloned())
            .collect();
        for definition in pool_of(&ring.id) {
            if !definition
                .definition
                .placeholders
                .values()
                .any(|stat| implicit_stats.contains(stat))
            {
                continue;
            }
            // Given a rare item of the ring that holds only this mod
            let mut item = item_of(&ring, Rarity::Rare, &mut rng);
            let explicit = ExplicitMod {
                name: definition.name.clone(),
                affix_type: definition.affix_type,
                modifier: Mod::roll(&definition.definition, &mut rng),
            };
            match explicit.affix_type {
                AffixType::Prefix => item.prefixes.push(explicit),
                AffixType::Suffix => item.suffixes.push(explicit),
            }
            let implicits_before = item.implicits.clone();
            // When the item shows its explicit lines
            // Then they are the lines of the mod alone, with its own values
            assert_eq!(
                item.explicit_texts(),
                expected_explicit_texts(&item).0,
                "{} {}: {:?}",
                ring.name,
                ring.id,
                definition.name
            );
            // And the implicits are unchanged
            assert_eq!(item.implicits, implicits_before, "{}", ring.name);
            checked += 1;
        }
    }
    assert!(
        checked > 0,
        "some ring base has a mod that shows a stat of its implicit"
    );
}
