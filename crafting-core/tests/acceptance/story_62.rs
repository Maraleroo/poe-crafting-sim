//! Acceptance tests for AB#62 "Orb of Transmutation auf den Iron Ring anwenden".
//!
//! The tests use the public interface of `crafting-core` from the tech lead's plan on AB#62
//! and ADR 0008 to 0011: `AffixType`, `ExplicitModDefinition`, `ExplicitMod`, the fields
//! `prefix` and `suffix` of `Item`, `Item::name()`, `explicit_mods()`, `Currency`
//! (`id`, `name`), `CurrencyDefinition`, `currencies()`, `CraftError`,
//! `can_apply_orb_of_transmutation` and `apply_orb_of_transmutation`, together with the
//! interface of AB#52 (`base_item`, `Item::new`, `Mod`, `SeededRng`). Following ADR 0004,
//! web criteria are checked through `web/i18n/en.json` and the file contents under `web/`
//! (without the build output in `web/dist` and `web/target`).
//!
//! The PO's rule is `poe-crafting-rules`, `currencies/orb-of-transmutation.md`. Every
//! statistical test uses a fixed seed and compares observed shares with the shares that
//! follow from the mod weights, with a tolerance of 5 standard deviations. The tests depend
//! only on frequencies and invariants, never on the order in which the generator is used.
//!
//! Red state before the implementation: the module does not compile, because the new
//! interface does not exist yet. AC9 is a text check of the PO's rules, which were committed
//! to `poe-crafting-rules` before these tests (39f87c4); it passes as soon as the module
//! compiles.
//!
//! What is checked outside `cargo test`, by the PO in the local preview:
//! - AC1 to AC3: the rendering of the buttons and the magic item on the start page.
//! - AC6: different results after reloading the page, and the network and storage tabs of
//!   the browser's developer tools.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crafting_core::{
    apply_orb_of_transmutation, base_item, can_apply_orb_of_transmutation, currencies,
    explicit_mods, AffixType, BaseItem, CraftError, Currency, CurrencyDefinition, ExplicitMod,
    ExplicitModDefinition, Item, Mod, ModDefinition, Rarity, SeededRng, StatDefinition,
};
use serde_json::{Map, Value};

// ---------------------------------------------------------------------------------------
// Values from the story
// ---------------------------------------------------------------------------------------

/// The Iron Ring from AB#52 (source: poedb.tw).
const IRON_RING_ID: &str = "Metadata/Items/Rings/Ring1";
const IRON_RING_NAME: &str = "Iron Ring";

/// Name of the currency (AC1), a game text from the data (ADR 0011).
const ORB_NAME: &str = "Orb of Transmutation";
const ORB_ID: &str = "orb_of_transmutation";

/// Translation keys and texts (tech lead's plan, PO's answer for the error text).
const MAGIC_KEY: &str = "item.rarity.magic";
const MAGIC_TEXT: &str = "Magic";
const RESET_KEY: &str = "craft.reset";
const RESET_TEXT: &str = "Reset";

/// One row of the PO's mini mod pool on AB#62.
struct TableMod {
    affix_type: &'static str,
    name: &'static str,
    family: &'static str,
    level: u32,
    /// The display text, one entry per line.
    text_lines: &'static [&'static str],
    /// Placeholder, stat identifier, min, max.
    stats: &'static [(&'static str, &'static str, i32, i32)],
    tags: &'static [&'static str],
    weight: u32,
}

/// The PO's table, byte for byte from the description of AB#62. All stats are "Global".
const TABLE: [TableMod; 6] = [
    TableMod {
        affix_type: "prefix",
        name: "Hale",
        family: "IncreasedLife",
        level: 1,
        text_lines: &["+{value} to maximum Life"],
        stats: &[("value", "base maximum life", 3, 9)],
        tags: &["resource", "Life"],
        weight: 1000,
    },
    TableMod {
        affix_type: "prefix",
        name: "Beryl",
        family: "IncreasedMana",
        level: 1,
        text_lines: &["+{value} to maximum Mana"],
        stats: &[("value", "base maximum mana", 15, 19)],
        tags: &["resource", "Mana"],
        weight: 1000,
    },
    TableMod {
        affix_type: "prefix",
        name: "Humming",
        family: "LightningDamage",
        level: 3,
        text_lines: &["Adds {minimum} to {maximum} Lightning Damage to Attacks"],
        stats: &[
            ("minimum", "attack minimum added lightning damage", 1, 1),
            ("maximum", "attack maximum added lightning damage", 5, 5),
        ],
        tags: &[
            "elemental_damage",
            "Damage",
            "Elemental",
            "Lightning",
            "Attack",
        ],
        weight: 500,
    },
    TableMod {
        affix_type: "suffix",
        name: "of the Pupil",
        family: "Intelligence",
        level: 1,
        text_lines: &["+{value} to Intelligence"],
        stats: &[("value", "additional intelligence", 8, 12)],
        tags: &["Attribute"],
        weight: 1000,
    },
    TableMod {
        affix_type: "suffix",
        name: "of Shining",
        family: "LightRadiusAndAccuracy",
        level: 8,
        text_lines: &[
            "{accuracy}% increased Global Accuracy Rating",
            "{light}% increased Light Radius",
        ],
        stats: &[
            ("accuracy", "accuracy rating +%", 9, 11),
            ("light", "light radius +%", 5, 5),
        ],
        tags: &["Attack"],
        weight: 1000,
    },
    TableMod {
        affix_type: "suffix",
        name: "of the Lost",
        family: "ChaosResistance",
        level: 16,
        text_lines: &["+{value}% to Chaos Resistance"],
        stats: &[("value", "base chaos damage resistance %", 5, 10)],
        tags: &["Chaos", "Resistance"],
        weight: 250,
    },
];

/// Rounded shares quoted in AC7, used to cross-check the shares computed from the weights.
const AC7_ONE_MOD_SHARE: f64 = 0.5;
const AC7_SINGLE_PREFIX_SHARE: f64 = 0.526;
const AC7_HUMMING_SHARE_OF_PREFIXES: f64 = 0.20;
const AC7_LOST_SHARE_OF_SUFFIXES: f64 = 0.11;
/// Rounded share quoted in AC8 for the unbalanced pool.
const AC8_SINGLE_PREFIX_SHARE: f64 = 0.90;

/// The PO's rule for magic items in "Item basics" (AC9).
const MAGIC_ITEM_RULE: &str = "A magic item has at most one prefix and one suffix. Its name is \
    \"<prefix name> <base name> <suffix name>\"; a missing prefix or suffix leaves its part out.";

