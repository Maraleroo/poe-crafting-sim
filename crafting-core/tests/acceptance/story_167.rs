//! Acceptance tests for AB#167 "Iron Ring mit seinem vollständigen Mod-Pool aus RePoE".
//!
//! The tests use the public interface from the tech lead's plan on AB#167 and ADR 0016:
//! - `crafting-core`: `StatDefinition { id, min, max }` and `Stat` without the Global flag,
//!   `Conversion { divisor, precision }`, `ModDefinition` with `level` and `conversions`,
//!   `ExplicitModDefinition { name, affix_type, weight, definition }`, `BaseItem` with
//!   `required_level` as the base's own requirement, `Mod` with `level` and `conversions`,
//!   `Mod::display_text()` with conversion and `Item::required_level()`, together with the
//!   unchanged interface of AB#52, AB#62 and AB#114.
//! - `data-import` (dev-dependency of `crafting-core`): `RawFiles`, `Converted` and
//!   `convert(raw, base_ids)`.
//!
//! The expected game data (pool, weights, levels, stat identifiers, line order, conversions)
//! was taken by a program from the PO's RePoE export 3.29.3.3 (downloaded 2026-10-25); the
//! raw files are not in the repository. The display of converted values and the level
//! requirement follow the PO's rules in `poe-crafting-rules`, "Item basics". Every random test
//! uses a fixed seed.
//!
//! Red state before the implementation: the module does not compile, because `data-import`,
//! `Conversion`, the new fields and `Item::required_level` do not exist yet.
//!
//! Criteria without a permanent test: AC6 to AC9 only add the PO's rules to
//! `poe-crafting-rules` word for word. Their wording is checked at the acceptance; their
//! behaviour is checked here in AC4 (conversion) and AC5 (level requirement).
//!
//! What is checked outside `cargo test`:
//! - AC1: that the developer's second converter run on the raw files gives no diff, and that
//!   the binary writes nothing when the conversion fails (the binary needs the raw files).
//! - AC4 and AC5: the rendering on the page, by the PO in the local preview.
//!
//! Adapted for AB#248 (PO-confirmed): criterion 5 of AB#248 replaces the fields `prefix` and
//! `suffix` of `Item` by the lists `prefixes` and `suffixes` (ADR 0021). The tests read and set
//! the lists; what they check is unchanged. Criterion 4 of AB#248 replaces the check in AC3 that
//! the page builds the pool with `valid_mods`.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crafting_core::{
    apply_orb_of_alteration, apply_orb_of_transmutation, base_item, explicit_mods, mod_pool,
    valid_mods, AffixType, BaseItem, Conversion, ExplicitMod, ExplicitModDefinition, Item, Mod,
    ModDefinition, Rarity, SeededRng, StatDefinition,
};
use data_import::{convert, Converted, RawFiles};
use serde_json::{json, Value};

// ---------------------------------------------------------------------------------------
// Values from the story and the RePoE export 3.29.3.3
// ---------------------------------------------------------------------------------------

const IRON_RING_ID: &str = "Metadata/Items/Rings/Ring1";
const IRON_RING_NAME: &str = "Iron Ring";
const IRON_RING_CLASS: &str = "Rings";

/// The item level the UI uses by default ("Item level" in `poe-crafting-rules`).
const UI_ITEM_LEVEL: u32 = 100;

/// The Iron Ring's implicit in RePoE: one mod with level 2 and two stats.
const IMPLICIT_FAMILY: &str = "PhysicalDamage";
const IMPLICIT_LEVEL: u32 = 2;
const IMPLICIT_TAGS: [&str; 4] = ["physical_damage", "damage", "physical", "attack"];
const IMPLICIT_STATS: [(&str, i32, i32); 2] = [
    ("attack_minimum_added_physical_damage", 1, 1),
    ("attack_maximum_added_physical_damage", 4, 4),
];
const IMPLICIT_LINES: [&str; 1] = [
    "Adds {attack_minimum_added_physical_damage} to {attack_maximum_added_physical_damage} \
     Physical Damage to Attacks",
];

/// The Iron Ring's pool: categories `prefix` and `suffix` of its entry in
/// `mods_by_base.json` (ADR 0016).
const POOL_PREFIXES: usize = 81;
const POOL_SUFFIXES: usize = 126;
const PREFIX_WEIGHT_SUM: u64 = 60_250;
const SUFFIX_WEIGHT_SUM: u64 = 103_600;

/// The 35 mod families of the pool: affix type, number of mods, sum of their weights.
const FAMILIES: [(&str, &str, usize, u64); 35] = [
    ("IncreasedLife", "prefix", 8, 8000),
    ("IncreasedMana", "prefix", 13, 13000),
    ("IncreasedEnergyShield", "prefix", 11, 11000),
    ("PhysicalDamage", "prefix", 6, 6000),
    ("FireDamage", "prefix", 9, 3250),
    ("ColdDamage", "prefix", 9, 3250),
    ("LightningDamage", "prefix", 9, 3250),
    ("IncreasedEvasionRating", "prefix", 7, 7000),
    ("IncreasedWeaponElementalDamagePercent", "prefix", 5, 1500),
    ("ItemFoundRarityIncreasePrefix", "prefix", 4, 4000),
    ("Strength", "suffix", 9, 9000),
    ("Dexterity", "suffix", 9, 9000),
    ("Intelligence", "suffix", 9, 9000),
    ("AllAttributes", "suffix", 4, 1600),
    ("LifeLeech", "suffix", 1, 1000),
    ("ManaLeech", "suffix", 1, 1000),
    ("ItemFoundRarityIncrease", "suffix", 4, 4000),
    ("IncreasedCastSpeed", "suffix", 3, 2400),
    ("IncreasedAttackSpeed", "suffix", 1, 500),
    ("IncreasedAccuracy", "suffix", 5, 4600),
    ("LifeRegeneration", "suffix", 7, 7000),
    ("ManaRegeneration", "suffix", 6, 6000),
    ("FireResistance", "suffix", 8, 8000),
    ("ColdResistance", "suffix", 8, 8000),
    ("LightningResistance", "suffix", 8, 8000),
    ("ChaosResistance", "suffix", 6, 1500),
    ("AllResistances", "suffix", 5, 5000),
    ("LifeGainPerTarget", "suffix", 1, 1000),
    ("FireDamagePercentage", "suffix", 4, 2000),
    ("ColdDamagePercentage", "suffix", 4, 2000),
    ("LightningDamagePercentage", "suffix", 4, 2000),
    ("LifeGainedFromEnemyDeath", "suffix", 6, 3000),
    ("ManaGainedFromEnemyDeath", "suffix", 6, 3000),
    ("LightRadiusAndAccuracy", "suffix", 3, 3000),
    ("DamageTakenGainedAsLife", "suffix", 4, 2000),
];

/// A mod as expected in a converted data file.
struct ExpectedMod {
    id: &'static str,
    name: &'static str,
    affix_type: &'static str,
    family: &'static str,
    level: u32,
    weight: u32,
    tags: &'static [&'static str],
    /// Stat identifier, min, max (raw values).
    stats: &'static [(&'static str, i32, i32)],
    /// The display text, one entry per line, in the game's order. `{<stat id>}` stands for
    /// the placeholder that names this stat; the placeholder's own name does not matter.
    lines: &'static [&'static str],
    /// Stat identifier, divisor, precision.
    conversions: &'static [(&'static str, u32, Option<u32>)],
}

/// Samples of the Iron Ring's pool, with every value from RePoE.
const POOL_SAMPLES: [ExpectedMod; 8] = [
    ExpectedMod {
        id: "IncreasedLife0",
        name: "Hale",
        affix_type: "prefix",
        family: "IncreasedLife",
        level: 1,
        weight: 1000,
        tags: &["resource", "life"],
        stats: &[("base_maximum_life", 3, 9)],
        lines: &["+{base_maximum_life} to maximum Life"],
        conversions: &[],
    },
    ExpectedMod {
        id: "AddedLightningDamage1",
        name: "Humming",
        affix_type: "prefix",
        family: "LightningDamage",
        level: 3,
        weight: 500,
        tags: &[
            "elemental_damage",
            "damage",
            "elemental",
            "lightning",
            "attack",
        ],
        stats: &[
            ("attack_minimum_added_lightning_damage", 1, 1),
            ("attack_maximum_added_lightning_damage", 5, 5),
        ],
        lines: &["Adds {attack_minimum_added_lightning_damage} to \
                  {attack_maximum_added_lightning_damage} Lightning Damage to Attacks"],
        conversions: &[],
    },
    ExpectedMod {
        id: "LightRadiusAndAccuracyNew1",
        name: "of Shining",
        affix_type: "suffix",
        family: "LightRadiusAndAccuracy",
        level: 8,
        weight: 1000,
        tags: &["attack"],
        stats: &[("light_radius_+%", 5, 5), ("accuracy_rating_+%", 9, 11)],
        lines: &[
            "{accuracy_rating_+%}% increased Global Accuracy Rating",
            "{light_radius_+%}% increased Light Radius",
        ],
        conversions: &[],
    },
    ExpectedMod {
        id: "LifeRegeneration1",
        name: "of the Newt",
        affix_type: "suffix",
        family: "LifeRegeneration",
        level: 1,
        weight: 1000,
        tags: &["flat_life_regen", "resource", "life"],
        stats: &[("base_life_regeneration_rate_per_minute", 60, 120)],
        lines: &["Regenerate {base_life_regeneration_rate_per_minute} Life per second"],
        conversions: &[("base_life_regeneration_rate_per_minute", 60, Some(1))],
    },
    ExpectedMod {
        id: "LifeLeechPermyriadSuffix1",
        name: "of the Remora",
        affix_type: "suffix",
        family: "LifeLeech",
        level: 50,
        weight: 1000,
        tags: &["resource", "life", "physical", "attack"],
        stats: &[("life_leech_from_physical_attack_damage_permyriad", 20, 40)],
        lines: &[
            "{life_leech_from_physical_attack_damage_permyriad}% of Physical Attack Damage \
                  Leeched as Life",
        ],
        conversions: &[(
            "life_leech_from_physical_attack_damage_permyriad",
            100,
            None,
        )],
    },
    ExpectedMod {
        id: "ChaosResist1",
        name: "of the Lost",
        affix_type: "suffix",
        family: "ChaosResistance",
        level: 16,
        weight: 250,
        tags: &["chaos", "resistance"],
        stats: &[("base_chaos_damage_resistance_%", 5, 10)],
        lines: &["+{base_chaos_damage_resistance_%}% to Chaos Resistance"],
        conversions: &[],
    },
    ExpectedMod {
        id: "Strength5",
        name: "of the Gorilla",
        affix_type: "suffix",
        family: "Strength",
        level: 44,
        weight: 1000,
        tags: &["attribute"],
        stats: &[("additional_strength", 28, 32)],
        lines: &["+{additional_strength} to Strength"],
        conversions: &[],
    },
    ExpectedMod {
        id: "IncreasedEnergyShield1",
        name: "Shining",
        affix_type: "prefix",
        family: "IncreasedEnergyShield",
        level: 3,
        weight: 1000,
        tags: &["defences", "energy_shield"],
        stats: &[("base_maximum_energy_shield", 1, 3)],
        lines: &["+{base_maximum_energy_shield} to maximum Energy Shield"],
        conversions: &[],
    },
];

