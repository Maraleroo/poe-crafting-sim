//! Acceptance tests for AB#52 "Iron Ring als festes Basis-Item anzeigen".
//!
//! The tests use the public interface of `crafting-core` from the tech lead's plan and
//! ADR 0008 / ADR 0009: `Rarity`, `StatDefinition`, `ModDefinition`, `BaseItem`, `Stat`,
//! `Mod` (`roll`, `display_text`), `Item::new`, `base_items()`, `base_item(id)`, the `Rng`
//! trait and `SeededRng::new(seed)`. Following ADR 0004, web criteria are checked through
//! the English translation file `web/i18n/en.json` and the file contents under `web/`
//! (without the build output in `web/dist` and `web/target`).
//!
//! Red state before the implementation: the module does not compile, because the
//! interface does not exist yet. AC8 is a text check of the PO's rule, which was added
//! to `poe-crafting-rules` before these tests (AB#55); it passes as soon as the module
//! compiles.
//!
//! What is checked outside `cargo test`:
//! - AC1: the rendering on the start page. The PO checks it in the local preview. Here:
//!   the item, its texts from `crafting-core` and the translation keys the page uses.
//! - AC5: the unit test with an invented value pair inside `crafting-core` is the
//!   developer's (AB#58). Here: the same behaviour through the public interface.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crafting_core::{
    base_item, base_items, BaseItem, Item, Mod, ModDefinition, Rarity, SeededRng, StatDefinition,
};
use serde_json::{Map, Value};

/// Values from the PO's table on AB#52 (source: poedb.tw, page "Iron Ring"). Adapted for
/// AB#167 (PO-confirmed): the data comes from RePoE, so the stat identifiers have
/// underscores and the Iron Ring has no own level requirement (0); the 2 of the table was
/// the drop level.
const IRON_RING_ID: &str = "Metadata/Items/Rings/Ring1";
const IRON_RING_NAME: &str = "Iron Ring";
const IRON_RING_CLASS: &str = "Rings";
const IRON_RING_REQUIRED_LEVEL: u32 = 0;
const IRON_RING_TAGS: [&str; 2] = ["default", "ring"];
const IMPLICIT_FAMILY: &str = "PhysicalDamage";
const IMPLICIT_LEVEL: u32 = 2;
const IMPLICIT_TAGS: [&str; 4] = ["physical_damage", "damage", "physical", "attack"];
const MINIMUM_STAT: &str = "attack_minimum_added_physical_damage";
const MAXIMUM_STAT: &str = "attack_maximum_added_physical_damage";
const IMPLICIT_TEMPLATE: &str = "Adds {minimum} to {maximum} Physical Damage to Attacks";
/// The implicit's text with every placeholder written as `<stat identifier>` (AB#167: the
/// placeholder names come from the converter, so only the stat they name is checked).
const IMPLICIT_TEXT_BY_STAT: &str = "Adds <attack_minimum_added_physical_damage> to \
    <attack_maximum_added_physical_damage> Physical Damage to Attacks";

/// Ranges of the two implicit stats from the PO's table (1–1 and 4–4).
const MINIMUM_RANGE: (i32, i32) = (1, 1);
const MAXIMUM_RANGE: (i32, i32) = (4, 4);

/// Texts the start page shows (AC1).
const IRON_RING_IMPLICIT_TEXT: &str = "Adds 1 to 4 Physical Damage to Attacks";

/// Translation keys and texts from the tech lead's plan (AC1, AC7).
const REQUIRES_LEVEL_KEY: &str = "item.requires_level";
const REQUIRES_LEVEL_TEMPLATE: &str = "Requires Level {level}";
const RARITY_KEYS: [(&str, &str); 4] = [
    ("item.rarity.normal", "Normal"),
    ("item.rarity.magic", "Magic"),
    ("item.rarity.rare", "Rare"),
    ("item.rarity.unique", "Unique"),
];

/// Visible texts that must not be written into the web code (AC7).
const REQUIRES_LEVEL_LABEL: &str = "Requires Level";
const MOD_TEXT_FRAGMENT: &str = "Physical Damage to Attacks";