/// The PO's rule file for the Orb of Transmutation (AC9), line by line, with "≈" (U+2248)
/// and "⁻¹⁷" (U+207B U+00B9 U+2077) in the evidence.
const ORB_RULE_LINES: [&str; 8] = [
    "**Orb of Transmutation**",
    "- Applies to: normal items.",
    "- Effect: the item becomes magic and gets new mods.",
    "  1. The item gets 1 or 2 mods, each with a 50% chance.",
    "  2. One mod: it is chosen from one pool of all valid prefixes and all valid suffixes \
     together. The chance of a mod is its weight divided by the sum of the weights of all mods \
     in this pool. There is no separate choice between prefix and suffix.",
    "  3. Two mods: one prefix is chosen from the valid prefixes and one suffix from the valid \
     suffixes. Within each pool, the chance of a mod is its weight divided by the sum of the \
     weights in that pool. If one type has no valid mod, that mod is left out; no second mod \
     of the other type is chosen instead.",
    "  4. If there is no valid mod at all, applying the orb fails with an error message and \
     the item stays unchanged.",
    "- Evidence: measured by the PO on 2026-10-21 with 1000 Orbs of Transmutation on a Bone \
     Ring (item level 83): 1 mod 49.5 %, 2 mods 50.5 %; with one mod, 68.5 % suffixes against \
     69.4 % expected from the weights. A 50:50 choice between prefix and suffix is ruled out \
     (p \u{2248} 10\u{207b}\u{b9}\u{2077}). Not documented by GGG. The empty-pool case in rule \
     3 is the PO's assumption and not measured.",
];

const MODS_JSON: &str = "data/mods.json";
const CURRENCIES_JSON: &str = "data/currencies.json";
const EN_JSON: &str = "web/i18n/en.json";
const WEB_CARGO_TOML: &str = "web/Cargo.toml";
const CRAFTING_RULES: &str = ".claude/skills/poe-crafting-rules/SKILL.md";
const ORB_RULE_FILE: &str = ".claude/skills/poe-crafting-rules/currencies/orb-of-transmutation.md";
const ORB_RULE_LINK: &str = "currencies/orb-of-transmutation.md";

/// Fixed seeds, so every run is identical.
const SEED: u64 = 62;
const STATISTICS_SEED: u64 = 6207;
const TEST_POOL_SEED: u64 = 6208;

/// Items for invariant checks over many seeds.
const ITEMS: u64 = 2_000;
/// Trials for AC7: large enough that a 50:50 choice between prefix and suffix (50 % instead
/// of 52.6 %) lies far outside 5 standard deviations, and still done in a few seconds.
const AC7_TRIALS: u64 = 100_000;
/// Trials per test pool in AC8.
const AC8_TRIALS: u64 = 20_000;

/// Browser storage in any spelling of JavaScript or Rust (AC6), lower case.
const BROWSER_STORAGE: [&str; 7] = [
    "cookie",
    "localstorage",
    "sessionstorage",
    "indexeddb",
    "local_storage",
    "session_storage",
    "indexed_db",
];
/// Absolute and protocol-relative addresses (AC6), lower case.
const EXTERNAL_ADDRESSES: [&str; 5] = ["http://", "https://", "src=\"//", "href=\"//", "url(//"];
/// Time-based or other seed sources rejected by ADR 0010.
const REJECTED_SEED_SOURCES: [&str; 5] = [
    "Date::now",
    "date_now",
    "Math::random",
    "math_random",
    ".now()",
];

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

/// All files under `web/` except the translation file.
fn web_files_except_translations() -> Vec<PathBuf> {
    let en_json = repo_path(EN_JSON);
    web_files()
        .into_iter()
        .filter(|path| *path != en_json)
        .collect()
}

/// All files under `web/` outside the PO's policy texts in `web/legal/`.
fn web_files_outside_legal() -> Vec<PathBuf> {
    let legal = repo_path("web").join("legal");
    web_files()
        .into_iter()
        .filter(|path| !path.starts_with(&legal))
        .collect()
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

/// Files whose text contains `needle`; with `ignore_case`, both are compared in lower case.
fn files_containing(files: &[PathBuf], needle: &str, ignore_case: bool) -> Vec<String> {
    files
        .iter()
        .filter(|path| {
            let text = read_path(path);
            if ignore_case {
                text.to_lowercase().contains(&needle.to_lowercase())
            } else {
                text.contains(needle)
            }
        })
        .map(|path| path.display().to_string())
        .collect()
}

/// "<file> contains <needle>" for every needle found in one of the files.
fn findings(files: &[PathBuf], needles: &[&str], ignore_case: bool) -> Vec<String> {
    needles
        .iter()
        .flat_map(|needle| {
            files_containing(files, needle, ignore_case)
                .into_iter()
                .map(move |file| format!("{file} contains {needle:?}"))
        })
        .collect()
}

/// Asserts that some file under `web/src/` contains each of the needles.
fn assert_web_src_uses(needles: &[&str], why: &str) {
    let src = web_src_files();
    for needle in needles {
        assert!(
            !files_containing(&src, needle, false).is_empty(),
            "a file under web/src/ must use `{needle}`: {why}"
        );
    }
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
// Helpers: JSON data files (format from the tech lead's plan)
// ---------------------------------------------------------------------------------------

fn load_json(relative: &str) -> Value {
    let content = read_text(relative);
    serde_json::from_str(&content).unwrap_or_else(|e| panic!("{relative} is not valid JSON: {e}"))
}

fn json_field<'a>(object: &'a Value, key: &str, what: &str) -> &'a Value {
    object
        .get(key)
        .unwrap_or_else(|| panic!("{what} must have the field {key:?}, found {object}"))
}

fn json_array<'a>(object: &'a Value, key: &str, what: &str) -> &'a Vec<Value> {
    json_field(object, key, what)
        .as_array()
        .unwrap_or_else(|| panic!("{what}.{key} must be an array"))
}

fn json_str<'a>(object: &'a Value, key: &str, what: &str) -> &'a str {
    json_field(object, key, what)
        .as_str()
        .unwrap_or_else(|| panic!("{what}.{key} must be a string"))
}

fn json_i64(object: &Value, key: &str, what: &str) -> i64 {
    json_field(object, key, what)
        .as_i64()
        .unwrap_or_else(|| panic!("{what}.{key} must be an integer"))
}