/// Every stat of the pool that the game converts for display, with its conversion.
const POOL_CONVERSIONS: [(&str, u32, Option<u32>); 3] = [
    ("base_life_regeneration_rate_per_minute", 60, Some(1)),
    (
        "life_leech_from_physical_attack_damage_permyriad",
        100,
        None,
    ),
    (
        "mana_leech_from_physical_attack_damage_permyriad",
        100,
        None,
    ),
];
/// Number of pool mods with a conversion: 7 life regeneration and 2 leech mods.
const POOL_MODS_WITH_CONVERSION: usize = 9;

/// The pool's mods with two lines; the game shows the accuracy line first (ADR 0015).
const TWO_LINE_MODS: [&str; 3] = ["of Shining", "of Light", "of Radiance"];
const TWO_LINE_ORDER: [&str; 2] = ["accuracy_rating_+%", "light_radius_+%"];

/// Provenance of the data (AC1).
const REPOE_SOURCE: &str = "repoe-fork.github.io";
const REPOE_VERSION: &str = "3.29.3.3";
const DOWNLOAD_DATE: &str = "2026-10-25";
const CONVERTER: &str = "data-import";
/// The six raw files the PO supplied and their SHA-256.
const RAW_FILES: [(&str, &str); 6] = [
    (
        "base_items.json",
        "48afb07a5f40eb51552f90c17d53186469db282a2e759dab7b46e7f9d2a0f58b",
    ),
    (
        "mods.json",
        "defbba31b60f9191080c78197278057f834f7f927c1a2a080a7ca6ed3ea686b5",
    ),
    (
        "mods_by_base.json",
        "55e7eb45ae56654c1eea4d115c07e8d723ab6972aabc47ca72eb06ed892dff39",
    ),
    (
        "stat_translations.json",
        "3e62106782eee9ee6dd68f0477ae5375222009700208b9a0b13435a282d3befa",
    ),
    (
        "stat_value_handlers.json",
        "d0426db079bb4d1256476deadd2305073fdaf81be24d305a4f013627d451c46e",
    ),
    (
        "item_classes.json",
        "678fa3ba653c2750c38ddba4eed2c0f7d6c5ec040b2bbc07bc502fed69c5e632",
    ),
];

/// Examples from the PO's rules (AC8, AC9): raw value and shown value.
const PER_SECOND_EXAMPLES: [(i32, &str); 7] = [
    (60, "1"),
    (120, "2"),
    (123, "2"),
    (129, "2.2"),
    (135, "2.2"),
    (195, "3.2"),
    (3840, "64"),
];
const LEECH_EXAMPLES: [(i32, &str); 5] = [
    (20, "0.2"),
    (21, "0.21"),
    (30, "0.3"),
    (40, "0.4"),
    (100, "1"),
];

/// Examples from the PO's rule for the level requirement (AC6): base requirement, levels of
/// all mods, shown requirement.
const LEVEL_EXAMPLES: [(&str, u32, &[u32], Option<u32>); 4] = [
    ("normal Bone Ring", 0, &[32], Some(25)),
    (
        "Blasting Blade with a mod of level 74",
        50,
        &[74, 35],
        Some(59),
    ),
    (
        "Blasting Blade with mods up to level 35",
        50,
        &[35, 20],
        Some(50),
    ),
    ("Iron Ring of the Gorilla", 0, &[2, 44], Some(35)),
];

const BASE_ITEMS_JSON: &str = "data/base_items.json";
const MODS_JSON: &str = "data/mods.json";
const DATA_README: &str = "data/README.md";

/// Fixed seeds, so every run is identical.
const SEED: u64 = 167;
const ORB_SEED: u64 = 16_703;

/// Items for invariant checks over many seeds.
const ITEMS: u64 = 2_000;
/// Rolls per pool mod for the text check.
const ROLLS_PER_MOD: u64 = 50;
/// Orbs per currency in AC3: enough that the rarest mod (weight 50, about 0.06 % per orb)
/// rolls about 11 times, so every mod of the pool is reached.
const ORB_TRIALS: u64 = 20_000;

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

fn read_text(relative: &str) -> String {
    let path = repo_path(relative);
    let bytes = fs::read(&path).unwrap_or_else(|e| panic!("cannot read {relative}: {e}"));
    String::from_utf8_lossy(&bytes).into_owned()
}

fn load_json(relative: &str) -> Value {
    parse_json(&read_text(relative), relative)
}

fn parse_json(text: &str, what: &str) -> Value {
    serde_json::from_str(text).unwrap_or_else(|e| panic!("{what} is not valid JSON: {e}"))
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
    collect(&repo_path("web").join("src"), &mut files);
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
// Helpers: JSON
// ---------------------------------------------------------------------------------------

fn field<'a>(object: &'a Value, key: &str, what: &str) -> &'a Value {
    object
        .get(key)
        .unwrap_or_else(|| panic!("{what} must have the field {key:?}, found {object}"))
}

fn str_field<'a>(object: &'a Value, key: &str, what: &str) -> &'a str {
    field(object, key, what)
        .as_str()
        .unwrap_or_else(|| panic!("{what}.{key} must be a string"))
}

fn u64_field(object: &Value, key: &str, what: &str) -> u64 {
    field(object, key, what)
        .as_u64()
        .unwrap_or_else(|| panic!("{what}.{key} must be a whole number of at least 0"))
}

fn array_field<'a>(object: &'a Value, key: &str, what: &str) -> &'a Vec<Value> {
    field(object, key, what)
        .as_array()
        .unwrap_or_else(|| panic!("{what}.{key} must be an array"))
}

fn strings_field(object: &Value, key: &str, what: &str) -> Vec<String> {
    array_field(object, key, what)
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

/// The `placeholders` object of a mod entry as a map from placeholder name to stat id.
fn placeholders_field(entry: &Value, what: &str) -> BTreeMap<String, String> {
    field(entry, "placeholders", what)
        .as_object()
        .unwrap_or_else(|| panic!("{what}.placeholders must be an object"))
        .iter()
        .map(|(name, stat)| {
            let stat = stat
                .as_str()
                .unwrap_or_else(|| panic!("{what}.placeholders.{name} must be a string"));
            (name.clone(), stat.to_owned())
        })
        .collect()
}

/// The `stats` array of a mod entry as a map from stat id to (min, max). A stat has no
/// Global flag (ADR 0016).
fn stats_field(entry: &Value, what: &str) -> BTreeMap<String, (i64, i64)> {
    let stats = array_field(entry, "stats", what);
    let map: BTreeMap<String, (i64, i64)> = stats
        .iter()
        .map(|stat| {
            assert!(
                stat.get("global").is_none(),
                "{what}: a stat has no Global flag any more, found {stat}"
            );
            let id = str_field(stat, "id", what).to_owned();
            let bound = |key: &str| {
                field(stat, key, what)
                    .as_i64()
                    .unwrap_or_else(|| panic!("{what}: {id}.{key} must be a whole number"))
            };
            (id.clone(), (bound("min"), bound("max")))
        })
        .collect();
    assert_eq!(map.len(), stats.len(), "{what}: stat ids must be unique");
    map
}

/// The `conversions` object of a mod entry as a map from stat id to (divisor, precision).
/// A missing object counts as empty, a missing or null precision as none.
fn conversions_field(entry: &Value, what: &str) -> BTreeMap<String, (u64, Option<u64>)> {
    let Some(conversions) = entry.get("conversions") else {
        return BTreeMap::new();
    };
    conversions
        .as_object()
        .unwrap_or_else(|| panic!("{what}.conversions must be an object"))
        .iter()
        .map(|(stat, conversion)| {
            let divisor = u64_field(conversion, "divisor", what);
            let precision = match conversion.get("precision") {
                None | Some(Value::Null) => None,
                Some(value) => Some(value.as_u64().unwrap_or_else(|| {
                    panic!("{what}: the precision of {stat:?} must be a whole number or null")
                })),
            };
            (stat.clone(), (divisor, precision))
        })
        .collect()
}

/// The text of a mod entry with every placeholder written as `{<stat id>}`.
fn canonical_entry_text(entry: &Value, what: &str) -> String {
    let placeholders = placeholders_field(entry, what);
    render(str_field(entry, "text", what), &placeholders, |stat| {
        format!("{{{stat}}}")
    })
}

/// Checks a mod entry of a converted data file against the expected values.
fn assert_mod_entry(entry: &Value, expected: &ExpectedMod, file: &str) {
    let what = format!("{file}: {:?}", expected.id);
    assert_eq!(str_field(entry, "id", &what), expected.id, "{what}: id");
    assert_eq!(
        str_field(entry, "name", &what),
        expected.name,
        "{what}: name"
    );
    assert_eq!(
        str_field(entry, "affix_type", &what),
        expected.affix_type,
        "{what}: affix_type"
    );
    assert_eq!(
        str_field(entry, "family", &what),
        expected.family,
        "{what}: family"
    );
    assert_eq!(
        u64_field(entry, "level", &what),
        u64::from(expected.level),
        "{what}: level"
    );
    assert_eq!(
        u64_field(entry, "weight", &what),
        u64::from(expected.weight),
        "{what}: weight"
    );
    assert_eq!(
        strings_field(entry, "tags", &what),
        strings(expected.tags),
        "{what}: tags, unchanged from RePoE"
    );
    let expected_stats: BTreeMap<String, (i64, i64)> = expected
        .stats
        .iter()
        .map(|(id, min, max)| ((*id).to_owned(), (i64::from(*min), i64::from(*max))))
        .collect();
    assert_eq!(stats_field(entry, &what), expected_stats, "{what}: stats");
    assert_eq!(
        canonical_entry_text(entry, &what),
        expected.lines.join("\n"),
        "{what}: text, lines in the game's order"
    );
    let expected_conversions: BTreeMap<String, (u64, Option<u64>)> = expected
        .conversions
        .iter()
        .map(|(stat, divisor, precision)| {
            (
                (*stat).to_owned(),
                (u64::from(*divisor), precision.map(u64::from)),
            )
        })
        .collect();
    assert_eq!(
        conversions_field(entry, &what),
        expected_conversions,
        "{what}: conversions"
    );
}

/// The entries of a JSON array field, by their `id`.
fn entries_by_id<'a>(file: &'a Value, key: &str, what: &str) -> BTreeMap<String, &'a Value> {
    let entries = array_field(file, key, what);
    let map: BTreeMap<String, &Value> = entries
        .iter()
        .map(|entry| (str_field(entry, "id", what).to_owned(), entry))
        .collect();
    assert_eq!(map.len(), entries.len(), "{what}: ids must be unique");
    map
}