/// The PO's rule from AC8, verbatim (with the en dash U+2013 in "min–max").
const ITEM_BASICS_RULE: &str = "Rarities: Normal, Magic, Rare, Unique. A normal item has no \
    prefixes or suffixes. It only has the implicit mods of its base, which can be none (for \
    example a Cobalt Jewel). A mod has one or more stats; each stat has a range (min\u{2013}max) \
    and rolls a value within that range when the mod is created. A mod's display text refers \
    to its stats by identifier, never by their order.";

const BASE_ITEMS_JSON: &str = "data/base_items.json";
const EN_JSON: &str = "web/i18n/en.json";
const CRAFTING_RULES: &str = ".claude/skills/poe-crafting-rules/SKILL.md";

/// Fixed seed, so every run is identical.
const SEED: u64 = 52;

/// Number of rolls for range checks; small enough to finish in well under a second.
const ROLLS: u64 = 1_000;

// ---------------------------------------------------------------------------------------
// Helpers: files
// ---------------------------------------------------------------------------------------

fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crafting-core must live inside the repository root")
}

fn repo_path(relative: &str) -> PathBuf {
    repo_root().join(relative)
}

fn read_path(path: &Path) -> String {
    let bytes = fs::read(path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    String::from_utf8_lossy(&bytes).into_owned()
}

fn read_text(relative: &str) -> String {
    let path = repo_path(relative);
    assert!(path.is_file(), "{relative} must exist");
    read_path(&path)
}

/// Collapses every run of whitespace, including line breaks, into one space.
fn normalize_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// All files below `dir`, skipping the given direct subdirectories (build output).
fn files_below(dir: &Path, skipped: &[&str]) -> Vec<PathBuf> {
    fn collect(root: &Path, dir: &Path, skipped: &[&str], out: &mut Vec<PathBuf>) {
        let entries =
            fs::read_dir(dir).unwrap_or_else(|e| panic!("cannot list {}: {e}", dir.display()));
        for entry in entries {
            let path = entry
                .unwrap_or_else(|e| panic!("cannot read entry in {}: {e}", dir.display()))
                .path();
            let relative = path.strip_prefix(root).expect("path lies under its root");
            if skipped.iter().any(|skip| relative == Path::new(skip)) {
                continue;
            }
            if path.is_dir() {
                collect(root, &path, skipped, out);
            } else {
                out.push(path);
            }
        }
    }
    let mut files = Vec::new();
    collect(dir, dir, skipped, &mut files);
    files
}

/// All files under `web/`, except build output in `web/dist` and `web/target`.
fn web_files() -> Vec<PathBuf> {
    let files = files_below(&repo_path("web"), &["dist", "target"]);
    assert!(
        files
            .iter()
            .any(|path| path.extension().is_some_and(|e| e == "rs")),
        "web/ must contain Rust source files; the scan found none"
    );
    files
}

/// All files under `web/src/`.
fn web_src_files() -> Vec<PathBuf> {
    let src = repo_path("web").join("src");
    let files: Vec<PathBuf> = web_files()
        .into_iter()
        .filter(|path| path.starts_with(&src))
        .collect();
    assert!(!files.is_empty(), "web/src/ must contain files");
    files
}

/// All files under `crafting-core/src/`.
fn core_src_files() -> Vec<PathBuf> {
    let files = files_below(&repo_path("crafting-core").join("src"), &[]);
    assert!(!files.is_empty(), "crafting-core/src/ must contain files");
    files
}

/// Files whose text contains `needle`.
fn files_containing(files: &[PathBuf], needle: &str) -> Vec<String> {
    files
        .iter()
        .filter(|path| read_path(path).contains(needle))
        .map(|path| path.display().to_string())
        .collect()
}

/// Loads `web/i18n/en.json` and checks that it is a flat JSON object of strings.
fn load_english_translations() -> Map<String, Value> {
    let content = read_text(EN_JSON);
    let value: Value = serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("{EN_JSON} is not valid JSON: {e}"));
    let Value::Object(map) = value else {
        panic!("{EN_JSON} must contain a JSON object at the top level");
    };
    for (key, value) in &map {
        assert!(
            value.is_string(),
            "{EN_JSON} must be a flat object of strings, but key {key:?} has value {value}"
        );
    }
    map
}