fn json_strings(object: &Value, key: &str, what: &str) -> Vec<String> {
    json_array(object, key, what)
        .iter()
        .map(|value| {
            value
                .as_str()
                .unwrap_or_else(|| panic!("{what}.{key} must hold strings"))
                .to_owned()
        })
        .collect()
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

// ---------------------------------------------------------------------------------------
// Helpers: model
// ---------------------------------------------------------------------------------------

fn iron_ring() -> BaseItem {
    base_item(IRON_RING_ID)
        .unwrap_or_else(|| panic!("crafting-core must offer the base item {IRON_RING_ID:?}"))
}

fn affix_type(table_value: &str) -> AffixType {
    match table_value {
        "prefix" => AffixType::Prefix,
        "suffix" => AffixType::Suffix,
        other => panic!("unknown affix type {other:?} in the test table"),
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

/// The text the PO's rule expects for a rolled mod: every placeholder replaced by the rolled
/// value of the stat it names, looked up by identifier.
fn expected_display_text(rolled: &Mod) -> String {
    let mut text = rolled.text.clone();
    for (placeholder, stat_id) in &rolled.placeholders {
        text = text.replace(
            &format!("{{{placeholder}}}"),
            &stat_value(rolled, stat_id).to_string(),
        );
    }
    assert!(
        !text.contains('{'),
        "every placeholder of {:?} must name a stat",
        rolled.text
    );
    text
}

/// The name the PO's rule expects: "<prefix name> <base name> <suffix name>", missing parts
/// left out.
fn expected_name(item: &Item) -> String {
    [
        item.prefix.as_ref().map(|m| m.name.as_str()),
        Some(item.base.name.as_str()),
        item.suffix.as_ref().map(|m| m.name.as_str()),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" ")
}

fn find_definition<'a>(pool: &'a [ExplicitModDefinition], name: &str) -> &'a ExplicitModDefinition {
    pool.iter()
        .find(|definition| definition.name == name)
        .unwrap_or_else(|| panic!("the pool must contain the mod {name:?}"))
}

/// An explicit mod rolled from the named definition of the pool.
fn rolled_explicit(pool: &[ExplicitModDefinition], name: &str, rng: &mut SeededRng) -> ExplicitMod {
    let definition = find_definition(pool, name);
    ExplicitMod {
        name: definition.name.clone(),
        affix_type: definition.affix_type,
        modifier: Mod::roll(&definition.definition, rng),
    }
}

/// Checks that `rolled` sits in the slot of `slot_type` and was rolled from a definition of
/// `pool`: same name and type, same mod data, every stat value within its range.
fn assert_rolled_from_pool(
    pool: &[ExplicitModDefinition],
    rolled: &ExplicitMod,
    slot_type: AffixType,
) {
    assert_eq!(
        rolled.affix_type, slot_type,
        "{:?} sits in the {slot_type:?} slot",
        rolled.name
    );
    let definition = pool
        .iter()
        .find(|definition| definition.name == rolled.name && definition.affix_type == slot_type)
        .unwrap_or_else(|| panic!("{:?} ({slot_type:?}) must come from the pool", rolled.name));
    let expected = &definition.definition;
    let modifier = &rolled.modifier;
    assert_eq!(
        modifier.family, expected.family,
        "family of {:?}",
        rolled.name
    );
    assert_eq!(modifier.tags, expected.tags, "tags of {:?}", rolled.name);
    assert_eq!(modifier.text, expected.text, "text of {:?}", rolled.name);
    assert_eq!(
        modifier.placeholders, expected.placeholders,
        "placeholders of {:?}",
        rolled.name
    );
    assert_eq!(
        modifier.stats.len(),
        expected.stats.len(),
        "stats of {:?}",
        rolled.name
    );
    for stat in &expected.stats {
        let value = stat_value(modifier, &stat.id);
        assert!(
            (stat.min..=stat.max).contains(&value),
            "{:?}: {:?} rolled {value} outside {}..={}",
            rolled.name,
            stat.id,
            stat.min,
            stat.max
        );
    }
}

/// A test mod with one stat (AC8). The stat's range is real, so values are rolled.
fn test_mod(name: &str, affix_type: AffixType, weight: u32) -> ExplicitModDefinition {
    let stat_id = format!("{name} stat");
    ExplicitModDefinition {
        name: name.to_owned(),
        affix_type,
        level: 1,
        weight,
        definition: ModDefinition {
            family: format!("{name} family"),
            tags: vec!["test".to_owned()],
            stats: vec![StatDefinition {
                id: stat_id.clone(),
                min: 1,
                max: 10,
                global: true,
            }],
            text: format!("+{{value}} to {name}"),
            placeholders: BTreeMap::from([("value".to_owned(), stat_id)]),
        },
    }
}

fn weight_sum<'a>(definitions: impl IntoIterator<Item = &'a ExplicitModDefinition>) -> f64 {
    definitions
        .into_iter()
        .map(|definition| f64::from(definition.weight))
        .sum()
}

fn of_type(pool: &[ExplicitModDefinition], affix_type: AffixType) -> Vec<&ExplicitModDefinition> {
    pool.iter()
        .filter(|definition| definition.affix_type == affix_type)
        .collect()
}

// ---------------------------------------------------------------------------------------
// Helpers: statistics
// ---------------------------------------------------------------------------------------

/// Outcomes of many Orbs of Transmutation, each on a new normal item.
#[derive(Default)]
struct Tally {
    trials: u64,
    /// Items that got exactly one mod.
    one_mod: u64,
    /// Items that got a prefix and a suffix.
    two_mods: u64,
    /// Items with one mod: how often each mod was that mod.
    one_mod_by_name: BTreeMap<String, u64>,
    /// Items with one mod: how often it was a prefix.
    one_mod_prefixes: u64,
    /// Items with two mods: how often each prefix and each suffix was chosen.
    two_mod_prefixes: BTreeMap<String, u64>,
    two_mod_suffixes: BTreeMap<String, u64>,
}

impl Tally {
    fn count(map: &BTreeMap<String, u64>, name: &str) -> u64 {
        map.get(name).copied().unwrap_or(0)
    }
}