// ---------------------------------------------------------------------------------------
// Helpers: texts and the PO's display rules
// ---------------------------------------------------------------------------------------

/// Replaces every placeholder `{name}` of `text` by `value(stat id)`, where `placeholders`
/// maps the name to the stat id. One pass, so a value is never replaced again.
fn render(
    text: &str,
    placeholders: &BTreeMap<String, String>,
    value: impl Fn(&str) -> String,
) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let end = start
            + rest[start..]
                .find('}')
                .unwrap_or_else(|| panic!("unclosed placeholder in {text:?}"));
        let name = &rest[start + 1..end];
        let stat = placeholders
            .get(name)
            .unwrap_or_else(|| panic!("the placeholder {name:?} of {text:?} names no stat"));
        out.push_str(&value(stat));
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    out
}

/// The text of a definition with every placeholder written as `{<stat id>}`.
fn canonical_text(definition: &ModDefinition) -> String {
    render(&definition.text, &definition.placeholders, |stat| {
        format!("{{{stat}}}")
    })
}

/// `scaled / scale` as a decimal number without trailing zeros; `scale` is a power of ten.
fn decimal(scaled: i64, scale: i64) -> String {
    let sign = if scaled < 0 { "-" } else { "" };
    let whole = scaled.abs() / scale;
    let digits = scale.to_string().len() - 1;
    if digits == 0 {
        return format!("{sign}{whole}");
    }
    let fraction = format!("{:0digits$}", scaled.abs() % scale);
    let fraction = fraction.trim_end_matches('0');
    if fraction.is_empty() {
        format!("{sign}{whole}")
    } else {
        format!("{sign}{whole}.{fraction}")
    }
}

/// The value the game shows for a raw stat value, by the PO's rules in "Item basics":
/// without a conversion the raw value; with a precision the value computed as a 32-bit
/// floating-point number, rounded to the precision, exact halves to the even digit; without
/// a precision the exact value. Trailing zeros are dropped.
fn shown_value(raw: i32, conversion: Option<&Conversion>) -> String {
    let Some(conversion) = conversion else {
        return raw.to_string();
    };
    match conversion.precision {
        Some(precision) => {
            let converted = raw as f32 / conversion.divisor as f32;
            let scale = 10_i64.pow(precision);
            // The f32 value is exact in f64; scaling by a power of ten keeps exact halves exact.
            let scaled = (f64::from(converted) * scale as f64).round_ties_even() as i64;
            decimal(scaled, scale)
        }
        None => {
            let divisor = i64::from(conversion.divisor);
            let mut scale = 1_i64;
            while scale % divisor != 0 {
                scale *= 10;
                assert!(
                    scale <= 1_000_000_000,
                    "the test only knows exact values for divisors made of 2 and 5"
                );
            }
            decimal(i64::from(raw) * (scale / divisor), scale)
        }
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

/// The text the PO's rules expect for a rolled mod.
fn expected_display_text(rolled: &Mod) -> String {
    render(&rolled.text, &rolled.placeholders, |stat| {
        shown_value(stat_value(rolled, stat), rolled.conversions.get(stat))
    })
}

// ---------------------------------------------------------------------------------------
// Helpers: model
// ---------------------------------------------------------------------------------------

fn iron_ring() -> BaseItem {
    base_item(IRON_RING_ID)
        .unwrap_or_else(|| panic!("crafting-core must offer the base item {IRON_RING_ID:?}"))
}

fn iron_ring_pool() -> Vec<ExplicitModDefinition> {
    valid_mods(&explicit_mods(), UI_ITEM_LEVEL)
}

fn find_definition<'a>(pool: &'a [ExplicitModDefinition], name: &str) -> &'a ExplicitModDefinition {
    pool.iter()
        .find(|definition| definition.name == name)
        .unwrap_or_else(|| panic!("the pool must contain the mod {name:?}"))
}

fn affix_type_name(affix_type: AffixType) -> &'static str {
    match affix_type {
        AffixType::Prefix => "prefix",
        AffixType::Suffix => "suffix",
    }
}

/// An invented mod definition with one stat.
fn one_stat_mod(level: u32, stat: &str, (min, max): (i32, i32)) -> ModDefinition {
    ModDefinition {
        family: format!("{stat} family"),
        level,
        tags: vec!["test".to_owned()],
        stats: vec![StatDefinition {
            id: stat.to_owned(),
            min,
            max,
        }],
        text: format!("{{value}} {stat}"),
        placeholders: BTreeMap::from([("value".to_owned(), stat.to_owned())]),
        conversions: BTreeMap::new(),
    }
}

/// A copy of `definition` whose stats all roll the fixed value `value`.
fn fixed(definition: &ModDefinition, value: i32) -> ModDefinition {
    let mut copy = definition.clone();
    for stat in &mut copy.stats {
        stat.min = value;
        stat.max = value;
    }
    copy
}

/// An invented base item with the given own requirement and implicit levels.
fn invented_base(required_level: u32, implicit_levels: &[u32]) -> BaseItem {
    BaseItem {
        id: "Metadata/Items/Test/Invented".to_owned(),
        name: "Invented Base".to_owned(),
        item_class: "Inventeds".to_owned(),
        required_level,
        tags: vec!["default".to_owned()],
        implicits: implicit_levels
            .iter()
            .map(|level| one_stat_mod(*level, "invented_implicit", (1, 1)))
            .collect(),
    }
}

/// A magic item of `base` with an explicit mod of each given level (at most one prefix and
/// one suffix).
fn magic_item(base: &BaseItem, explicit_levels: &[u32], rng: &mut SeededRng) -> Item {
    assert!(
        explicit_levels.len() <= 2,
        "a magic item has at most two mods"
    );
    let mut item = Item::new(base, rng);
    for (index, level) in explicit_levels.iter().enumerate() {
        let (affix_type, name) = if index == 0 {
            (AffixType::Prefix, "Invented")
        } else {
            (AffixType::Suffix, "of Invention")
        };
        let rolled = ExplicitMod {
            name: name.to_owned(),
            affix_type,
            modifier: Mod::roll(&one_stat_mod(*level, "invented_explicit", (1, 9)), rng),
        };
        item.rarity = Rarity::Magic;
        match affix_type {
            AffixType::Prefix => item.prefixes.push(rolled),
            AffixType::Suffix => item.suffixes.push(rolled),
        }
    }
    item
}

/// Base requirement, implicit levels, explicit levels, shown requirement.
type LevelCase = (u32, &'static [u32], &'static [u32], Option<u32>);

/// The requirement the PO's rule expects: the higher of the base's own requirement and 80 %
/// of the highest mod level, rounded down; none if it is 1 or lower.
fn expected_required_level(item: &Item) -> Option<u32> {
    let highest_mod_level = item
        .implicits
        .iter()
        .chain(item.prefixes.iter().map(|explicit| &explicit.modifier))
        .chain(item.suffixes.iter().map(|explicit| &explicit.modifier))
        .map(|rolled| rolled.level)
        .max()
        .unwrap_or(0);
    let requirement = item.base.required_level.max(highest_mod_level * 4 / 5);
    (requirement > 1).then_some(requirement)
}

// ---------------------------------------------------------------------------------------
// Helpers: invented raw files in RePoE's format (AC2)
// ---------------------------------------------------------------------------------------

const RING_1: &str = "Metadata/Items/Test/TestRing1";
const RING_2: &str = "Metadata/Items/Test/TestRing2";
const AMULET: &str = "Metadata/Items/Test/TestAmulet1";

/// The six raw files as JSON values, so that test cases can change single values.
#[derive(Clone)]
struct RawSet {
    base_items: Value,
    mods: Value,
    mods_by_base: Value,
    stat_translations: Value,
    stat_value_handlers: Value,
    item_classes: Value,
}

impl RawSet {
    fn files(&self) -> RawFiles {
        RawFiles {
            base_items: self.base_items.to_string(),
            mods: self.mods.to_string(),
            mods_by_base: self.mods_by_base.to_string(),
            stat_translations: self.stat_translations.to_string(),
            stat_value_handlers: self.stat_value_handlers.to_string(),
            item_classes: self.item_classes.to_string(),
        }
    }
}

/// The value at `path` (object keys or array indices) inside `value`.
fn at<'a>(value: &'a mut Value, path: &[&str]) -> &'a mut Value {
    path.iter().fold(value, |current, key| {
        let next = match current {
            Value::Array(items) => {
                let index: usize = key
                    .parse()
                    .unwrap_or_else(|_| panic!("{key:?} is not an array index"));
                items.get_mut(index)
            }
            Value::Object(map) => map.get_mut(*key),
            _ => None,
        };
        next.unwrap_or_else(|| panic!("the invented raw files have no {path:?}"))
    })
}

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