fn translation<'a>(translations: &'a Map<String, Value>, key: &str) -> &'a str {
    translations
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("{EN_JSON} has no text for key {key:?}"))
}

// ---------------------------------------------------------------------------------------
// Helpers: data file (format from the tech lead's plan)
// ---------------------------------------------------------------------------------------

fn load_base_items_file() -> Value {
    let content = read_text(BASE_ITEMS_JSON);
    serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("{BASE_ITEMS_JSON} is not valid JSON: {e}"))
}

fn json_array<'a>(value: &'a Value, what: &str) -> &'a Vec<Value> {
    value
        .as_array()
        .unwrap_or_else(|| panic!("{BASE_ITEMS_JSON}: {what} must be an array, found {value}"))
}

fn json_field<'a>(object: &'a Value, key: &str, what: &str) -> &'a Value {
    object
        .get(key)
        .unwrap_or_else(|| panic!("{BASE_ITEMS_JSON}: {what} must have the field {key:?}"))
}

fn json_str<'a>(object: &'a Value, key: &str, what: &str) -> &'a str {
    json_field(object, key, what)
        .as_str()
        .unwrap_or_else(|| panic!("{BASE_ITEMS_JSON}: {what}.{key} must be a string"))
}

fn json_i64(object: &Value, key: &str, what: &str) -> i64 {
    json_field(object, key, what)
        .as_i64()
        .unwrap_or_else(|| panic!("{BASE_ITEMS_JSON}: {what}.{key} must be an integer"))
}

fn json_string_set(object: &Value, key: &str, what: &str) -> BTreeSet<String> {
    json_array(json_field(object, key, what), &format!("{what}.{key}"))
        .iter()
        .map(|tag| {
            tag.as_str()
                .unwrap_or_else(|| panic!("{BASE_ITEMS_JSON}: {what}.{key} must hold strings"))
                .to_owned()
        })
        .collect()
}

fn string_set(values: &[&str]) -> BTreeSet<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

// ---------------------------------------------------------------------------------------
// Helpers: model
// ---------------------------------------------------------------------------------------

fn iron_ring() -> BaseItem {
    base_item(IRON_RING_ID)
        .unwrap_or_else(|| panic!("crafting-core must offer the base item {IRON_RING_ID:?}"))
}

fn stat_definition(id: &str, (min, max): (i32, i32)) -> StatDefinition {
    StatDefinition {
        id: id.to_owned(),
        min,
        max,
    }
}

/// A hand-built mod definition with the Iron Ring's text and placeholder map, but with
/// the given stats (any ranges, any order). Adapted for AB#167: no Global flag; the mod
/// has a level and no conversion.
fn physical_damage_definition(stats: Vec<StatDefinition>) -> ModDefinition {
    ModDefinition {
        family: IMPLICIT_FAMILY.to_owned(),
        level: IMPLICIT_LEVEL,
        tags: IMPLICIT_TAGS.iter().map(|tag| (*tag).to_owned()).collect(),
        stats,
        text: IMPLICIT_TEMPLATE.to_owned(),
        placeholders: BTreeMap::from([
            ("minimum".to_owned(), MINIMUM_STAT.to_owned()),
            ("maximum".to_owned(), MAXIMUM_STAT.to_owned()),
        ]),
        conversions: BTreeMap::new(),
    }
}

/// Rolled value of the stat with the given identifier.
fn stat_value(rolled: &Mod, id: &str) -> i32 {
    rolled
        .stats
        .iter()
        .find(|stat| stat.id == id)
        .unwrap_or_else(|| panic!("the mod must have a stat {id:?}, found {:?}", rolled.stats))
        .value
}

/// The definition of the stat with the given identifier.
fn find_stat_definition<'a>(definition: &'a ModDefinition, id: &str) -> &'a StatDefinition {
    definition
        .stats
        .iter()
        .find(|stat| stat.id == id)
        .unwrap_or_else(|| panic!("the mod definition must have a stat {id:?}"))
}

// ---------------------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------------------