/// Applies the orb `trials` times, each time to a new Iron Ring, with one seeded generator,
/// and checks the invariants of the PO's rule on every result.
fn transmute_many(pool: &[ExplicitModDefinition], seed: u64, trials: u64) -> Tally {
    let base = iron_ring();
    let mut rng = SeededRng::new(seed);
    let mut tally = Tally {
        trials,
        ..Tally::default()
    };
    for trial in 0..trials {
        let mut item = Item::new(&base, &mut rng);
        let implicits_before = item.implicits.clone();
        let result = apply_orb_of_transmutation(&mut item, pool, &mut rng);
        assert_eq!(result, Ok(()), "trial {trial}: the orb must apply");
        assert_eq!(
            item.rarity,
            Rarity::Magic,
            "trial {trial}: the item is magic"
        );
        assert_eq!(
            item.base, base,
            "trial {trial}: the base item stays the same"
        );
        assert_eq!(
            item.implicits, implicits_before,
            "trial {trial}: the implicits stay unchanged"
        );
        if let Some(prefix) = &item.prefix {
            assert_rolled_from_pool(pool, prefix, AffixType::Prefix);
        }
        if let Some(suffix) = &item.suffix {
            assert_rolled_from_pool(pool, suffix, AffixType::Suffix);
        }
        match (&item.prefix, &item.suffix) {
            (Some(prefix), Some(suffix)) => {
                tally.two_mods += 1;
                *tally
                    .two_mod_prefixes
                    .entry(prefix.name.clone())
                    .or_default() += 1;
                *tally
                    .two_mod_suffixes
                    .entry(suffix.name.clone())
                    .or_default() += 1;
            }
            (Some(only), None) | (None, Some(only)) => {
                tally.one_mod += 1;
                if only.affix_type == AffixType::Prefix {
                    tally.one_mod_prefixes += 1;
                }
                *tally.one_mod_by_name.entry(only.name.clone()).or_default() += 1;
            }
            (None, None) => panic!("trial {trial}: a magic item must have at least one mod"),
        }
    }
    tally
}

/// Number of different items in `items`.
fn distinct_count(items: &[Item]) -> usize {
    let mut seen: Vec<&Item> = Vec::new();
    for item in items {
        if !seen.contains(&item) {
            seen.push(item);
        }
    }
    seen.len()
}

/// Asserts that `count` of `n` lies within 5 standard deviations of the share `p`.
fn assert_share(what: &str, count: u64, n: u64, p: f64) {
    assert!(n > 0, "{what}: no trials to compare");
    let (count_f, n_f) = (count as f64, n as f64);
    let observed = count_f / n_f;
    let tolerance = 5.0 * (p * (1.0 - p) / n_f).sqrt();
    assert!(
        (observed - p).abs() <= tolerance,
        "{what}: observed {observed:.4} ({count} of {n}), expected {p:.4} \u{b1} {tolerance:.4}"
    );
}

/// Asserts the shares of the PO's rule for a pool with prefixes and suffixes:
/// 1 and 2 mods each 50 %; one mod by weight from the whole pool; two mods by weight
/// within each type.
fn assert_rule_shares(pool: &[ExplicitModDefinition], tally: &Tally, label: &str) {
    let prefixes = of_type(pool, AffixType::Prefix);
    let suffixes = of_type(pool, AffixType::Suffix);
    let total = weight_sum(pool);
    let prefix_total = weight_sum(prefixes.iter().copied());
    let suffix_total = weight_sum(suffixes.iter().copied());

    assert_eq!(tally.one_mod + tally.two_mods, tally.trials);
    assert_share(
        &format!("{label}: items with one mod"),
        tally.one_mod,
        tally.trials,
        0.5,
    );
    assert_share(
        &format!("{label}: prefixes among items with one mod"),
        tally.one_mod_prefixes,
        tally.one_mod,
        prefix_total / total,
    );
    for definition in pool {
        assert_share(
            &format!("{label}: {:?} among items with one mod", definition.name),
            Tally::count(&tally.one_mod_by_name, &definition.name),
            tally.one_mod,
            f64::from(definition.weight) / total,
        );
    }
    for definition in &prefixes {
        assert_share(
            &format!(
                "{label}: {:?} among prefixes of items with two mods",
                definition.name
            ),
            Tally::count(&tally.two_mod_prefixes, &definition.name),
            tally.two_mods,
            f64::from(definition.weight) / prefix_total,
        );
    }
    for definition in &suffixes {
        assert_share(
            &format!(
                "{label}: {:?} among suffixes of items with two mods",
                definition.name
            ),
            Tally::count(&tally.two_mod_suffixes, &definition.name),
            tally.two_mods,
            f64::from(definition.weight) / suffix_total,
        );
    }
}

/// Asserts the outcome of the PO's rule 3 for a pool with only one type: every item has
/// exactly one mod of that type, chosen by weight, never a mod of the other type.
fn assert_one_sided_shares(pool: &[ExplicitModDefinition], tally: &Tally, label: &str) {
    let total = weight_sum(pool);
    assert_eq!(
        tally.two_mods, 0,
        "{label}: with one empty pool, no item may get two mods"
    );
    assert_eq!(
        tally.one_mod, tally.trials,
        "{label}: every item gets exactly one mod"
    );
    for definition in pool {
        assert_share(
            &format!("{label}: {:?}", definition.name),
            Tally::count(&tally.one_mod_by_name, &definition.name),
            tally.trials,
            f64::from(definition.weight) / total,
        );
    }
}

// ---------------------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------------------