fn raw_base(
    name: &str,
    item_class: &str,
    tags: &[&str],
    implicits: &[&str],
    requirements: Value,
) -> Value {
    json!({
        "domain": "item",
        "drop_level": 7,
        "implicits": implicits,
        "inventory_height": 1,
        "inventory_width": 1,
        "inherits_from": "Metadata/Items/Rings/AbstractRing",
        "item_class": item_class,
        "name": name,
        "properties": null_properties(),
        "release_state": "released",
        "tags": tags,
        "visual_identity": { "dds_file": "Art/2DItems/Test.dds", "id": "Test" },
        "requirements": requirements,
        "grants_buff": null,
        "skills_granted": null
    })
}

fn raw_mod(
    name: &str,
    generation_type: &str,
    group: &str,
    level: u32,
    tags: &[&str],
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
        "implicit_tags": tags,
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

/// One condition of a translation entry.
fn condition(min: Option<i32>, max: Option<i32>) -> Value {
    json!({ "min": min, "max": max, "negated": null })
}

/// One English translation of an entry: conditions, formats and handlers per stat.
fn translation(
    conditions: Vec<Value>,
    formats: &[&str],
    handlers: &[&[&str]],
    string: &str,
) -> Value {
    json!({
        "condition": conditions,
        "format": formats,
        "index_handlers": handlers,
        "string": string,
        "reminder_text": null,
        "is_markup": null
    })
}

fn translation_entry(ids: &[&str], english: Vec<Value>) -> Value {
    json!({
        "English": english,
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

/// A value handler of `stat_value_handlers.json`.
fn handler(
    divisor: Option<f64>,
    multiplier: Option<f64>,
    addend: Option<f64>,
    precision: Option<u32>,
    fixed_value: Option<bool>,
) -> Value {
    json!({
        "addend": addend,
        "divisor": divisor,
        "multiplier": multiplier,
        "precision": precision,
        "fixed": fixed_value,
        "type": "int"
    })
}

/// Indices of the entries in the invented `stat_translations.json`.
const ACCURACY_ENTRY: &str = "0";
const LIFE_ENTRY: &str = "2";
const REGENERATION_ENTRY: &str = "3";
const LEECH_ENTRY: &str = "4";

/// Small invented raw files in RePoE's format: two rings and an amulet, a pool with the
/// categories `prefix`, `suffix`, `corrupted` and `prefix_shaper`, a two-line mod whose stats
/// and RePoE text are in the other order than its translation entries, and both conversions.
fn invented_raw_files() -> RawSet {
    let base_items = json!({
        RING_1: raw_base("Plain Test Ring", "TestRing", &["testring", "default"],
            &["TestImplicitDamage1"], Value::Null),
        RING_2: raw_base("Heavy Test Ring", "TestRing", &["testring", "default"], &[],
            json!({ "dexterity": 0, "intelligence": 0, "level": 12, "strength": 20 })),
        AMULET: raw_base("Test Amulet", "TestAmulet", &["testamulet", "default"], &[],
            Value::Null),
    });
    let mods = json!({
        "TestImplicitDamage1": raw_mod("", "unique", "TestDamage", 3, &["damage", "attack"],
            json!([
                { "id": "test_attack_maximum_added_damage", "max": 6, "min": 6 },
                { "id": "test_attack_minimum_added_damage", "max": 2, "min": 2 }
            ]),
            "Adds 2 to 6 Test Damage to Attacks"),
        "TestLife1": raw_mod("Testy", "prefix", "TestLife", 1, &["resource", "life"],
            json!([{ "id": "test_maximum_life", "max": 9, "min": 3 }]),
            "+(3-9) to maximum Test Life"),
        "TestLife2": raw_mod("Very Testy", "prefix", "TestLife", 20, &["resource", "life"],
            json!([{ "id": "test_maximum_life", "max": 19, "min": 10 }]),
            "+(10-19) to maximum Test Life"),
        "TestRegen1": raw_mod("of Test Regeneration", "suffix", "TestRegen", 7,
            &["flat_life_regen", "resource", "life"],
            json!([{ "id": "test_life_regeneration_rate_per_minute", "max": 480, "min": 126 }]),
            "Regenerate (2.1-8) Test Life per second"),
        "TestLeech1": raw_mod("of Test Leech", "suffix", "TestLeech", 50,
            &["resource", "life", "attack"],
            json!([{ "id": "test_life_leech_permyriad", "max": 40, "min": 20 }]),
            "(0.2-0.4)% of Test Damage Leeched as Life"),
        "TestDual1": raw_mod("of Test Light", "suffix", "TestDual", 8, &["attack"],
            json!([
                { "id": "test_light_radius_+%", "max": 5, "min": 5 },
                { "id": "test_accuracy_rating_+%", "max": 11, "min": 9 }
            ]),
            "5% increased Test Light Radius\n(9-11)% increased Test Accuracy Rating"),
        "TestCorrupted1": raw_mod("Corrupted Test", "corrupted", "TestCorrupted", 1,
            &["life"], json!([{ "id": "test_maximum_life", "max": 2, "min": 1 }]),
            "+(1-2) to maximum Test Life"),
        "TestShaper1": raw_mod("The Shaper's Test", "prefix", "TestShaper", 68, &["life"],
            json!([{ "id": "test_maximum_life", "max": 30, "min": 20 }]),
            "+(20-30) to maximum Test Life"),
        "TestAmuletOnly1": raw_mod("Amulet Testy", "prefix", "TestAmuletLife", 1, &["life"],
            json!([{ "id": "test_maximum_life", "max": 5, "min": 1 }]),
            "+(1-5) to maximum Test Life"),
        "TestUnrelated1": raw_mod("Unrelated Test", "prefix", "TestUnrelated", 1, &["life"],
            json!([{ "id": "test_maximum_life", "max": 5, "min": 1 }]),
            "+(1-5) to maximum Test Life"),
    });
    let mods_by_base = json!({
        "Test Rings": {
            "testring,default": {
                "bases": [RING_1, RING_2],
                "mods": {
                    "suffix": {
                        "TestRegen": { "TestRegen1": 800 },
                        "TestLeech": { "TestLeech1": 250 },
                        "TestDual": { "TestDual1": 300 }
                    },
                    "prefix": { "TestLife": { "TestLife1": 1000, "TestLife2": 500 } },
                    "corrupted": { "TestCorrupted": { "TestCorrupted1": 100 } },
                    "prefix_shaper": { "TestShaper": { "TestShaper1": 400 } }
                },
                "conditional_mods": null
            }
        },
        "Test Amulets": {
            "testamulet,default": {
                "bases": [AMULET],
                "mods": {
                    "prefix": { "TestAmuletLife": { "TestAmuletOnly1": 1000 } },
                    "suffix": { "TestRegen": { "TestRegen1": 800 } }
                },
                "conditional_mods": null
            }
        }
    });
    let any = || condition(None, None);
    let stat_translations = json!([
        translation_entry(
            &["test_accuracy_rating_+%"],
            vec![translation(
                vec![any()],
                &["#"],
                &[&[]],
                "{0}% increased Test Accuracy Rating"
            )]
        ),
        translation_entry(
            &[
                "test_attack_minimum_added_damage",
                "test_attack_maximum_added_damage"
            ],
            vec![translation(
                vec![any(), any()],
                &["#", "#"],
                &[&[], &[]],
                "Adds {0} to {1} Test Damage to Attacks"
            )]
        ),
        translation_entry(
            &["test_maximum_life"],
            vec![
                translation(
                    vec![condition(Some(1), None)],
                    &["+#"],
                    &[&[]],
                    "{0} to maximum Test Life"
                ),
                translation(
                    vec![condition(None, Some(-1))],
                    &["#"],
                    &[&["negate"]],
                    "{0} less maximum Test Life"
                ),
            ]
        ),
        translation_entry(
            &["test_life_regeneration_rate_per_minute"],
            vec![translation(
                vec![any()],
                &["#"],
                &[&["per_minute_to_per_second"]],
                "Regenerate {0} Test Life per second"
            )]
        ),
        translation_entry(
            &["test_life_leech_permyriad"],
            vec![translation(
                vec![condition(Some(1), None)],
                &["#"],
                &[&["divide_by_one_hundred"]],
                "{0}% of Test Damage Leeched as Life"
            )]
        ),
        translation_entry(
            &["test_light_radius_+%"],
            vec![
                translation(
                    vec![condition(Some(1), None)],
                    &["#"],
                    &[&[]],
                    "{0}% increased Test Light Radius"
                ),
                translation(
                    vec![condition(None, Some(-1))],
                    &["#"],
                    &[&["negate"]],
                    "{0}% reduced Test Light Radius"
                ),
            ]
        ),
        translation_entry(
            &["test_unused_stat"],
            vec![translation(
                vec![any()],
                &["#"],
                &[&[]],
                "{0} unused Test Stat"
            )]
        ),
    ]);
    let stat_value_handlers = json!({
        "tq_noop": { "type": "noop" },
        "per_minute_to_per_second": handler(Some(60.0), None, None, Some(1), None),
        "divide_by_one_hundred": handler(Some(100.0), None, None, None, None),
        "negate": handler(None, Some(-1.0), None, None, None),
        "30%_of_value": handler(Some(100.0), Some(30.0), None, None, None),
        "per_minute_to_per_second_1dp": handler(Some(60.0), None, None, Some(1), Some(true)),
        "test_plus_one": handler(None, None, Some(1.0), None, None),
        "test_divide_by_two_and_a_half": handler(Some(2.5), None, None, None, None),
    });
    let item_classes = json!({
        "TestRing": { "category": "Ring", "category_id": "Ring", "name": "Test Rings",
            "influence_tags": [] },
        "TestAmulet": { "category": "Amulet", "category_id": "Amulet", "name": "Test Amulets",
            "influence_tags": [] },
    });
    RawSet {
        base_items,
        mods,
        mods_by_base,
        stat_translations,
        stat_value_handlers,
        item_classes,
    }
}

/// The mods the converter must write for the two invented rings.
const INVENTED_POOL: [ExpectedMod; 5] = [
    ExpectedMod {
        id: "TestLife1",
        name: "Testy",
        affix_type: "prefix",
        family: "TestLife",
        level: 1,
        weight: 1000,
        tags: &["resource", "life"],
        stats: &[("test_maximum_life", 3, 9)],
        lines: &["+{test_maximum_life} to maximum Test Life"],
        conversions: &[],
    },
    ExpectedMod {
        id: "TestLife2",
        name: "Very Testy",
        affix_type: "prefix",
        family: "TestLife",
        level: 20,
        weight: 500,
        tags: &["resource", "life"],
        stats: &[("test_maximum_life", 10, 19)],
        lines: &["+{test_maximum_life} to maximum Test Life"],
        conversions: &[],
    },
    ExpectedMod {
        id: "TestRegen1",
        name: "of Test Regeneration",
        affix_type: "suffix",
        family: "TestRegen",
        level: 7,
        weight: 800,
        tags: &["flat_life_regen", "resource", "life"],
        stats: &[("test_life_regeneration_rate_per_minute", 126, 480)],
        lines: &["Regenerate {test_life_regeneration_rate_per_minute} Test Life per second"],
        conversions: &[("test_life_regeneration_rate_per_minute", 60, Some(1))],
    },
    ExpectedMod {
        id: "TestLeech1",
        name: "of Test Leech",
        affix_type: "suffix",
        family: "TestLeech",
        level: 50,
        weight: 250,
        tags: &["resource", "life", "attack"],
        stats: &[("test_life_leech_permyriad", 20, 40)],
        lines: &["{test_life_leech_permyriad}% of Test Damage Leeched as Life"],
        conversions: &[("test_life_leech_permyriad", 100, None)],
    },
    ExpectedMod {
        id: "TestDual1",
        name: "of Test Light",
        affix_type: "suffix",
        family: "TestDual",
        level: 8,
        weight: 300,
        tags: &["attack"],
        stats: &[
            ("test_light_radius_+%", 5, 5),
            ("test_accuracy_rating_+%", 9, 11),
        ],
        lines: &[
            "{test_accuracy_rating_+%}% increased Test Accuracy Rating",
            "{test_light_radius_+%}% increased Test Light Radius",
        ],
        conversions: &[],
    },
];

/// Runs the converter and expects an error message that names at least one of `hints`
/// (the value, the stat, the mod or the base it could not convert).
fn assert_conversion_fails(label: &str, raw: &RawFiles, base_ids: &[&str], hints: &[&str]) {
    let result = std::panic::catch_unwind(|| convert(raw, base_ids));
    let result = result.unwrap_or_else(|_| {
        panic!("{label}: the converter must stop with a message, not with a panic")
    });
    match result {
        Ok(_) => panic!("{label}: the converter must stop with a message, but it converted"),
        Err(message) => {
            assert!(
                !message.trim().is_empty(),
                "{label}: the converter must give a message"
            );
            assert!(
                hints.is_empty() || hints.iter().any(|hint| message.contains(hint)),
                "{label}: the message {message:?} must name one of {hints:?}"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------------------

// AC1: „`data/` enthält den Iron Ring und alle Mods seines Pools aus RePoE, erzeugt mit dem
// Konverter aus ADR 0015. `data/README.md` nennt Quelle, Version 3.29.3.3, Datum des
// Downloads und die SHA-256-Prüfsumme jeder Rohdatei."
// (The pool is the categories `prefix` and `suffix` of the Iron Ring's entry in
// `mods_by_base.json` (ADR 0016). The data files are checked against figures and samples
// taken from the raw files; that they are the converter's output is checked by the
// developer's second run without a diff. The raw files include `stat_value_handlers.json`
// (ADR 0016).)
// (Adapted for AB#185, PO-confirmed: criterion 5 of AB#185 makes the divisor of `Conversion`
// signed (ADR 0017). The expected divisors are turned into an `i32`.)
#[test]
fn ac1_data_holds_the_iron_ring_and_its_full_repoe_pool_with_provenance() {
    // Given the data files, crafting-core and the provenance in data/README.md
    let base_items = load_json(BASE_ITEMS_JSON);
    let mods = load_json(MODS_JSON);
    let readme = read_text(DATA_README);
    let core_pool = explicit_mods();

    // When looking up the Iron Ring and its pool
    let bases = entries_by_id(&base_items, "base_items", BASE_ITEMS_JSON);
    let ring = bases
        .get(IRON_RING_ID)
        .unwrap_or_else(|| panic!("{BASE_ITEMS_JSON} must contain {IRON_RING_ID}"));
    let what = format!("{BASE_ITEMS_JSON}: the Iron Ring");
    let pool_ids = strings_field(ring, "mod_pool", &what);
    let mod_entries = entries_by_id(&mods, "mods", MODS_JSON);

    // Then the data file holds the Iron Ring as in RePoE: no own level requirement
    assert_eq!(str_field(ring, "name", &what), IRON_RING_NAME);
    assert_eq!(str_field(ring, "item_class", &what), IRON_RING_CLASS);
    assert_eq!(
        u64_field(ring, "required_level", &what),
        0,
        "{what}: a ring has no own level requirement"
    );
    // And its implicit with level, tags, stat identifiers, text and no conversion
    let implicits = array_field(ring, "implicits", &what);
    assert_eq!(implicits.len(), 1, "{what}: one implicit");
    let implicit = &implicits[0];
    let implicit_what = format!("{what}, implicit");
    assert_eq!(
        str_field(implicit, "family", &implicit_what),
        IMPLICIT_FAMILY
    );
    assert_eq!(
        u64_field(implicit, "level", &implicit_what),
        u64::from(IMPLICIT_LEVEL)
    );
    assert_eq!(
        strings_field(implicit, "tags", &implicit_what),
        strings(&IMPLICIT_TAGS)
    );
    let implicit_stats: BTreeMap<String, (i64, i64)> = IMPLICIT_STATS
        .iter()
        .map(|(id, min, max)| ((*id).to_owned(), (i64::from(*min), i64::from(*max))))
        .collect();
    assert_eq!(stats_field(implicit, &implicit_what), implicit_stats);
    assert_eq!(
        canonical_entry_text(implicit, &implicit_what),
        IMPLICIT_LINES.join("\n")
    );
    assert!(conversions_field(implicit, &implicit_what).is_empty());

    // And its pool holds every prefix and suffix of RePoE, each once
    let unique_ids: BTreeSet<&String> = pool_ids.iter().collect();
    assert_eq!(
        unique_ids.len(),
        pool_ids.len(),
        "{what}: the pool lists each mod once"
    );
    assert_eq!(
        pool_ids.len(),
        POOL_PREFIXES + POOL_SUFFIXES,
        "{what}: size of the pool"
    );
    let pool_entries: Vec<&Value> = pool_ids
        .iter()
        .map(|id| {
            *mod_entries
                .get(id)
                .unwrap_or_else(|| panic!("{MODS_JSON} must contain the pool mod {id:?}"))
        })
        .collect();
    let mut families: BTreeMap<String, (String, usize, u64)> = BTreeMap::new();
    let mut names: BTreeSet<&str> = BTreeSet::new();
    for entry in pool_entries.iter().copied() {
        let id = str_field(entry, "id", MODS_JSON);
        let entry_what = format!("{MODS_JSON}: {id:?}");
        let affix_type = str_field(entry, "affix_type", &entry_what).to_owned();
        let family = str_field(entry, "family", &entry_what).to_owned();
        let weight = u64_field(entry, "weight", &entry_what);
        assert!(weight > 0, "{entry_what}: weight above zero");
        assert!(
            names.insert(str_field(entry, "name", &entry_what)),
            "{entry_what}: names are unique"
        );
        // Every mod has all its values, even those no function uses yet
        u64_field(entry, "level", &entry_what);
        strings_field(entry, "tags", &entry_what);
        stats_field(entry, &entry_what);
        canonical_entry_text(entry, &entry_what);
        conversions_field(entry, &entry_what);
        let row = families
            .entry(family.clone())
            .or_insert_with(|| (affix_type.clone(), 0, 0));
        assert_eq!(row.0, affix_type, "{entry_what}: one affix type per family");
        row.1 += 1;
        row.2 += weight;
    }
    let expected_families: BTreeMap<String, (String, usize, u64)> = FAMILIES
        .iter()
        .map(|(family, affix_type, count, weight)| {
            (
                (*family).to_owned(),
                ((*affix_type).to_owned(), *count, *weight),
            )
        })
        .collect();
    assert_eq!(
        families, expected_families,
        "families of the pool: affix type, number of mods, sum of weights"
    );
    let sum_of = |affix: &str| -> (usize, u64) {
        families
            .values()
            .filter(|(affix_type, _, _)| affix_type == affix)
            .fold((0, 0), |(count, weight), row| {
                (count + row.1, weight + row.2)
            })
    };
    assert_eq!(sum_of("prefix"), (POOL_PREFIXES, PREFIX_WEIGHT_SUM));
    assert_eq!(sum_of("suffix"), (POOL_SUFFIXES, SUFFIX_WEIGHT_SUM));
    // And the samples have every value from RePoE
    for sample in &POOL_SAMPLES {
        assert!(
            pool_ids.iter().any(|id| id == sample.id),
            "the Iron Ring's pool must contain {:?}",
            sample.id
        );
        assert_mod_entry(mod_entries[sample.id], sample, MODS_JSON);
    }
    // And every mod of the data file belongs to the pool of a base item
    let all_pool_ids: BTreeSet<String> = bases
        .values()
        .copied()
        .flat_map(|base| strings_field(base, "mod_pool", BASE_ITEMS_JSON))
        .collect();
    let stray: Vec<&String> = mod_entries
        .keys()
        .filter(|id| !all_pool_ids.contains(*id))
        .collect();
    assert!(
        stray.is_empty(),
        "{MODS_JSON} has mods outside every pool: {stray:?}"
    );
    // And no stat has the Global flag any more
    for file in [BASE_ITEMS_JSON, MODS_JSON] {
        assert!(
            !read_text(file).contains("\"global\""),
            "{file} must not contain the Global flag"
        );
    }

    // And crafting-core offers exactly the mods of the data file, with the same values
    let core_names: BTreeSet<&str> = core_pool.iter().map(|m| m.name.as_str()).collect();
    let file_names: BTreeSet<&str> = mod_entries
        .values()
        .copied()
        .map(|entry| str_field(entry, "name", MODS_JSON))
        .collect();
    assert_eq!(
        core_pool.len(),
        mod_entries.len(),
        "explicit_mods() offers every mod"
    );
    assert_eq!(core_names, file_names);
    for sample in &POOL_SAMPLES {
        let definition = find_definition(&core_pool, sample.name);
        let modifier = &definition.definition;
        let what = format!("explicit_mods(): {:?}", sample.name);
        assert_eq!(
            affix_type_name(definition.affix_type),
            sample.affix_type,
            "{what}"
        );
        assert_eq!(definition.weight, sample.weight, "{what}");
        assert_eq!(modifier.level, sample.level, "{what}");
        assert_eq!(modifier.family, sample.family, "{what}");
        assert_eq!(modifier.tags, strings(sample.tags), "{what}");
        let stats: BTreeMap<String, (i32, i32)> = modifier
            .stats
            .iter()
            .map(|stat| (stat.id.clone(), (stat.min, stat.max)))
            .collect();
        let expected_stats: BTreeMap<String, (i32, i32)> = sample
            .stats
            .iter()
            .map(|(id, min, max)| ((*id).to_owned(), (*min, *max)))
            .collect();
        assert_eq!(stats, expected_stats, "{what}");
        assert_eq!(canonical_text(modifier), sample.lines.join("\n"), "{what}");
        let expected_conversions: BTreeMap<String, Conversion> = sample
            .conversions
            .iter()
            .map(|(stat, divisor, precision)| {
                (
                    (*stat).to_owned(),
                    Conversion {
                        divisor: i32::try_from(*divisor).expect("the divisor fits into an i32"),
                        precision: *precision,
                    },
                )
            })
            .collect();
        assert_eq!(modifier.conversions, expected_conversions, "{what}");
    }

    // And data/README.md names the source, the version, the download date and the converter
    for needle in [REPOE_SOURCE, REPOE_VERSION, DOWNLOAD_DATE, CONVERTER] {
        assert!(
            readme.contains(needle),
            "{DATA_README} must name {needle:?}"
        );
    }
    // And the SHA-256 of each of the six raw files, next to its file name
    for (file, sha256) in RAW_FILES {
        assert!(
            readme
                .lines()
                .any(|line| line.contains(file) && line.to_lowercase().contains(sha256)),
            "{DATA_README} must give the SHA-256 {sha256} of the raw file {file} on its line"
        );
    }
}

// AC2: „Der Konverter erzeugt aus denselben Rohdateien Byte für Byte dieselben Dateien und
// bricht mit einer Meldung ab, wenn er einen Wert nicht umwandeln kann."
// (Checked with small invented raw files in RePoE's format through the library function of
// `data-import` (ADR 0016). The cases in which the converter must stop are those of ADR 0015,
// ADR 0016 and the tech lead's plan. That the binary writes nothing after an error is checked
// by the developer.)
// (Adapted for AB#185, PO-confirmed: criterion 5 of AB#185 lets the converter turn the
// handler `negate` into the divisor -1 (ADR 0017). The case "a handler with a multiplier" uses
// the handler `30%_of_value` from RePoE instead, so a multiplier without a rule still stops
// the converter.)
#[test]
fn ac2_converter_is_deterministic_and_stops_with_a_message_on_unconvertible_values() {
    // Given small invented raw files and the two rings to convert
    let raw = invented_raw_files();
    let files = raw.files();
    let rings = [RING_1, RING_2];

    // When the converter runs twice on the same raw files
    let first: Converted = convert(&files, &rings).expect("the invented raw files convert");
    let second: Converted = convert(&files, &rings).expect("the invented raw files convert");

    // Then it gives the same files byte for byte
    assert_eq!(first.base_items_json, second.base_items_json);
    assert_eq!(first.mods_json, second.mods_json);

    // And the base items are the two rings with their own requirement, implicit and pool
    let base_items = parse_json(&first.base_items_json, "the converted base_items.json");
    let bases = entries_by_id(&base_items, "base_items", "the converted base_items.json");
    assert_eq!(
        bases.keys().map(String::as_str).collect::<Vec<_>>(),
        {
            let mut ids = rings.to_vec();
            ids.sort_unstable();
            ids
        },
        "exactly the requested bases"
    );
    let expected_pool: BTreeSet<String> = INVENTED_POOL.iter().map(|m| m.id.to_owned()).collect();
    for (id, name, required_level, implicit_count) in [
        (RING_1, "Plain Test Ring", 0, 1),
        (RING_2, "Heavy Test Ring", 12, 0),
    ] {
        let base = bases[id];
        let what = format!("converted base {id}");
        assert_eq!(str_field(base, "name", &what), name);
        assert_eq!(str_field(base, "item_class", &what), "Test Rings");
        assert_eq!(u64_field(base, "required_level", &what), required_level);
        assert_eq!(
            strings_field(base, "tags", &what)
                .into_iter()
                .collect::<BTreeSet<_>>(),
            ["default", "testring"]
                .iter()
                .map(|tag| (*tag).to_owned())
                .collect::<BTreeSet<_>>()
        );
        assert_eq!(
            array_field(base, "implicits", &what).len(),
            implicit_count,
            "{what}: implicits"
        );
        let pool: BTreeSet<String> = strings_field(base, "mod_pool", &what).into_iter().collect();
        assert_eq!(
            pool, expected_pool,
            "{what}: the pool is the categories prefix and suffix of its entry"
        );
    }
    let implicit = &array_field(bases[RING_1], "implicits", "converted ring 1")[0];
    let what = "converted ring 1, implicit";
    assert_eq!(str_field(implicit, "family", what), "TestDamage");
    assert_eq!(u64_field(implicit, "level", what), 3);
    assert_eq!(strings_field(implicit, "tags", what), ["damage", "attack"]);
    assert_eq!(
        stats_field(implicit, what),
        BTreeMap::from([
            ("test_attack_minimum_added_damage".to_owned(), (2, 2)),
            ("test_attack_maximum_added_damage".to_owned(), (6, 6)),
        ])
    );
    assert_eq!(
        canonical_entry_text(implicit, what),
        "Adds {test_attack_minimum_added_damage} to {test_attack_maximum_added_damage} Test \
         Damage to Attacks"
    );
    assert!(conversions_field(implicit, what).is_empty());

    // And the mods are exactly the pool's prefixes and suffixes, with every value converted:
    // texts from the translation entries in their order, conversions from the handlers
    let mods = parse_json(&first.mods_json, "the converted mods.json");
    let entries = entries_by_id(&mods, "mods", "the converted mods.json");
    assert_eq!(
        entries.keys().cloned().collect::<BTreeSet<_>>(),
        expected_pool,
        "no implicit, corrupted, shaper, amulet or unrelated mod"
    );
    for expected in &INVENTED_POOL {
        assert_mod_entry(entries[expected.id], expected, "the converted mods.json");
    }

    // When a value cannot be converted
    // Then the converter stops with a message that names it, and does not panic
    let cases: Vec<(&str, RawSet, Vec<&str>, Vec<&str>)> = vec![
        (
            "an unknown base",
            raw.clone(),
            vec![RING_1, "Metadata/Items/Test/NoSuchRing"],
            vec!["NoSuchRing"],
        ),
        {
            let mut changed = raw.clone();
            *at(&mut changed.base_items, &[RING_1, "item_class"]) = json!("TestUnknownClass");
            (
                "an unknown item class",
                changed,
                rings.to_vec(),
                vec!["TestUnknownClass", RING_1],
            )
        },
        {
            let mut changed = raw.clone();
            changed
                .mods
                .as_object_mut()
                .expect("mods")
                .remove("TestLeech1");
            (
                "a pool mod missing from mods.json",
                changed,
                rings.to_vec(),
                vec!["TestLeech1"],
            )
        },
        {
            let mut changed = raw.clone();
            *at(&mut changed.mods, &["TestLife1", "groups"]) = json!(["TestLife", "TestOther"]);
            (
                "a mod with two groups",
                changed,
                rings.to_vec(),
                vec!["TestLife1"],
            )
        },
        {
            let mut changed = raw.clone();
            *at(
                &mut changed.mods_by_base,
                &[
                    "Test Rings",
                    "testring,default",
                    "mods",
                    "prefix",
                    "TestLife",
                    "TestLife2",
                ],
            ) = json!(0);
            (
                "a mod with weight 0",
                changed,
                rings.to_vec(),
                vec!["TestLife2"],
            )
        },
        {
            let mut changed = raw.clone();
            *at(&mut changed.mods, &["TestLife2", "name"]) = json!("Testy");
            (
                "two mods with the same name",
                changed,
                rings.to_vec(),
                vec!["Testy", "TestLife1", "TestLife2"],
            )
        },
        {
            // The heavy ring gets its own pool, in which TestLife1 has another weight
            let mut changed = raw.clone();
            *at(
                &mut changed.mods_by_base,
                &["Test Rings", "testring,default", "bases"],
            ) = json!([RING_1]);
            let heavy = json!({
                "bases": [RING_2],
                "mods": { "prefix": { "TestLife": { "TestLife1": 900 } } },
                "conditional_mods": null
            });
            at(&mut changed.mods_by_base, &["Test Rings"])
                .as_object_mut()
                .expect("item class entry")
                .insert("testring,heavy,default".to_owned(), heavy);
            (
                "a mod with different weights in two pools",
                changed,
                rings.to_vec(),
                vec!["TestLife1"],
            )
        },
        {
            let mut changed = raw.clone();
            changed
                .stat_translations
                .as_array_mut()
                .expect("stat translations")
                .remove(LEECH_ENTRY.parse().expect("an index"));
            (
                "a stat without a translation",
                changed,
                rings.to_vec(),
                vec!["test_life_leech_permyriad", "TestLeech1"],
            )
        },
        {
            let mut changed = raw.clone();
            *at(&mut changed.mods, &["TestLife1", "stats", "0", "min"]) = json!(-2);
            (
                "a range that needs two translations",
                changed,
                rings.to_vec(),
                vec!["test_maximum_life", "TestLife1"],
            )
        },
        {
            let mut changed = raw.clone();
            *at(
                &mut changed.stat_translations,
                &[REGENERATION_ENTRY, "English", "0", "format", "0"],
            ) = json!("ignore");
            (
                "a format other than # and +#",
                changed,
                rings.to_vec(),
                vec![
                    "ignore",
                    "test_life_regeneration_rate_per_minute",
                    "TestRegen1",
                ],
            )
        },
        {
            let mut changed = raw.clone();
            *at(
                &mut changed.stat_translations,
                &[REGENERATION_ENTRY, "English", "0", "index_handlers", "0"],
            ) = json!(["test_unknown_handler"]);
            (
                "an unknown handler",
                changed,
                rings.to_vec(),
                vec![
                    "test_unknown_handler",
                    "test_life_regeneration_rate_per_minute",
                    "TestRegen1",
                ],
            )
        },
        {
            let mut changed = raw.clone();
            *at(
                &mut changed.stat_translations,
                &[LIFE_ENTRY, "English", "0", "index_handlers", "0"],
            ) = json!(["30%_of_value"]);
            (
                "a handler with a multiplier",
                changed,
                rings.to_vec(),
                vec!["30%_of_value", "test_maximum_life", "TestLife1"],
            )
        },
        {
            let mut changed = raw.clone();
            *at(
                &mut changed.stat_translations,
                &[LEECH_ENTRY, "English", "0", "index_handlers", "0"],
            ) = json!(["test_plus_one"]);
            (
                "a handler with an addend",
                changed,
                rings.to_vec(),
                vec!["test_plus_one", "test_life_leech_permyriad", "TestLeech1"],
            )
        },
        {
            let mut changed = raw.clone();
            *at(
                &mut changed.stat_translations,
                &[REGENERATION_ENTRY, "English", "0", "index_handlers", "0"],
            ) = json!(["per_minute_to_per_second_1dp"]);
            (
                "a handler with a fixed value",
                changed,
                rings.to_vec(),
                vec![
                    "per_minute_to_per_second_1dp",
                    "test_life_regeneration_rate_per_minute",
                    "TestRegen1",
                ],
            )
        },
        {
            let mut changed = raw.clone();
            *at(
                &mut changed.stat_translations,
                &[ACCURACY_ENTRY, "English", "0", "index_handlers", "0"],
            ) = json!(["test_divide_by_two_and_a_half"]);
            (
                "a divisor that is not a whole number",
                changed,
                rings.to_vec(),
                vec![
                    "test_divide_by_two_and_a_half",
                    "test_accuracy_rating_+%",
                    "TestDual1",
                ],
            )
        },
    ];
    for (label, changed, base_ids, hints) in &cases {
        assert_conversion_fails(label, &changed.files(), base_ids, hints);
    }
    // And a raw file that is not valid JSON gives a message too
    let mut broken = raw.files();
    broken.mods.truncate(broken.mods.len() / 2);
    assert_conversion_fails("a broken raw file", &broken, &rings, &[]);
}

// AC3: „Orb of Transmutation und Orb of Alteration würfeln auf dem Iron Ring aus dem
// vollständigen Pool nach ihren Regeln, mit Item-Level 100."
// (The shares of the rules with the full pool are checked by the tests of AB#62 (AC7) and
// AB#114 (AC2), which use the pool from crafting-core. Here: the pool at item level 100 is
// the Iron Ring's whole pool, both orbs reach every mod of it and no other, and the page
// builds the pool with the item level rule.)
// (Adapted for AB#185, PO-confirmed: criterion 6 of AB#185 gives every base its own pool
// (ADR 0017). The Iron Ring's pool comes from `mod_pool` instead of `explicit_mods()`, which
// now offers the mods of all ring bases, and the page takes it from `mod_pool`.)
// (Adapted for AB#248, PO-confirmed: criterion 4 of AB#248 replaces the check that the page
// builds the pool with `valid_mods`. The page passes the base's pool from `mod_pool`, and the
// orbs apply the item level rule themselves (ADR 0021, `story_248::ac4`). The rest stays.)
#[test]
fn ac3_both_orbs_roll_from_the_full_pool_at_item_level_100() {
    // Given the Iron Ring, its pool in the data file and the pool at the UI's item level
    let base = iron_ring();
    let base_items = load_json(BASE_ITEMS_JSON);
    let mods = load_json(MODS_JSON);
    let ring = entries_by_id(&base_items, "base_items", BASE_ITEMS_JSON)[IRON_RING_ID];
    let mod_entries = entries_by_id(&mods, "mods", MODS_JSON);
    let file_pool: BTreeMap<String, (String, u64, u64)> =
        strings_field(ring, "mod_pool", BASE_ITEMS_JSON)
            .iter()
            .map(|id| {
                let entry = mod_entries[id.as_str()];
                (
                    str_field(entry, "name", MODS_JSON).to_owned(),
                    (
                        str_field(entry, "affix_type", MODS_JSON).to_owned(),
                        u64_field(entry, "level", MODS_JSON),
                        u64_field(entry, "weight", MODS_JSON),
                    ),
                )
            })
            .collect();
    let pool = valid_mods(
        &mod_pool(IRON_RING_ID)
            .unwrap_or_else(|| panic!("crafting-core must offer the pool of {IRON_RING_ID:?}")),
        UI_ITEM_LEVEL,
    );

    // Then the pool at item level 100 is the Iron Ring's whole pool, with its levels and weights
    let core_pool: BTreeMap<String, (String, u64, u64)> = pool
        .iter()
        .map(|definition| {
            (
                definition.name.clone(),
                (
                    affix_type_name(definition.affix_type).to_owned(),
                    u64::from(definition.definition.level),
                    u64::from(definition.weight),
                ),
            )
        })
        .collect();
    assert_eq!(pool.len(), POOL_PREFIXES + POOL_SUFFIXES);
    assert_eq!(core_pool, file_pool);

    // When each orb is applied many times
    let mut rng = SeededRng::new(ORB_SEED);
    let mut rolled_by_orb: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
    let mut one_mod_prefixes: BTreeMap<&str, (u64, u64)> = BTreeMap::new();
    let mut altered = Item::new(&base, &mut rng);
    apply_orb_of_transmutation(&mut altered, &pool, &mut rng).expect("the orb applies");
    for _ in 0..ORB_TRIALS {
        let mut transmuted = Item::new(&base, &mut rng);
        apply_orb_of_transmutation(&mut transmuted, &pool, &mut rng)
            .expect("the Orb of Transmutation applies to a normal ring");
        apply_orb_of_alteration(&mut altered, &pool, &mut rng)
            .expect("the Orb of Alteration applies to a magic ring");
        for (orb, item) in [("transmutation", &transmuted), ("alteration", &altered)] {
            assert_eq!(item.rarity, Rarity::Magic, "{orb}: the ring is magic");
            // Then every new mod comes from the pool, in its slot
            for (slot, explicits) in [
                (AffixType::Prefix, &item.prefixes),
                (AffixType::Suffix, &item.suffixes),
            ] {
                for explicit in explicits {
                    let definition = find_definition(&pool, &explicit.name);
                    assert_eq!(definition.affix_type, slot, "{orb}: {:?}", explicit.name);
                    assert_eq!(explicit.affix_type, slot, "{orb}: {:?}", explicit.name);
                    rolled_by_orb
                        .entry(orb)
                        .or_default()
                        .insert(explicit.name.clone());
                }
            }
            if item.prefixes.is_empty() != item.suffixes.is_empty() {
                let counts = one_mod_prefixes.entry(orb).or_default();
                counts.1 += 1;
                if !item.prefixes.is_empty() {
                    counts.0 += 1;
                }
            }
        }
    }

    // And each orb reaches every mod of the pool
    let all_names: BTreeSet<String> = pool.iter().map(|m| m.name.clone()).collect();
    for orb in ["transmutation", "alteration"] {
        let rolled = rolled_by_orb.get(orb).cloned().unwrap_or_default();
        let missing: Vec<&String> = all_names.difference(&rolled).collect();
        assert!(
            missing.is_empty(),
            "{orb}: in {ORB_TRIALS} orbs these mods of the pool never rolled: {missing:?}"
        );
        // And with one mod, the share of prefixes follows the weights of the whole pool
        let (prefixes, one_mod) = one_mod_prefixes.get(orb).copied().unwrap_or_default();
        let p = PREFIX_WEIGHT_SUM as f64 / (PREFIX_WEIGHT_SUM + SUFFIX_WEIGHT_SUM) as f64;
        let observed = prefixes as f64 / one_mod as f64;
        let tolerance = 5.0 * (p * (1.0 - p) / one_mod as f64).sqrt();
        assert!(
            (observed - p).abs() <= tolerance,
            "{orb}: prefixes among results with one mod: observed {observed:.4} ({prefixes} of \
             {one_mod}), expected {p:.4} \u{b1} {tolerance:.4}"
        );
    }
    // And the page takes the pool from crafting-core
    assert_web_src_uses("mod_pool", "the page takes the pool from crafting-core");
}

// AC4: „Mod-Texte erscheinen wie im Spiel: mit den gewürfelten Werten, bei mehrzeiligen Mods
// in der Reihenfolge des Spiels, und mit der Umrechnung aus den RePoE-Daten, wo das Spiel
// umrechnet."
// (PO: "wie im Spiel" means values, line order and conversion; no reminder texts yet. The
// display of converted values follows the PO's rules of AC8 and AC9 in "Item basics".)
// (Adapted for AB#185, PO-confirmed: criterion 5 of AB#185 makes the divisor of `Conversion`
// signed (ADR 0017). The expected divisors are turned into an `i32`.)
#[test]
fn ac4_mod_texts_show_rolled_values_in_game_line_order_with_conversion() {
    let mut rng = SeededRng::new(SEED);

    // --- The PO's examples for the conversion ---
    // Given a mod stored per minute and shown per second with one decimal, and a mod stored
    // in permyriad and shown in percent without a precision
    let mut per_second = one_stat_mod(1, "invented_per_minute", (1, 1));
    per_second.conversions = BTreeMap::from([(
        "invented_per_minute".to_owned(),
        Conversion {
            divisor: 60,
            precision: Some(1),
        },
    )]);
    let mut leech = one_stat_mod(1, "invented_permyriad", (1, 1));
    leech.conversions = BTreeMap::from([(
        "invented_permyriad".to_owned(),
        Conversion {
            divisor: 100,
            precision: None,
        },
    )]);
    for (definition, examples) in [
        (&per_second, &PER_SECOND_EXAMPLES[..]),
        (&leech, &LEECH_EXAMPLES[..]),
    ] {
        for (raw, shown) in examples {
            // When the mod rolls the raw value
            let rolled = Mod::roll(&fixed(definition, *raw), &mut rng);
            // Then the text shows the converted value as the rules say
            let stat = &definition.stats[0].id;
            assert_eq!(
                rolled.display_text(),
                format!("{shown} {stat}"),
                "raw value {raw} of {stat}"
            );
            assert_eq!(shown_value(*raw, definition.conversions.get(stat)), *shown);
            // And the rolled mod keeps the conversion and the raw value
            assert_eq!(rolled.conversions, definition.conversions);
            assert_eq!(stat_value(&rolled, stat), *raw);
        }
    }
    // And a stat without a conversion is shown as rolled, also next to a converted one
    let mixed = ModDefinition {
        family: "Invented".to_owned(),
        level: 1,
        tags: Vec::new(),
        stats: vec![
            StatDefinition {
                id: "invented_plain".to_owned(),
                min: 135,
                max: 135,
            },
            StatDefinition {
                id: "invented_converted".to_owned(),
                min: 135,
                max: 135,
            },
        ],
        text: "+{plain} and {converted}".to_owned(),
        placeholders: BTreeMap::from([
            ("plain".to_owned(), "invented_plain".to_owned()),
            ("converted".to_owned(), "invented_converted".to_owned()),
        ]),
        conversions: BTreeMap::from([(
            "invented_converted".to_owned(),
            Conversion {
                divisor: 60,
                precision: Some(1),
            },
        )]),
    };
    assert_eq!(Mod::roll(&mixed, &mut rng).display_text(), "+135 and 2.2");

    // --- The Iron Ring's pool ---
    // Given the pool from crafting-core
    let pool = iron_ring_pool();

    // Then the game converts exactly the stats that RePoE converts, with RePoE's parameters
    let mut converted: BTreeMap<String, Conversion> = BTreeMap::new();
    let mut mods_with_conversion = 0;
    for definition in &pool {
        let modifier = &definition.definition;
        if !modifier.conversions.is_empty() {
            mods_with_conversion += 1;
        }
        for (stat, conversion) in &modifier.conversions {
            assert!(
                modifier.stats.iter().any(|s| s.id == *stat),
                "{:?}: the conversion names its own stat {stat:?}",
                definition.name
            );
            if let Some(earlier) = converted.insert(stat.clone(), conversion.clone()) {
                assert_eq!(
                    &earlier, conversion,
                    "{stat:?} converts the same in every mod"
                );
            }
        }
    }
    let expected: BTreeMap<String, Conversion> = POOL_CONVERSIONS
        .iter()
        .map(|(stat, divisor, precision)| {
            (
                (*stat).to_owned(),
                Conversion {
                    divisor: i32::try_from(*divisor).expect("the divisor fits into an i32"),
                    precision: *precision,
                },
            )
        })
        .collect();
    assert_eq!(converted, expected);
    assert_eq!(mods_with_conversion, POOL_MODS_WITH_CONVERSION);
    assert!(
        iron_ring()
            .implicits
            .iter()
            .all(|m| m.conversions.is_empty()),
        "the Iron Ring's implicit is shown as rolled"
    );

    // When every mod of the pool is rolled many times
    for definition in &pool {
        for _ in 0..ROLLS_PER_MOD {
            let rolled = Mod::roll(&definition.definition, &mut rng);
            // Then its text shows the rolled values, converted where the game converts
            assert_eq!(
                rolled.display_text(),
                expected_display_text(&rolled),
                "{:?}",
                definition.name
            );
        }
        // And every raw value a converted mod can roll is shown as the rules say
        if !definition.definition.conversions.is_empty() {
            let stat = &definition.definition.stats[0];
            assert_eq!(
                definition.definition.stats.len(),
                1,
                "{:?}",
                definition.name
            );
            for raw in stat.min..=stat.max {
                let rolled = Mod::roll(&fixed(&definition.definition, raw), &mut rng);
                assert_eq!(
                    rolled.display_text(),
                    expected_display_text(&rolled),
                    "{:?} with the raw value {raw}",
                    definition.name
                );
            }
        }
    }

    // And mods with several lines show them in the game's order
    for name in TWO_LINE_MODS {
        let definition = &find_definition(&pool, name).definition;
        let lines: Vec<String> = canonical_text(definition)
            .lines()
            .map(str::to_owned)
            .collect();
        assert_eq!(lines.len(), TWO_LINE_ORDER.len(), "{name:?} has two lines");
        for (line, stat) in lines.iter().zip(TWO_LINE_ORDER) {
            assert!(
                line.contains(&format!("{{{stat}}}")),
                "{name:?}: the game shows the line of {stat:?} here, found {lines:?}"
            );
        }
        let rolled = Mod::roll(definition, &mut rng);
        assert_eq!(rolled.display_text().lines().count(), 2, "{name:?}");
    }
    let multi_line: Vec<&str> = pool
        .iter()
        .filter(|definition| definition.definition.text.contains('\n'))
        .map(|definition| definition.name.as_str())
        .collect();
    assert_eq!(
        multi_line.into_iter().collect::<BTreeSet<_>>(),
        TWO_LINE_MODS.into_iter().collect::<BTreeSet<_>>(),
        "the pool's mods with several lines"
    );
}

// AC5: „Die angezeigte Level-Anforderung folgt der Regel in `poe-crafting-rules`. Ein
// normaler Iron Ring zeigt keine Level-Anforderung."
// (The rule is the PO's paragraph of AC6 in "Item basics". The examples are those of its
// evidence. The page shows the requirement from crafting-core; its rendering is checked by
// the PO in the local preview.)
#[test]
fn ac5_level_requirement_follows_the_rule_and_a_normal_iron_ring_shows_none() {
    let mut rng = SeededRng::new(SEED);

    // --- A normal Iron Ring ---
    // Given the Iron Ring: no own requirement, an implicit of level 2
    let base = iron_ring();
    assert_eq!(base.required_level, 0, "a ring has no own requirement");
    assert_eq!(
        base.implicits.iter().map(|m| m.level).collect::<Vec<_>>(),
        [IMPLICIT_LEVEL]
    );
    for _ in 0..ITEMS {
        // When a normal Iron Ring is created
        let item = Item::new(&base, &mut rng);
        // Then its implicit keeps its level, and it shows no level requirement (80 % of 2 is 1)
        assert_eq!(item.implicits[0].level, IMPLICIT_LEVEL);
        assert_eq!(item.required_level(), None);
    }

    // --- Iron Ring of the Gorilla (mod level 44) ---
    let pool = iron_ring_pool();
    let gorilla = find_definition(&pool, "of the Gorilla");
    let mut item = Item::new(&base, &mut rng);
    item.rarity = Rarity::Magic;
    item.suffixes = vec![ExplicitMod {
        name: gorilla.name.clone(),
        affix_type: gorilla.affix_type,
        modifier: Mod::roll(&gorilla.definition, &mut rng),
    }];
    assert_eq!(item.suffixes.first().map(|m| m.modifier.level), Some(44));
    assert_eq!(item.required_level(), Some(35));

    // --- The examples of the rule's evidence, with invented bases ---
    for (label, base_requirement, levels, expected) in LEVEL_EXAMPLES {
        let (implicit_levels, explicit_levels) = levels.split_at(1);
        let item = magic_item(
            &invented_base(base_requirement, implicit_levels),
            explicit_levels,
            &mut rng,
        );
        assert_eq!(item.required_level(), expected, "{label}");
    }

    // --- The rule at its edges ---
    let edges: [LevelCase; 12] = [
        (0, &[], &[], None),
        (1, &[], &[], None),
        (2, &[], &[], Some(2)),
        (0, &[1], &[], None),
        (0, &[2], &[], None),
        (0, &[3], &[], Some(2)),
        (0, &[], &[3], Some(2)),
        (0, &[32], &[10], Some(25)),
        (0, &[10], &[32, 1], Some(25)),
        (0, &[2], &[1, 85], Some(68)),
        (40, &[2], &[44], Some(40)),
        (40, &[2], &[51, 10], Some(40)),
    ];
    for (base_requirement, implicit_levels, explicit_levels, expected) in edges {
        let item = magic_item(
            &invented_base(base_requirement, implicit_levels),
            explicit_levels,
            &mut rng,
        );
        assert_eq!(
            item.required_level(),
            expected,
            "base {base_requirement}, implicits {implicit_levels:?}, mods {explicit_levels:?}"
        );
    }
    // And 80 % of the mod level is rounded down: 51 * 0.8 = 40.8 gives 40
    let item = magic_item(&invented_base(0, &[]), &[51], &mut rng);
    assert_eq!(item.required_level(), Some(40));

    // --- Rings crafted with both orbs ---
    for trial in 0..ITEMS {
        // When an Iron Ring is made magic and altered
        let mut item = Item::new(&base, &mut rng);
        apply_orb_of_transmutation(&mut item, &pool, &mut rng).expect("the orb applies");
        let transmuted = item.clone();
        apply_orb_of_alteration(&mut item, &pool, &mut rng).expect("the orb applies");
        for item in [&transmuted, &item] {
            // Then every rolled mod keeps the level of its definition
            for explicit in item.prefixes.iter().chain(item.suffixes.iter()) {
                assert_eq!(
                    explicit.modifier.level,
                    find_definition(&pool, &explicit.name).definition.level,
                    "trial {trial}: level of {:?}",
                    explicit.name
                );
            }
            // And the requirement follows the rule
            assert_eq!(
                item.required_level(),
                expected_required_level(item),
                "trial {trial}: {}",
                item.name()
            );
        }
    }

    // And the page shows the requirement that crafting-core computes
    assert_web_src_uses(
        "required_level()",
        "the page shows the level requirement from crafting-core",
    );
}