// AC1: „Die Startseite zeigt den Iron Ring als normales Item: Name „Iron Ring“, Klasse
// „Rings“, „Requires Level 2“, Seltenheit Normal und das Implicit „Adds 1 to 4 Physical
// Damage to Attacks“."
// (PO decision: the rendering is checked in the local preview, on the condition that the
// data file is embedded at build time; that condition is checked in AC2.)
// (Adapted for AB#167, PO-confirmed: AC5 of AB#167 replaces "Requires Level 2". A normal
// Iron Ring shows no level requirement; "Requires Level 2" was the drop level.)
#[test]
fn ac1_iron_ring_is_a_normal_item_with_its_base_values_and_labels() {
    // Given the Iron Ring from crafting-core, a seeded RNG and the English translations
    let base = iron_ring();
    let mut rng = SeededRng::new(SEED);
    let translations = load_english_translations();

    // When the start page creates the item and looks up its labels
    let item = Item::new(&base, &mut rng);
    let rarity_label = translation(&translations, "item.rarity.normal");
    let implicit_texts: Vec<String> = item.implicits.iter().map(Mod::display_text).collect();

    // Then the item is a normal Iron Ring of class Rings
    assert_eq!(item.base.name, IRON_RING_NAME);
    assert_eq!(item.base.item_class, IRON_RING_CLASS);
    assert_eq!(item.rarity, Rarity::Normal);
    // And it has no level requirement to show, and the page shows the rarity "Normal"
    assert_eq!(item.required_level(), None);
    assert_eq!(rarity_label, "Normal");
    // And its only implicit reads "Adds 1 to 4 Physical Damage to Attacks"
    assert_eq!(implicit_texts, [IRON_RING_IMPLICIT_TEXT]);
}