// AC1: „Unter dem Iron Ring gibt es einen Button „Orb of Transmutation“. Ein Klick macht den
// Ring magisch: Seltenheit Magic, Implicit unverändert, darunter die neuen Mods als Text mit
// ihren gewürfelten Werten."
// (The rendering is checked by the PO in the local preview. Here: the result of the orb, the
// texts the page shows, the button label from the game data (ADR 0011) and that web uses it.)
#[test]
fn ac1_orb_of_transmutation_makes_the_iron_ring_magic_with_rolled_mod_texts() {
    // Given the Iron Ring, the mod pool from crafting-core and the English translations
    let base = iron_ring();
    let pool = explicit_mods();
    let translations = load_english_translations();
    let mut rng = SeededRng::new(SEED);

    for _ in 0..ITEMS {
        let mut item = Item::new(&base, &mut rng);
        let implicits_before = item.implicits.clone();
        let implicit_texts_before: Vec<String> =
            item.implicits.iter().map(Mod::display_text).collect();

        // When the Orb of Transmutation is applied
        let result = apply_orb_of_transmutation(&mut item, &pool, &mut rng);

        // Then the ring is magic
        assert_eq!(result, Ok(()));
        assert_eq!(item.rarity, Rarity::Magic);
        // And its implicit is unchanged
        assert_eq!(item.implicits, implicits_before);
        let implicit_texts: Vec<String> = item.implicits.iter().map(Mod::display_text).collect();
        assert_eq!(implicit_texts, implicit_texts_before);
        // And it has new mods from the pool, shown as text with their rolled values
        let new_mods: Vec<&ExplicitMod> = item.prefix.iter().chain(item.suffix.iter()).collect();
        assert!(!new_mods.is_empty(), "the magic ring must have new mods");
        for rolled in new_mods {
            assert_rolled_from_pool(&pool, rolled, rolled.affix_type);
            assert_eq!(
                rolled.modifier.display_text(),
                expected_display_text(&rolled.modifier)
            );
        }
    }
    // And the page shows the rarity as "Magic"
    assert_eq!(translation(&translations, MAGIC_KEY), MAGIC_TEXT);

    // And the button is labelled with the currency's name from the game data
    assert_eq!(Currency::OrbOfTransmutation.id(), ORB_ID);
    assert_eq!(Currency::OrbOfTransmutation.name(), ORB_NAME);
    let entries: Vec<CurrencyDefinition> = currencies();
    let entry = entries
        .iter()
        .find(|entry| entry.id == ORB_ID)
        .unwrap_or_else(|| panic!("currencies() must contain {ORB_ID:?}"));
    assert_eq!(entry.name, ORB_NAME);
    // And this name lives in data/currencies.json, which crafting-core embeds
    let data = load_json(CURRENCIES_JSON);
    let in_file = json_array(&data, "currencies", CURRENCIES_JSON)
        .iter()
        .find(|entry| entry.get("id").and_then(Value::as_str) == Some(ORB_ID))
        .unwrap_or_else(|| panic!("{CURRENCIES_JSON} must contain the currency {ORB_ID:?}"));
    assert_eq!(json_str(in_file, "name", CURRENCIES_JSON), ORB_NAME);
    let core_files = core_src_files();
    let embedding: Vec<&PathBuf> = core_files
        .iter()
        .filter(|path| {
            let text = read_path(path);
            text.contains("include_str!") && text.contains("currencies.json")
        })
        .collect();
    assert!(
        !embedding.is_empty(),
        "a file under crafting-core/src/ must embed {CURRENCIES_JSON} with `include_str!`"
    );
    // And the name is game data: it is not written into the code or the UI texts
    let name_in_code = [
        files_containing(&core_files, ORB_NAME, false),
        files_containing(&web_files(), ORB_NAME, false),
    ]
    .concat();
    assert!(
        name_in_code.is_empty(),
        "{ORB_NAME:?} must only be in {CURRENCIES_JSON}, also found in: {name_in_code:#?}"
    );
    // And the web page takes the label and the action from crafting-core
    assert_web_src_uses(
        &["OrbOfTransmutation", "apply_orb_of_transmutation"],
        "the orb button comes from crafting-core",
    );
}

// AC2: „Das magische Item heißt „<Prefix-Name> Iron Ring <Suffix-Name>“, z. B. „Hale Iron
// Ring of the Pupil“. Fehlt ein Prefix oder Suffix, entfällt sein Teil („Hale Iron Ring“,
// „Iron Ring of the Pupil“)."
#[test]
fn ac2_magic_item_name_is_prefix_base_name_suffix_with_missing_parts_left_out() {
    // Given the Iron Ring and the mods Hale and of the Pupil from the pool
    let base = iron_ring();
    let pool = explicit_mods();
    let mut rng = SeededRng::new(SEED);
    let normal = Item::new(&base, &mut rng);
    let mut both = normal.clone();
    both.rarity = Rarity::Magic;
    both.prefix = Some(rolled_explicit(&pool, "Hale", &mut rng));
    both.suffix = Some(rolled_explicit(&pool, "of the Pupil", &mut rng));
    let mut prefix_only = both.clone();
    prefix_only.suffix = None;
    let mut suffix_only = both.clone();
    suffix_only.prefix = None;

    // When asking for their names
    // Then the names follow the PO's examples
    assert_eq!(both.name(), "Hale Iron Ring of the Pupil");
    assert_eq!(prefix_only.name(), "Hale Iron Ring");
    assert_eq!(suffix_only.name(), "Iron Ring of the Pupil");
    // And the normal ring keeps its base name
    assert_eq!(normal.name(), IRON_RING_NAME);

    // And every ring made magic by the orb is named "<prefix> Iron Ring <suffix>"
    for _ in 0..ITEMS {
        let mut item = Item::new(&base, &mut rng);
        apply_orb_of_transmutation(&mut item, &pool, &mut rng).expect("the orb must apply");
        assert_eq!(item.name(), expected_name(&item));
    }
    // And the page shows the name from crafting-core
    assert_web_src_uses(&[".name()"], "the item name comes from crafting-core");
}

// AC3: „Ein Button „Reset“ stellt den normalen Iron Ring wieder her. Auf einem magischen Item
// ist „Orb of Transmutation“ deaktiviert; die Begründung dafür kommt mit Story 3."
#[test]
fn ac3_reset_restores_the_normal_ring_and_the_orb_is_disabled_on_a_magic_item() {
    // Given the Iron Ring, the mod pool and the English translations
    let base = iron_ring();
    let pool = explicit_mods();
    let translations = load_english_translations();
    let mut rng = SeededRng::new(SEED);
    let fresh = Item::new(&base, &mut rng);

    for _ in 0..ITEMS {
        // When the orb is applied to a normal ring
        let mut item = Item::new(&base, &mut rng);
        assert_eq!(
            can_apply_orb_of_transmutation(&item),
            Ok(()),
            "the orb is enabled on the normal ring"
        );
        apply_orb_of_transmutation(&mut item, &pool, &mut rng).expect("the orb must apply");

        // Then the orb cannot be used on the magic ring
        assert_eq!(
            can_apply_orb_of_transmutation(&item),
            Err(CraftError::ItemNotNormal)
        );
        // And applying it anyway fails and leaves the item unchanged
        let magic = item.clone();
        assert_eq!(
            apply_orb_of_transmutation(&mut item, &pool, &mut rng),
            Err(CraftError::ItemNotNormal)
        );
        assert_eq!(item, magic);

        // When the item is reset
        let reset = Item::new(&base, &mut rng);

        // Then it is the normal Iron Ring again, and the orb is enabled again
        assert_eq!(reset.rarity, Rarity::Normal);
        assert_eq!(reset.prefix, None);
        assert_eq!(reset.suffix, None);
        assert_eq!(reset.implicits, fresh.implicits);
        assert_eq!(reset.name(), IRON_RING_NAME);
        assert_eq!(can_apply_orb_of_transmutation(&reset), Ok(()));
    }

    // And the Reset button's label is a translation key
    assert_eq!(translation(&translations, RESET_KEY), RESET_TEXT);
    let hard_coded = findings(
        &web_files_except_translations(),
        &["\"Reset\"", ">Reset<"],
        false,
    );
    assert!(
        hard_coded.is_empty(),
        "{RESET_TEXT:?} must only be in {EN_JSON}: {hard_coded:#?}"
    );
    // And the page uses the key, asks crafting-core whether the orb can be used and
    // disables the button
    assert_web_src_uses(
        &[RESET_KEY, "can_apply_orb_of_transmutation", "disabled"],
        "Reset and the orb's state come from crafting-core and the translations",
    );
}