// AC2: „Die Item-Daten liegen in einer Datendatei im Repo, nicht im Code verstreut. Sie
// enthalten alle Werte aus der Tabelle, Tags und „Global“ eingeschlossen, auch wenn noch
// keine Funktion sie nutzt."
// (Includes the PO's condition for AC1: the data file is embedded at build time and not
// loaded at runtime, ADR 0008.)
// (Adapted for AB#167, PO-confirmed: AC1 of AB#167 replaces the table values by RePoE's.
// `required_level` is the base's own requirement (0 for rings), the stat identifiers have
// underscores, the placeholder names come from the converter, and there is no Global flag.)
#[test]
fn ac2_item_data_lives_in_an_embedded_data_file_with_all_table_values() {
    // Given the data file, the crafting-core sources and the web files
    let data = load_base_items_file();
    let core_files = core_src_files();
    let web = web_files();

    // When looking up the Iron Ring in the data file
    let entries = json_array(json_field(&data, "base_items", "the file"), "base_items");
    let ring = entries
        .iter()
        .find(|entry| entry.get("id").and_then(Value::as_str) == Some(IRON_RING_ID))
        .unwrap_or_else(|| panic!("{BASE_ITEMS_JSON} must contain the base item {IRON_RING_ID}"));
    let implicits = json_array(json_field(ring, "implicits", "the Iron Ring"), "implicits");
    let implicit = implicits
        .first()
        .unwrap_or_else(|| panic!("{BASE_ITEMS_JSON}: the Iron Ring must have an implicit"));
    let stats = json_array(json_field(implicit, "stats", "the implicit"), "stats");
    let placeholders = json_field(implicit, "placeholders", "the implicit");

    // Then the data file holds every value of the PO's table for the base item
    assert_eq!(json_str(ring, "name", "the Iron Ring"), IRON_RING_NAME);
    assert_eq!(
        json_str(ring, "item_class", "the Iron Ring"),
        IRON_RING_CLASS
    );
    assert_eq!(
        json_i64(ring, "required_level", "the Iron Ring"),
        i64::from(IRON_RING_REQUIRED_LEVEL)
    );
    assert_eq!(
        json_string_set(ring, "tags", "the Iron Ring"),
        string_set(&IRON_RING_TAGS)
    );
    // And for its implicit: family, tags, display text and placeholder map
    assert_eq!(implicits.len(), 1, "the Iron Ring has exactly one implicit");
    assert_eq!(
        json_str(implicit, "family", "the implicit"),
        IMPLICIT_FAMILY
    );
    assert_eq!(
        json_string_set(implicit, "tags", "the implicit"),
        string_set(&IMPLICIT_TAGS)
    );
    let placeholders = placeholders
        .as_object()
        .unwrap_or_else(|| panic!("{BASE_ITEMS_JSON}: the placeholders must be an object"));
    assert_eq!(placeholders.len(), 2, "one placeholder per stat");
    let mut text_by_stat = json_str(implicit, "text", "the implicit").to_owned();
    for (name, stat) in placeholders {
        let stat = stat
            .as_str()
            .unwrap_or_else(|| panic!("{BASE_ITEMS_JSON}: placeholder {name:?} must name a stat"));
        text_by_stat = text_by_stat.replace(&format!("{{{name}}}"), &format!("<{stat}>"));
    }
    assert_eq!(text_by_stat, IMPLICIT_TEXT_BY_STAT);
    // And for both stats: identifier and range, without a "Global" flag
    assert_eq!(stats.len(), 2, "the implicit has exactly two stats");
    for (id, (min, max)) in [(MINIMUM_STAT, MINIMUM_RANGE), (MAXIMUM_STAT, MAXIMUM_RANGE)] {
        let stat = stats
            .iter()
            .find(|stat| stat.get("id").and_then(Value::as_str) == Some(id))
            .unwrap_or_else(|| panic!("{BASE_ITEMS_JSON}: the implicit must have a stat {id:?}"));
        assert_eq!(json_i64(stat, "min", id), i64::from(min), "min of {id:?}");
        assert_eq!(json_i64(stat, "max", id), i64::from(max), "max of {id:?}");
        assert!(
            stat.get("global").is_none(),
            "{id:?} has no Global flag in RePoE"
        );
    }
    // And crafting-core offers exactly the base items of the data file
    let file_ids: BTreeSet<String> = entries
        .iter()
        .map(|entry| json_str(entry, "id", "every base item").to_owned())
        .collect();
    let core_ids: BTreeSet<String> = base_items().into_iter().map(|base| base.id).collect();
    assert_eq!(core_ids, file_ids);
    // And the item data is not scattered in code
    let in_core = files_containing(&core_files, IRON_RING_NAME);
    let in_web = files_containing(&web, IRON_RING_NAME);
    assert!(
        in_core.is_empty() && in_web.is_empty(),
        "{IRON_RING_NAME:?} must only be in {BASE_ITEMS_JSON}, also found in: {:#?}",
        [in_core, in_web].concat()
    );
    // And crafting-core embeds the data file at build time
    let embedding: Vec<String> = core_files
        .iter()
        .filter(|path| {
            let text = read_path(path);
            text.contains("include_str!") && text.contains("base_items.json")
        })
        .map(|path| path.display().to_string())
        .collect();
    assert!(
        !embedding.is_empty(),
        "a file under crafting-core/src/ must embed {BASE_ITEMS_JSON} with `include_str!`"
    );
    // And nothing reads it at runtime: crafting-core has no file access, and the web part
    // neither refers to the data file nor copies the data directory
    let file_access = files_containing(&core_files, "std::fs");
    assert!(
        file_access.is_empty(),
        "crafting-core/src/ must not read files at runtime: {file_access:#?}"
    );
    let web_loading = [
        files_containing(&web, "base_items.json"),
        files_containing(&web, "../data"),
    ]
    .concat();
    assert!(
        web_loading.is_empty(),
        "web/ must take the item data from crafting-core, not load {BASE_ITEMS_JSON}: \
         {web_loading:#?}"
    );
}