// AC4: „Die Mods aus der Tabelle liegen in einer Datendatei im Repo mit allen Werten aus der
// Tabelle, auch wenn noch keine Funktion sie nutzt (z. B. Tags, Family)."
#[test]
fn ac4_mod_pool_lives_in_a_data_file_with_all_table_values() {
    // Given the data file, the mods from crafting-core and the crafting-core sources
    let data = load_json(MODS_JSON);
    let pool = explicit_mods();
    let core_files = core_src_files();

    // When reading all mods of the data file
    let entries = json_array(&data, "mods", MODS_JSON);

    // Then the file holds exactly the six mods of the table
    let file_names: BTreeSet<&str> = entries
        .iter()
        .map(|entry| json_str(entry, "name", "every mod in mods.json"))
        .collect();
    let table_names: BTreeSet<&str> = TABLE.iter().map(|row| row.name).collect();
    assert_eq!(
        entries.len(),
        TABLE.len(),
        "{MODS_JSON} has one entry per table row"
    );
    assert_eq!(file_names, table_names);

    for row in &TABLE {
        // And each mod in the file has every value of its table row
        let what = format!("{MODS_JSON}: {:?}", row.name);
        let entry = entries
            .iter()
            .find(|entry| entry.get("name").and_then(Value::as_str) == Some(row.name))
            .unwrap_or_else(|| panic!("{MODS_JSON} must contain {:?}", row.name));
        assert_eq!(
            json_str(entry, "affix_type", &what),
            row.affix_type,
            "{what}"
        );
        assert_eq!(json_str(entry, "family", &what), row.family, "{what}");
        assert_eq!(
            json_i64(entry, "level", &what),
            i64::from(row.level),
            "{what}"
        );
        assert_eq!(
            json_i64(entry, "weight", &what),
            i64::from(row.weight),
            "{what}"
        );
        assert_eq!(
            json_strings(entry, "tags", &what),
            strings(row.tags),
            "{what}"
        );
        assert_eq!(
            json_str(entry, "text", &what),
            row.text_lines.join("\n"),
            "{what}"
        );
        let placeholders = json_field(entry, "placeholders", &what);
        let stats = json_array(entry, "stats", &what);
        assert_eq!(stats.len(), row.stats.len(), "{what}: number of stats");
        for (placeholder, stat_id, min, max) in row.stats {
            assert_eq!(
                json_str(placeholders, placeholder, &what),
                *stat_id,
                "{what}: placeholder {placeholder:?}"
            );
            let stat = stats
                .iter()
                .find(|stat| stat.get("id").and_then(Value::as_str) == Some(*stat_id))
                .unwrap_or_else(|| panic!("{what} must have the stat {stat_id:?}"));
            assert_eq!(
                json_i64(stat, "min", stat_id),
                i64::from(*min),
                "{what}: min"
            );
            assert_eq!(
                json_i64(stat, "max", stat_id),
                i64::from(*max),
                "{what}: max"
            );
            assert_eq!(
                json_field(stat, "global", stat_id).as_bool(),
                Some(true),
                "{what}: {stat_id:?} is global"
            );
        }
        assert_eq!(
            placeholders.as_object().map(Map::len),
            Some(row.stats.len()),
            "{what}: one placeholder per stat"
        );

        // And crafting-core offers the same values
        let definition = find_definition(&pool, row.name);
        assert_eq!(definition.affix_type, affix_type(row.affix_type), "{what}");
        assert_eq!(definition.level, row.level, "{what}");
        assert_eq!(definition.weight, row.weight, "{what}");
        let modifier = &definition.definition;
        assert_eq!(modifier.family, row.family, "{what}");
        assert_eq!(modifier.tags, strings(row.tags), "{what}");
        assert_eq!(modifier.text, row.text_lines.join("\n"), "{what}");
        let expected_placeholders: BTreeMap<String, String> = row
            .stats
            .iter()
            .map(|(placeholder, stat_id, _, _)| ((*placeholder).to_owned(), (*stat_id).to_owned()))
            .collect();
        assert_eq!(modifier.placeholders, expected_placeholders, "{what}");
        let expected_stats: BTreeMap<String, (i32, i32, bool)> = row
            .stats
            .iter()
            .map(|(_, stat_id, min, max)| ((*stat_id).to_owned(), (*min, *max, true)))
            .collect();
        let core_stats: BTreeMap<String, (i32, i32, bool)> = modifier
            .stats
            .iter()
            .map(|stat| (stat.id.clone(), (stat.min, stat.max, stat.global)))
            .collect();
        assert_eq!(modifier.stats.len(), row.stats.len(), "{what}");
        assert_eq!(core_stats, expected_stats, "{what}");
    }
    assert_eq!(
        pool.len(),
        TABLE.len(),
        "explicit_mods() offers the six mods"
    );

    // And crafting-core embeds the data file at build time
    let embedding: Vec<&PathBuf> = core_files
        .iter()
        .filter(|path| {
            let text = read_path(path);
            text.contains("include_str!") && text.contains("mods.json")
        })
        .collect();
    assert!(
        !embedding.is_empty(),
        "a file under crafting-core/src/ must embed {MODS_JSON} with `include_str!`"
    );
}

// AC5: „Die Transmutation ist in `crafting-core` umgesetzt, genau nach der Regel des PO
// (Kriterium 9); `web` zeigt nur an."
// (The shares of rules 1 to 3 are checked in AC7 and AC8, rule 4 in AC8.)
#[test]
fn ac5_transmutation_lives_in_crafting_core_and_web_only_displays() {
    // Given the mod pool from crafting-core
    let pool = explicit_mods();

    // When crafting-core applies the orb to many Iron Rings
    let tally = transmute_many(&pool, SEED, ITEMS);

    // Then every result follows the PO's rule: a magic item with unchanged implicits and
    // one or two mods from the pool, at most one prefix and one suffix (checked per item)
    assert_eq!(tally.one_mod + tally.two_mods, ITEMS);
    // And both outcomes of rule 1 occur
    assert!(tally.one_mod > 0, "some items must get one mod");
    assert!(tally.two_mods > 0, "some items must get two mods");

    // And the web page calls crafting-core for the orb, the pool and the name
    assert_web_src_uses(
        &[
            "crafting_core",
            "apply_orb_of_transmutation",
            "explicit_mods",
            "can_apply_orb_of_transmutation",
        ],
        "the transmutation happens in crafting-core",
    );
    // And no file under web/ holds the mod pool's own data
    let mut own_data: Vec<String> = Vec::new();
    for definition in &pool {
        let mut needles: Vec<&str> = vec![definition.name.as_str()];
        needles.extend(definition.definition.text.lines());
        needles.extend(
            definition
                .definition
                .stats
                .iter()
                .map(|stat| stat.id.as_str()),
        );
        own_data.extend(findings(&web_files(), &needles, false));
    }
    assert!(
        own_data.is_empty(),
        "web/ must only display the mods from crafting-core: {own_data:#?}"
    );
}

// AC6: „Jeder Klick kann ein anderes Ergebnis liefern, auch nach dem Neuladen der Seite.
// Tests nutzen einen festen Seed (ADR 0009). Woher die Seite ihren Seed nimmt, entscheidet
// der Tech Lead. Die Seite speichert dafür nichts im Browser und lädt nichts von fremden
// Servern."
// (Tech lead's decision, ADR 0010: one generator per page, seeded once from the Web Crypto
// API. Rolls after a reload and the browser's network and storage tabs are checked by the PO
// in the local preview.)
#[test]
fn ac6_each_click_and_each_page_load_can_give_a_different_result() {
    // Given the Iron Ring and the mod pool
    let base = iron_ring();
    let pool = explicit_mods();
    let transmute = |rng: &mut SeededRng| {
        let mut item = Item::new(&base, rng);
        apply_orb_of_transmutation(&mut item, &pool, rng).expect("the orb must apply");
        item
    };

    // When one generator serves many clicks (orb, reset, orb, ...) as on one page
    let mut page_rng = SeededRng::new(SEED);
    let clicks: Vec<Item> = (0..100).map(|_| transmute(&mut page_rng)).collect();
    // And many page loads each start with their own seed
    let first_clicks: Vec<Item> = (0..100)
        .map(|seed| transmute(&mut SeededRng::new(seed)))
        .collect();
    // And the same seed is used twice
    let repeated = transmute(&mut SeededRng::new(SEED));
    let repeated_again = transmute(&mut SeededRng::new(SEED));

    // Then clicks on one page give different results
    assert!(
        distinct_count(&clicks) > 1,
        "100 clicks always gave the same item"
    );
    // And different page loads give different results
    assert!(
        distinct_count(&first_clicks) > 1,
        "100 page loads always gave the same first item"
    );
    // And a fixed seed gives the same result every time, so tests are repeatable
    assert_eq!(repeated, repeated_again);

    // And the page takes its seed from the Web Crypto API, not from the time (ADR 0010)
    assert!(
        read_text(WEB_CARGO_TOML).contains("\"Crypto\""),
        "{WEB_CARGO_TOML} must enable the web-sys feature \"Crypto\""
    );
    assert_web_src_uses(
        &["get_random_values"],
        "the seed comes from the Web Crypto API",
    );
    let rejected = findings(&web_src_files(), &REJECTED_SEED_SOURCES, false);
    assert!(
        rejected.is_empty(),
        "the page must not take its seed from the time or Math.random: {rejected:#?}"
    );
    // And the page stores nothing in the browser and loads nothing from other servers
    let files = web_files_outside_legal();
    let storage = findings(&files, &BROWSER_STORAGE, true);
    assert!(
        storage.is_empty(),
        "outside web/legal/, no file under web/ may use cookies or browser storage: \
         {storage:#?}"
    );
    let external = findings(&files, &EXTERNAL_ADDRESSES, true);
    assert!(
        external.is_empty(),
        "outside web/legal/, no file under web/ may contain an absolute or protocol-relative \
         address: {external:#?}"
    );
}

// AC7: „Ein Test mit festem Seed und vielen Durchläufen (z. B. 10 000) zeigt, dass die
// Häufigkeiten innerhalb einer Toleranz der Regel entsprechen: 1 und 2 Mods je etwa 50 %; bei
// einem Mod Prefix etwa 52,6 % und Suffix etwa 47,4 % (Anteil an der Gewichtssumme 2500 zu
// 2250); bei zwei Mods innerhalb eines Pools Anteil nach Gewicht (z. B. Humming etwa 20 % der
// Prefixe, of the Lost etwa 11 % der Suffixe)."
// (100 000 trials instead of 10 000, so that a 50:50 choice between prefix and suffix
// cannot pass within 5 standard deviations.)
#[test]
fn ac7_frequencies_with_the_mini_pool_follow_the_rule() {
    // Given the mini mod pool from crafting-core
    let pool = explicit_mods();
    let prefixes = of_type(&pool, AffixType::Prefix);
    let suffixes = of_type(&pool, AffixType::Suffix);
    // And the expected shares from its weights match the rounded figures of the criterion
    let prefix_share = weight_sum(prefixes.iter().copied()) / weight_sum(&pool);
    let humming_share =
        f64::from(find_definition(&pool, "Humming").weight) / weight_sum(prefixes.iter().copied());
    let lost_share = f64::from(find_definition(&pool, "of the Lost").weight)
        / weight_sum(suffixes.iter().copied());
    assert!((prefix_share - AC7_SINGLE_PREFIX_SHARE).abs() < 0.001);
    assert!((humming_share - AC7_HUMMING_SHARE_OF_PREFIXES).abs() < 0.005);
    assert!((lost_share - AC7_LOST_SHARE_OF_SUFFIXES).abs() < 0.005);

    // When the orb is applied many times with a fixed seed
    let tally = transmute_many(&pool, STATISTICS_SEED, AC7_TRIALS);

    // Then 1 and 2 mods each occur about half the time
    assert_share(
        "items with one mod",
        tally.one_mod,
        tally.trials,
        AC7_ONE_MOD_SHARE,
    );
    // And with one mod, each mod and the prefixes as a whole occur by weight in the whole pool,
    // and with two mods, each mod occurs by weight within its own type
    assert_rule_shares(&pool, &tally, "mini pool");
}