// AC3: „Ein Mod besteht aus einem oder mehreren Stats. Jeder Stat hat eine Kennung (die
// Bezeichnung von poedb), eine Spanne (Min–Max) und einen gewürfelten Wert innerhalb
// dieser Spanne."
// (Adapted for AB#167, PO-confirmed: the test data has no Global flag, and the mod
// definitions have a level and no conversion.)
#[test]
fn ac3_every_stat_of_a_mod_has_identifier_range_and_rolled_value_in_range() {
    // Given a mod with one stat and a mod with two stats, both with real ranges
    let one_stat = ModDefinition {
        family: "TestFamily".to_owned(),
        level: 1,
        tags: Vec::new(),
        stats: vec![stat_definition("test single stat", (5, 9))],
        text: "+{value} to Test".to_owned(),
        placeholders: BTreeMap::from([("value".to_owned(), "test single stat".to_owned())]),
        conversions: BTreeMap::new(),
    };
    let two_stats = physical_damage_definition(vec![
        stat_definition(MINIMUM_STAT, (2, 7)),
        stat_definition(MAXIMUM_STAT, (10, 20)),
    ]);
    let mut rng = SeededRng::new(SEED);

    for definition in [&one_stat, &two_stats] {
        // When the mod is created many times
        let rolls: Vec<Mod> = (0..ROLLS)
            .map(|_| Mod::roll(definition, &mut rng))
            .collect();

        for rolled in &rolls {
            // Then it has exactly the stats of its definition
            assert_eq!(rolled.stats.len(), definition.stats.len());
            for stat in &rolled.stats {
                // And each stat keeps its identifier and range
                let defined = find_stat_definition(definition, &stat.id);
                assert_eq!((stat.min, stat.max), (defined.min, defined.max));
                // And its rolled value lies within the range
                assert!(
                    (stat.min..=stat.max).contains(&stat.value),
                    "{:?} rolled {} outside {}..={}",
                    stat.id,
                    stat.value,
                    stat.min,
                    stat.max
                );
            }
        }
        // And the values are rolled, not fixed: a real range gives more than one value
        for defined in &definition.stats {
            let values: BTreeSet<i32> = rolls
                .iter()
                .map(|rolled| stat_value(rolled, &defined.id))
                .collect();
            assert!(
                values.len() > 1,
                "{:?} with range {}..={} always rolled {values:?} in {ROLLS} rolls",
                defined.id,
                defined.min,
                defined.max
            );
        }
    }
}

// AC4: „Der Implicit des Iron Rings ist **ein** Mod mit **zwei** Stats. Wird das Item
// erzeugt, ergibt das Würfeln immer 1 und 4."
// (Adapted for AB#167, PO-confirmed: the stat identifiers are RePoE's, with underscores.)
#[test]
fn ac4_iron_ring_implicit_is_one_mod_with_two_stats_rolling_1_and_4() {
    // Given the Iron Ring from crafting-core
    let base = iron_ring();

    // Then its implicit is one mod with two stats with the ranges 1–1 and 4–4
    assert_eq!(
        base.implicits.len(),
        1,
        "the Iron Ring has one implicit mod"
    );
    let definition = &base.implicits[0];
    assert_eq!(definition.stats.len(), 2, "the implicit mod has two stats");
    let minimum = find_stat_definition(definition, MINIMUM_STAT);
    let maximum = find_stat_definition(definition, MAXIMUM_STAT);
    assert_eq!((minimum.min, minimum.max), MINIMUM_RANGE);
    assert_eq!((maximum.min, maximum.max), MAXIMUM_RANGE);

    for seed in 0..ROLLS {
        // When the item is created with any seed
        let mut rng = SeededRng::new(seed);
        let item = Item::new(&base, &mut rng);

        // Then it has one implicit mod with two stats that rolled 1 and 4
        assert_eq!(item.implicits.len(), 1, "seed {seed}: one implicit mod");
        let implicit = &item.implicits[0];
        assert_eq!(implicit.stats.len(), 2, "seed {seed}: two stats");
        assert_eq!(
            stat_value(implicit, MINIMUM_STAT),
            1,
            "seed {seed}: minimum"
        );
        assert_eq!(
            stat_value(implicit, MAXIMUM_STAT),
            4,
            "seed {seed}: maximum"
        );
    }
}

// AC5: „Der Mod-Text entsteht aus den gewürfelten Stat-Werten. Die Werte werden dabei über
// die Kennung des Stats zugeordnet, nicht über die Reihenfolge. Ein Unit-Test belegt das
// mit einem ausgedachten Wertepaar, z. B. „Adds 3 to 12 Physical Damage to Attacks“, und
// mit vertauschter Reihenfolge der Stats in den Daten."
// (The unit test inside crafting-core is the developer's, AB#58. This test checks the same
// behaviour through the public interface.)
// (Adapted for AB#167, PO-confirmed: the test data has no Global flag, and the mod
// definitions have a level and no conversion.)
#[test]
fn ac5_mod_text_uses_rolled_values_matched_by_stat_identifier_not_order() {
    // Given an invented value pair 3 and 12, once in the given and once in swapped order
    let in_order = physical_damage_definition(vec![
        stat_definition(MINIMUM_STAT, (3, 3)),
        stat_definition(MAXIMUM_STAT, (12, 12)),
    ]);
    let swapped = physical_damage_definition(vec![
        stat_definition(MAXIMUM_STAT, (12, 12)),
        stat_definition(MINIMUM_STAT, (3, 3)),
    ]);
    // And a definition in swapped order whose values really vary
    let varying = physical_damage_definition(vec![
        stat_definition(MAXIMUM_STAT, (10, 12)),
        stat_definition(MINIMUM_STAT, (3, 5)),
    ]);
    // And the Iron Ring with the stats of its implicit in swapped order
    let mut swapped_ring = iron_ring();
    swapped_ring.implicits[0].stats.reverse();
    let mut rng = SeededRng::new(SEED);

    // When the mods are created and their texts rendered
    let in_order_text = Mod::roll(&in_order, &mut rng).display_text();
    let swapped_text = Mod::roll(&swapped, &mut rng).display_text();
    let ring_texts: Vec<String> = Item::new(&swapped_ring, &mut rng)
        .implicits
        .iter()
        .map(Mod::display_text)
        .collect();

    // Then the order of the stats does not change the text
    assert_eq!(in_order_text, "Adds 3 to 12 Physical Damage to Attacks");
    assert_eq!(swapped_text, "Adds 3 to 12 Physical Damage to Attacks");
    assert_eq!(ring_texts, [IRON_RING_IMPLICIT_TEXT]);
    // And the text always shows the rolled values of the stats it names
    for _ in 0..ROLLS {
        let rolled = Mod::roll(&varying, &mut rng);
        let expected = format!(
            "Adds {} to {} Physical Damage to Attacks",
            stat_value(&rolled, MINIMUM_STAT),
            stat_value(&rolled, MAXIMUM_STAT)
        );
        assert_eq!(rolled.display_text(), expected);
    }
}

// AC6: „Basis-Item, Seltenheit, Mods und Stats sind in `crafting-core` modelliert; `web`
// zeigt sie nur an."
#[test]
fn ac6_crafting_core_models_the_item_and_web_only_displays_it() {
    // Given the Iron Ring from crafting-core and the web files
    let base = iron_ring();
    let mut rng = SeededRng::new(SEED);
    let src_files = web_src_files();
    let web = web_files();

    // When crafting-core creates the item
    let item = Item::new(&base, &mut rng);

    // Then crafting-core knows exactly the four rarities of the game (the match is
    // exhaustive, so another variant would not compile)
    let rarities: Vec<&str> = [Rarity::Normal, Rarity::Magic, Rarity::Rare, Rarity::Unique]
        .iter()
        .map(|rarity| match rarity {
            Rarity::Normal => "Normal",
            Rarity::Magic => "Magic",
            Rarity::Rare => "Rare",
            Rarity::Unique => "Unique",
        })
        .collect();
    assert_eq!(rarities, ["Normal", "Magic", "Rare", "Unique"]);
    // And the item is a normal item of its base item
    assert_eq!(item.base, base);
    assert_eq!(item.rarity, Rarity::Normal);
    // And its mods are rolled from the base item's implicit definitions
    assert_eq!(item.implicits.len(), base.implicits.len());
    for (rolled, definition) in item.implicits.iter().zip(&base.implicits) {
        assert_eq!(rolled.family, definition.family);
        assert_eq!(rolled.tags, definition.tags);
        assert_eq!(rolled.text, definition.text);
        assert_eq!(rolled.placeholders, definition.placeholders);
        assert_eq!(rolled.stats.len(), definition.stats.len());
    }
    // And the web code takes the item, the item creation and the mod text from crafting-core
    for needle in ["crafting_core", "base_item", "Item::new", "display_text"] {
        assert!(
            !files_containing(&src_files, needle).is_empty(),
            "a file under web/src/ must use `{needle}` from crafting-core"
        );
    }
    // And no file under web/ holds the mod's own data
    let own_data: Vec<String> = [MINIMUM_STAT, MAXIMUM_STAT, IMPLICIT_TEMPLATE]
        .iter()
        .flat_map(|needle| {
            files_containing(&web, needle)
                .into_iter()
                .map(move |file| format!("{file} contains {needle:?}"))
        })
        .collect();
    assert!(
        own_data.is_empty(),
        "web/ must only display the data from crafting-core: {own_data:#?}"
    );
}