// AC8: „Mit eigenen Testdaten abgedeckt: ein stark unausgeglichener Pool (z. B. Prefixe
// zusammen 9000, Suffixe 1000 → bei einem Mod etwa 90 % Prefixe, also kein Münzwurf), leerer
// Prefix-Pool und leerer Suffix-Pool (jeweils bei 1 und 2 Mods) und beide Pools leer (Fehler,
// Item bleibt unverändert)."
// (Whether an item gets 1 or 2 mods cannot be seen when one pool is empty, because the
// missing mod is left out. With 20 000 trials, both cases of rule 1 occur. The empty pools
// are covered with one and with two mods of the other type. The error message is the PO's
// text behind the key `craft.error.no_valid_mod`.)
#[test]
fn ac8_own_test_pools_cover_unbalanced_and_empty_pools() {
    let prefix_a = test_mod("Test Prefix A", AffixType::Prefix, 6000);
    let prefix_b = test_mod("Test Prefix B", AffixType::Prefix, 3000);
    let suffix_a = test_mod("of Test A", AffixType::Suffix, 600);
    let suffix_b = test_mod("of Test B", AffixType::Suffix, 400);

    // Given a strongly unbalanced pool: prefixes 9000, suffixes 1000
    let unbalanced = vec![
        prefix_a.clone(),
        prefix_b.clone(),
        suffix_a.clone(),
        suffix_b.clone(),
    ];
    let prefix_share =
        weight_sum(of_type(&unbalanced, AffixType::Prefix)) / weight_sum(&unbalanced);
    assert!((prefix_share - AC8_SINGLE_PREFIX_SHARE).abs() < 0.001);
    // When the orb is applied many times
    let tally = transmute_many(&unbalanced, TEST_POOL_SEED, AC8_TRIALS);
    // Then with one mod, about 90 % are prefixes, not a coin flip; and every share follows
    // the rule
    assert_share(
        "unbalanced pool: prefixes among items with one mod",
        tally.one_mod_prefixes,
        tally.one_mod,
        prefix_share,
    );
    assert_rule_shares(&unbalanced, &tally, "unbalanced pool");

    // Given an empty prefix pool, and an empty suffix pool, each with one and with two mods
    // of the other type
    let one_sided: [(&str, Vec<ExplicitModDefinition>, AffixType); 4] = [
        (
            "no prefixes, one suffix",
            vec![suffix_a.clone()],
            AffixType::Suffix,
        ),
        (
            "no prefixes, two suffixes",
            vec![suffix_a.clone(), suffix_b.clone()],
            AffixType::Suffix,
        ),
        (
            "no suffixes, one prefix",
            vec![prefix_a.clone()],
            AffixType::Prefix,
        ),
        (
            "no suffixes, two prefixes",
            vec![prefix_a.clone(), prefix_b.clone()],
            AffixType::Prefix,
        ),
    ];
    for (label, pool, present) in &one_sided {
        // When the orb is applied many times
        let base = iron_ring();
        let mut rng = SeededRng::new(TEST_POOL_SEED);
        for _ in 0..ITEMS {
            let mut item = Item::new(&base, &mut rng);
            apply_orb_of_transmutation(&mut item, pool, &mut rng).expect("the orb must apply");
            // Then the empty type's mod is left out, and no mod of the other type replaces it
            match present {
                AffixType::Prefix => {
                    assert!(item.prefix.is_some(), "{label}: the item gets a prefix");
                    assert_eq!(item.suffix, None, "{label}: the suffix is left out");
                }
                AffixType::Suffix => {
                    assert_eq!(item.prefix, None, "{label}: the prefix is left out");
                    assert!(item.suffix.is_some(), "{label}: the item gets a suffix");
                }
            }
        }
        // And the mod of the other type is chosen by weight
        let tally = transmute_many(pool, TEST_POOL_SEED, AC8_TRIALS);
        assert_one_sided_shares(pool, &tally, label);
    }

    // Given a pool without any mod
    let empty: Vec<ExplicitModDefinition> = Vec::new();
    let base = iron_ring();
    let mut rng = SeededRng::new(TEST_POOL_SEED);
    for _ in 0..ITEMS {
        let mut item = Item::new(&base, &mut rng);
        let before = item.clone();

        // When the orb is applied
        let result = apply_orb_of_transmutation(&mut item, &empty, &mut rng);

        // Then it fails with the reason "no valid mod", and the item is unchanged
        assert_eq!(result, Err(CraftError::NoValidMod));
        assert_eq!(item, before);
    }
}

// AC9: „Der Skill `poe-crafting-rules` enthält die Regeln des PO wörtlich: Im Abschnitt „Item
// basics“ wird ergänzt:
//     > A magic item has at most one prefix and one suffix. Its name is "<prefix name> <base
//     > name> <suffix name>"; a missing prefix or suffix leaves its part out.
// Und als eigene Datei `currencies/orb-of-transmutation.md`, verlinkt im Abschnitt
// „Currencies“: > **Orb of Transmutation** > - Applies to: normal items. > - Effect: the item
// becomes magic and gets new mods. > 1. The item gets 1 or 2 mods, each with a 50% chance.
// > 2. One mod: [...] > 3. Two mods: [...] > 4. If there is no valid mod at all, applying the
// orb fails with an error message and the item stays unchanged. > - Evidence: [...]"
// (The full quote is in `ORB_RULE_LINES`.)
#[test]
fn ac9_crafting_rules_contain_the_po_rules_verbatim() {
    // Given the skill poe-crafting-rules and its currency file
    let rules = read_text(CRAFTING_RULES);
    let orb_rules = read_text(ORB_RULE_FILE);

    // When reading the sections "Item basics" and "Currencies"
    let section = |heading: &str| -> String {
        let (_, after_heading) = rules
            .split_once(&format!("## {heading}"))
            .unwrap_or_else(|| panic!("{CRAFTING_RULES} must have a section `## {heading}`"));
        after_heading
            .split("\n## ")
            .next()
            .unwrap_or(after_heading)
            .to_owned()
    };
    let item_basics = section("Item basics");
    let currencies_section = section("Currencies");

    // Then "Item basics" contains the rule for magic items word for word
    assert!(
        normalize_whitespace(&item_basics).contains(&normalize_whitespace(MAGIC_ITEM_RULE)),
        "the section `## Item basics` of {CRAFTING_RULES} must contain {MAGIC_ITEM_RULE:?}"
    );
    // And the currency file contains every line of the Orb of Transmutation rule word for word
    let file_lines: Vec<&str> = orb_rules.lines().map(str::trim_end).collect();
    let missing: Vec<&str> = ORB_RULE_LINES
        .iter()
        .copied()
        .filter(|line| !file_lines.contains(line))
        .collect();
    assert!(
        missing.is_empty(),
        "{ORB_RULE_FILE} must contain these lines verbatim: {missing:#?}"
    );
    // And the lines are in the PO's order
    let positions: Vec<usize> = ORB_RULE_LINES
        .iter()
        .filter_map(|line| file_lines.iter().position(|file_line| file_line == line))
        .collect();
    assert!(
        positions.windows(2).all(|pair| pair[0] < pair[1]),
        "{ORB_RULE_FILE} must keep the PO's order of the lines"
    );
    // And the section "Currencies" links the file
    assert!(
        currencies_section.contains(&format!("({ORB_RULE_LINK})")),
        "the section `## Currencies` of {CRAFTING_RULES} must link {ORB_RULE_LINK}"
    );
}