// AC7: „Beschriftungen der Oberfläche wie „Requires Level“ und die Seltenheit kommen aus
// Übersetzungsschlüsseln. Spieldaten wie Item-Name und Mod-Text bleiben englische
// Originaltexte."
#[test]
fn ac7_labels_come_from_translation_keys_and_game_data_stays_english() {
    // Given the English translations, the web files and the Iron Ring from crafting-core
    let translations = load_english_translations();
    let en_json = repo_path("web").join("i18n").join("en.json");
    let src_files = web_src_files();
    let web = web_files();
    let mut rng = SeededRng::new(SEED);
    let item = Item::new(&iron_ring(), &mut rng);

    // When looking up the labels and searching the web files for visible texts
    let label_outside_translations: Vec<String> = files_containing(&web, REQUIRES_LEVEL_LABEL)
        .into_iter()
        .filter(|file| Path::new(file) != en_json)
        .collect();
    let game_text_in_web: Vec<String> = [IRON_RING_NAME, MOD_TEXT_FRAGMENT]
        .iter()
        .flat_map(|needle| {
            files_containing(&web, needle)
                .into_iter()
                .map(move |file| format!("{file} contains {needle:?}"))
        })
        .collect();

    // Then the labels are translation keys with English texts
    assert_eq!(
        translation(&translations, REQUIRES_LEVEL_KEY),
        REQUIRES_LEVEL_TEMPLATE
    );
    for (key, text) in RARITY_KEYS {
        assert_eq!(translation(&translations, key), text, "{EN_JSON}: {key}");
    }
    // And the web code uses these keys
    for key in [REQUIRES_LEVEL_KEY, "item.rarity."] {
        assert!(
            !files_containing(&src_files, key).is_empty(),
            "a file under web/src/ must use the translation key `{key}`"
        );
    }
    // And "Requires Level" is written nowhere else under web/
    assert!(
        label_outside_translations.is_empty(),
        "{REQUIRES_LEVEL_LABEL:?} must only be in {EN_JSON}: {label_outside_translations:#?}"
    );
    // And the game texts are the English originals from crafting-core, not from web/
    assert_eq!(item.base.name, IRON_RING_NAME);
    assert_eq!(item.implicits[0].display_text(), IRON_RING_IMPLICIT_TEXT);
    assert!(
        game_text_in_web.is_empty(),
        "game texts must come from crafting-core, not from web/: {game_text_in_web:#?}"
    );
}

// AC8: „Der Skill `poe-crafting-rules` enthält im Abschnitt „Item basics“ diese Regel des
// PO, wörtlich übernommen: > Rarities: Normal, Magic, Rare, Unique. A normal item has no
// prefixes or suffixes. It only has the implicit mods of its base, which can be none (for
// example a Cobalt Jewel). A mod has one or more stats; each stat has a range (min–max) and
// rolls a value within that range when the mod is created. A mod's display text refers to
// its stats by identifier, never by their order."
#[test]
fn ac8_crafting_rules_contain_the_item_basics_rule_verbatim() {
    // Given the skill poe-crafting-rules
    let rules = read_text(CRAFTING_RULES);

    // When reading its section "Item basics"
    let (_, after_heading) = rules
        .split_once("## Item basics")
        .unwrap_or_else(|| panic!("{CRAFTING_RULES} must have a section `## Item basics`"));
    let section = after_heading.split("\n## ").next().unwrap_or(after_heading);

    // Then the section contains the PO's rule word for word
    let section = normalize_whitespace(section);
    let rule = normalize_whitespace(ITEM_BASICS_RULE);
    assert!(
        section.contains(&rule),
        "the section `## Item basics` of {CRAFTING_RULES} must contain the rule {rule:?}"
    );
}
