//! Acceptance tests for AB#248 "Ein Mod wird zufällig hinzugefügt: eine Regel für alle
//! Currencies".
//!
//! The tests use the public interface of `crafting-core` from the tech lead's plan on AB#248
//! and ADR 0021:
//! - `Item { base, rarity, item_level, implicits, prefixes, suffixes }` and
//!   `DEFAULT_ITEM_LEVEL`; `Item::new(base, rng)` gives a normal item at `DEFAULT_ITEM_LEVEL`
//!   without prefixes and suffixes.
//! - `AffixLimits { mods, prefixes, suffixes }` and `AffixLimits::of(rarity)`.
//! - `valid_pool(item, base_pool)`: step 1 of the rule, in the order of `base_pool`.
//! - `add_random_mod(item, base_pool, rng) -> bool`: true if a mod was added.
//! - `apply_orb_of_transmutation` and `apply_orb_of_alteration` with unchanged signatures; their
//!   pool is the base's pool, and their only errors are `ItemNotNormal` and `ItemNotMagic`.
//! - unchanged: `valid_mods`, `unblocked_mods`, `mod_pool`, `base_item`, `Slot`.
//!
//! The PO's rules are `poe-crafting-rules`: "Affix limits" in "Item basics", "Adding a random
//! mod", and the currency files of both orbs. PO decisions on AB#248: the limits come from the
//! rarity only (the lines "+N Prefix Modifiers allowed" and "-N Suffix Modifiers allowed" come
//! with AB#189, so the Composite Ring is not tested here); implicit mods count neither toward
//! the limits nor for the family rule; both orbs never fail on an empty pool.
//!
//! Criteria 1, 1a, 2 and 3 give the PO's wording for `poe-crafting-rules`. That wording is
//! checked byte for byte at the acceptance (step 2 of the `story` skill), not here. Criterion 1a
//! only changes wording, so it has no test. The tests of criteria 1 to 3 check the behaviour of
//! the rules.
//!
//! Every random test uses a fixed seed. Statistical tests compare observed shares with the
//! shares that follow from the weights and the rule, with a tolerance of 5 standard deviations.
//! The tests depend only on frequencies and invariants, never on the order in which the
//! generator is used, except where the plan promises equal results for equal seeds (criterion
//! 4).
//!
//! Red state before the implementation: the module does not compile, because the new interface
//! does not exist yet.
//!
//! What is checked outside `cargo test`, by the PO in the local preview: criterion 6 on the
//! page itself.
//!
//! Adapted for AB#189 (PO-confirmed on AB#189): criterion 1 of AB#189 adds the six Heist rings
//! to the slot "Ring" (43 ring bases). Each of them has a side with the magic limit 0, so the
//! checks of criterion 6 and of the valid pool of every new magic ring in criterion 2 cover the
//! 37 rings of AB#248 only. The Heist rings are checked by the tests of AB#189.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crafting_core::{
    add_random_mod, apply_orb_of_alteration, apply_orb_of_transmutation, base_item, currencies,
    mod_pool, unblocked_mods, valid_mods, valid_pool, AffixLimits, AffixType, BaseItem, CraftError,
    Currency, ExplicitMod, ExplicitModDefinition, Item, Mod, ModDefinition, Rarity, SeededRng,
    Slot, StatDefinition, DEFAULT_ITEM_LEVEL,
};
use serde_json::Value;

// ---------------------------------------------------------------------------------------
// Values from the story and the PO's rules
// ---------------------------------------------------------------------------------------

const IRON_RING_ID: &str = "Metadata/Items/Rings/Ring1";
/// The ring of the PO's evidence for implicits: its implicit is Cold Resistance.
const SAPPHIRE_RING_ID: &str = "Metadata/Items/Rings/Ring6";
/// A Breach ring with the line "Cannot roll Modifiers of Non-Cold Damage Types" (AB#188).
const CRYONIC_RING_ID: &str = "Metadata/Items/Rings/RingB1";
/// The suffix of the PO's evidence: Cold Resistance, the family of the Sapphire Ring's implicit.
const POLAR_BEAR: &str = "of the Polar Bear";

/// The rings on the page: the 37 of AB#248 and the six Heist rings of AB#189.
const RING_BASES: usize = 43;
/// The rings on the page at AB#248 (story text and criterion 6): all ring bases except the six
/// Heist rings, which AB#189 added (PO confirmation on AB#189).
const TODAYS_RING_BASES: usize = 37;
/// The Heist rings of AB#189 (RePoE ids `RingE1` to `RingE6`).
const HEIST_RING_IDS: [&str; 6] = [
    "Metadata/Items/Rings/RingE1",
    "Metadata/Items/Rings/RingE2",
    "Metadata/Items/Rings/RingE3",
    "Metadata/Items/Rings/RingE4",
    "Metadata/Items/Rings/RingE5",
    "Metadata/Items/Rings/RingE6",
];

/// The default item level of the rule "Item level".
const UI_ITEM_LEVEL: u32 = 100;

/// The game message that these orbs never show (criterion 3).
const NO_VALID_MOD_MESSAGE: &str = "Failed to apply item: Could not generate Mod.";

const TRANSMUTATION_ID: &str = "orb_of_transmutation";
const ALTERATION_ID: &str = "orb_of_alteration";
const CURRENCIES_JSON: &str = "data/currencies.json";

/// Affix limits as (mods, prefixes, suffixes), from "Affix limits" in "Item basics". A normal
/// item has no prefixes or suffixes.
type Limits = (usize, usize, usize);
const NORMAL_LIMITS: Limits = (0, 0, 0);
const MAGIC_LIMITS: Limits = (2, 1, 1);
const RARE_LIMITS: Limits = (6, 3, 3);

/// Fixed seeds, so every run is identical.
const SEED: u64 = 248;
const STATISTICS_SEED: u64 = 24_807;
const ORB_SEED: u64 = 24_808;
const RING_SEED: u64 = 24_809;

/// Items for invariant checks with invented pools.
const ITEMS: u64 = 500;
/// Items for invariant checks with the whole pool of a ring base.
const RING_ITEMS: u64 = 200;
/// Items filled with mods until no mod is added.
const FILLED_ITEMS: u64 = 100;
/// Trials per statistical case with invented pools. With 20 000 trials, 5 standard deviations
/// of a share are at most 0.018, so the former rule 3 (the prefix with the suffix of its own
/// family in 16.7 % of the results) and a filter that is left out fail clearly.
const TRIALS: u64 = 20_000;
/// Seeds per ring base and item level for the comparison of the whole pool with the filtered
/// pool.
const EQUAL_SEEDS: u64 = 5;
/// Orbs of each kind per ring base (criterion 6). The prefix share of a ring's pool is about
/// 0.37. Over all 37 rings, about 3700 results have one mod; 5 standard deviations of the
/// prefix share are then about 0.04, so a 50:50 choice between prefix and suffix fails
/// clearly, and so does a wrong share of 1 and 2 mods (5 standard deviations: 0.03). Each orb
/// rebuilds the valid pool of about 200 mods for every mod it adds, so more orbs would make the
/// test take much longer than a few seconds.
const ORBS_PER_BASE: u64 = 100;

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

/// All files below `dir`.
fn files_below(dir: &Path) -> Vec<PathBuf> {
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
    collect(dir, &mut files);
    files
}

/// All files under `web/src/`.
fn web_src_files() -> Vec<PathBuf> {
    let files = files_below(&repo_path("web").join("src"));
    assert!(
        files
            .iter()
            .any(|path| path.extension().is_some_and(|e| e == "rs")),
        "web/src/ must contain Rust source files; the scan found none"
    );
    files
}

/// All files under `crafting-core/src/`.
fn core_src_files() -> Vec<PathBuf> {
    let files = files_below(&repo_path("crafting-core").join("src"));
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

/// How often the files define a function `name`, plain or generic.
fn definitions_of(files: &[PathBuf], name: &str) -> usize {
    files
        .iter()
        .map(|path| {
            let text = read_path(path);
            text.matches(&format!("fn {name}(")).count()
                + text.matches(&format!("fn {name}<")).count()
        })
        .sum()
}

// ---------------------------------------------------------------------------------------
// Helpers: model
// ---------------------------------------------------------------------------------------

fn ring(id: &str) -> BaseItem {
    base_item(id).unwrap_or_else(|| panic!("crafting-core must offer the base item {id:?}"))
}

fn pool_of(id: &str) -> Vec<ExplicitModDefinition> {
    mod_pool(id).unwrap_or_else(|| panic!("crafting-core must offer the pool of {id:?}"))
}

/// The ring bases of the slot "Ring" at AB#248: all of them except the Heist rings of AB#189.
fn todays_rings() -> Vec<BaseItem> {
    let rings: Vec<BaseItem> = Slot::Ring
        .base_items()
        .into_iter()
        .filter(|base| !HEIST_RING_IDS.contains(&base.id.as_str()))
        .collect();
    assert_eq!(rings.len(), TODAYS_RING_BASES, "the ring bases of AB#248");
    rings
}

/// An invented mod with one stat whose range is real, so values are rolled.
fn test_mod(
    name: &str,
    affix_type: AffixType,
    family: &str,
    weight: u32,
    level: u32,
    tags: &[&str],
) -> ExplicitModDefinition {
    let stat_id = format!("{name} stat");
    ExplicitModDefinition {
        name: name.to_owned(),
        affix_type,
        weight,
        definition: ModDefinition {
            family: family.to_owned(),
            level,
            tags: tags.iter().map(|tag| (*tag).to_owned()).collect(),
            stats: vec![StatDefinition {
                id: stat_id.clone(),
                min: 1,
                max: 10,
            }],
            text: format!("+{{value}} to {name}"),
            placeholders: BTreeMap::from([("value".to_owned(), stat_id)]),
            conversions: BTreeMap::new(),
        },
    }
}

/// An invented prefix of level 1 with a tag that is no damage type.
fn prefix(name: &str, family: &str, weight: u32) -> ExplicitModDefinition {
    test_mod(name, AffixType::Prefix, family, weight, 1, &["test"])
}

/// An invented suffix of level 1 with a tag that is no damage type.
fn suffix(name: &str, family: &str, weight: u32) -> ExplicitModDefinition {
    test_mod(name, AffixType::Suffix, family, weight, 1, &["test"])
}

/// Invented mods that items of these tests already hold, each of its own family.
fn held_mods(affix_type: AffixType, count: usize) -> Vec<ExplicitModDefinition> {
    (1..=count)
        .map(|n| {
            let name = match affix_type {
                AffixType::Prefix => format!("Held Prefix {n}"),
                AffixType::Suffix => format!("of the Held Suffix {n}"),
            };
            let family = format!("{name} family");
            test_mod(&name, affix_type, &family, 1000, 1, &["test"])
        })
        .collect()
}

/// An invented implicit mod of the given family.
fn implicit(family: &str) -> ModDefinition {
    test_mod(
        &format!("{family} implicit"),
        AffixType::Prefix,
        family,
        1,
        1,
        &["test"],
    )
    .definition
}

/// An invented ring without a lock, with the given implicits.
fn invented_base(implicits: Vec<ModDefinition>) -> BaseItem {
    BaseItem {
        id: "Metadata/Items/Test/AB248Ring".to_owned(),
        name: "Invented Ring".to_owned(),
        item_class: "Rings".to_owned(),
        required_level: 0,
        tags: vec!["ring".to_owned(), "default".to_owned()],
        implicits,
    }
}

fn rolled(definition: &ExplicitModDefinition, rng: &mut SeededRng) -> ExplicitMod {
    ExplicitMod {
        name: definition.name.clone(),
        affix_type: definition.affix_type,
        modifier: Mod::roll(&definition.definition, rng),
    }
}

/// A new item of `base` with the given rarity, prefixes and suffixes, rolled from the
/// definitions.
fn item_with(
    base: &BaseItem,
    rarity: Rarity,
    prefixes: &[&ExplicitModDefinition],
    suffixes: &[&ExplicitModDefinition],
    rng: &mut SeededRng,
) -> Item {
    let mut item = Item::new(base, rng);
    item.rarity = rarity;
    item.prefixes = prefixes
        .iter()
        .map(|definition| rolled(definition, rng))
        .collect();
    item.suffixes = suffixes
        .iter()
        .map(|definition| rolled(definition, rng))
        .collect();
    item
}

fn limits_of(rarity: Rarity) -> Option<Limits> {
    AffixLimits::of(rarity).map(|limits| (limits.mods, limits.prefixes, limits.suffixes))
}

fn names(pool: &[ExplicitModDefinition]) -> Vec<String> {
    pool.iter()
        .map(|definition| definition.name.clone())
        .collect()
}

fn find<'a>(pool: &'a [ExplicitModDefinition], name: &str) -> &'a ExplicitModDefinition {
    pool.iter()
        .find(|definition| definition.name == name)
        .unwrap_or_else(|| panic!("the pool must contain the mod {name:?}"))
}

/// The prefixes and suffixes of an item.
fn explicits(item: &Item) -> Vec<&ExplicitMod> {
    item.prefixes.iter().chain(item.suffixes.iter()).collect()
}

/// The names of the prefixes and suffixes of an item, sorted: the result of an orb.
fn outcome(item: &Item) -> Vec<String> {
    let mut names: Vec<String> = explicits(item)
        .iter()
        .map(|explicit| explicit.name.clone())
        .collect();
    names.sort();
    names
}

/// Asserts that the item's prefixes and suffixes are all of different families.
fn assert_different_families(item: &Item, ctx: &str) {
    let mods = explicits(item);
    let families: BTreeSet<&str> = mods
        .iter()
        .map(|explicit| explicit.modifier.family.as_str())
        .collect();
    assert_eq!(
        families.len(),
        mods.len(),
        "{ctx}: two mods of the same family in {:?}",
        outcome(item)
    );
}

/// Checks that `explicit` was rolled from `definition`: same name, type and mod data, every
/// stat value within its range.
fn assert_rolled_from(definition: &ExplicitModDefinition, explicit: &ExplicitMod, ctx: &str) {
    let what = format!("{ctx}: {:?}", explicit.name);
    assert_eq!(explicit.name, definition.name, "{what}: name");
    assert_eq!(explicit.affix_type, definition.affix_type, "{what}: type");
    let expected = &definition.definition;
    let modifier = &explicit.modifier;
    assert_eq!(modifier.family, expected.family, "{what}: family");
    assert_eq!(modifier.level, expected.level, "{what}: level");
    assert_eq!(modifier.tags, expected.tags, "{what}: tags");
    assert_eq!(modifier.text, expected.text, "{what}: text");
    assert_eq!(
        modifier.placeholders, expected.placeholders,
        "{what}: placeholders"
    );
    assert_eq!(
        modifier.conversions, expected.conversions,
        "{what}: conversions"
    );
    assert_eq!(modifier.stats.len(), expected.stats.len(), "{what}: stats");
    for stat in &expected.stats {
        let value = modifier
            .stats
            .iter()
            .find(|rolled| rolled.id == stat.id)
            .unwrap_or_else(|| panic!("{what}: the mod must have the stat {:?}", stat.id))
            .value;
        assert!(
            (stat.min..=stat.max).contains(&value),
            "{what}: {:?} rolled {value} outside {}..={}",
            stat.id,
            stat.min,
            stat.max
        );
    }
}

/// Asserts that `after` is `before` with exactly one more mod at the end of its side, and
/// returns that mod.
fn assert_one_mod_added(before: &Item, after: &Item, ctx: &str) -> ExplicitMod {
    assert_eq!(after.base, before.base, "{ctx}: the base stays");
    assert_eq!(after.rarity, before.rarity, "{ctx}: the rarity stays");
    assert_eq!(
        after.item_level, before.item_level,
        "{ctx}: the item level stays"
    );
    assert_eq!(
        after.implicits, before.implicits,
        "{ctx}: the implicits stay"
    );
    let (side, kept, other_after, other_before, affix_type) =
        if after.prefixes.len() > before.prefixes.len() {
            (
                &after.prefixes,
                &before.prefixes,
                &after.suffixes,
                &before.suffixes,
                AffixType::Prefix,
            )
        } else {
            (
                &after.suffixes,
                &before.suffixes,
                &after.prefixes,
                &before.prefixes,
                AffixType::Suffix,
            )
        };
    assert_eq!(
        side.len(),
        kept.len() + 1,
        "{ctx}: exactly one mod is added, before {:?}, after {:?}",
        outcome(before),
        outcome(after)
    );
    assert_eq!(
        &side[..kept.len()],
        &kept[..],
        "{ctx}: the mods on the side of the new mod stay, in their order"
    );
    assert_eq!(other_after, other_before, "{ctx}: the other side stays");
    let added = side.last().expect("one mod was added").clone();
    assert_eq!(
        added.affix_type, affix_type,
        "{ctx}: {:?} sits on its own side",
        added.name
    );
    added
}

/// The level requirement of the rule in "Item basics": the higher of the base's own
/// requirement and 80 % of the highest level of all mods, rounded down; none if 1 or lower.
fn expected_required_level(item: &Item) -> Option<u32> {
    let highest = item
        .implicits
        .iter()
        .chain(
            item.prefixes
                .iter()
                .chain(item.suffixes.iter())
                .map(|explicit| &explicit.modifier),
        )
        .map(|modifier| modifier.level)
        .max()
        .unwrap_or(0);
    let requirement = item.base.required_level.max(highest * 4 / 5);
    (requirement > 1).then_some(requirement)
}

// ---------------------------------------------------------------------------------------
// Helpers: the PO's rules as expected chances, for invented pools
// ---------------------------------------------------------------------------------------

/// The exact chance of every result of "Adding a random mod", repeated until the item has
/// `target` prefixes and suffixes or no mod is added, starting from an item without prefixes
/// and suffixes. For invented pools on an item without a lock whose item level allows every
/// mod, so step 1 leaves out only mods without room and mods of a family already on the item.
fn rule_outcomes(
    pool: &[ExplicitModDefinition],
    limits: Limits,
    target: usize,
) -> BTreeMap<Vec<String>, f64> {
    fn step<'a>(
        pool: &'a [ExplicitModDefinition],
        limits: Limits,
        target: usize,
        chosen: &mut Vec<&'a ExplicitModDefinition>,
        chance: f64,
        out: &mut BTreeMap<Vec<String>, f64>,
    ) {
        let (max_mods, max_prefixes, max_suffixes) = limits;
        let prefixes = chosen
            .iter()
            .filter(|definition| definition.affix_type == AffixType::Prefix)
            .count();
        let suffixes = chosen.len() - prefixes;
        let valid: Vec<&'a ExplicitModDefinition> = pool
            .iter()
            .filter(|definition| {
                let side_has_room = match definition.affix_type {
                    AffixType::Prefix => prefixes < max_prefixes,
                    AffixType::Suffix => suffixes < max_suffixes,
                };
                side_has_room
                    && chosen.len() < max_mods
                    && chosen
                        .iter()
                        .all(|held| held.definition.family != definition.definition.family)
            })
            .collect();
        if chosen.len() == target || valid.is_empty() {
            let mut key: Vec<String> = chosen.iter().map(|d| d.name.clone()).collect();
            key.sort();
            *out.entry(key).or_default() += chance;
            return;
        }
        let total: f64 = valid.iter().map(|d| f64::from(d.weight)).sum();
        for definition in valid {
            chosen.push(definition);
            let next = chance * f64::from(definition.weight) / total;
            step(pool, limits, target, chosen, next, out);
            chosen.pop();
        }
    }
    let mut out = BTreeMap::new();
    step(pool, limits, target, &mut Vec::new(), 1.0, &mut out);
    out
}

/// The exact chance of every result of both orbs on a magic item (rule in both currency
/// files): a target of 1 or 2 mods, each with a 50 % chance, then random mods are added until
/// the item has that many mods or no mod is added.
fn orb_outcomes(pool: &[ExplicitModDefinition]) -> BTreeMap<Vec<String>, f64> {
    let mut out: BTreeMap<Vec<String>, f64> = BTreeMap::new();
    for target in [1, 2] {
        for (key, chance) in rule_outcomes(pool, MAGIC_LIMITS, target) {
            *out.entry(key).or_default() += 0.5 * chance;
        }
    }
    out
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

// ---------------------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------------------

// Criterion 1: „Regel des PO, Wortlaut für poe-crafting-rules (verbindlich): Affix limits. A
// magic item has at most 2 mods and at most 1 prefix and 1 suffix. A rare item has at most 6
// mods and at most 3 prefixes and 3 suffixes. A mod line "+N Prefix Modifiers allowed" or "-N
// Suffix Modifiers allowed" (also singular) changes the limit of that side by N; a side's limit
// is never below 0. The total limit does not change. Both limits hold at all times. Implicit
// mods count neither toward these limits nor for the family rule in Adding a random mod.
// Evidence: 200 Orbs of Alteration on a Composite Ring ("+3 Prefix Modifiers allowed", "-3
// Suffix Modifiers allowed"), measured by the PO on 2026-10-30: never a suffix, 1 prefix 98
// times, 2 prefixes 102 times, never more than 2 mods. Evidence for implicits: a magic Sapphire
// Ring, copied by the PO on 2026-10-31, has the implicit "+26(20-30)% to Cold Resistance" and
// the suffix "of the Polar Bear" "+37(36-41)% to Cold Resistance"."
// (The wording is checked at the acceptance. Here: the limits by rarity, that both limits
// decide the room of a side, that they hold whenever a mod is added, and that implicits count
// neither for the limits nor for the family rule, with the PO's Sapphire Ring. The lines
// "Modifier(s) allowed" come with AB#189 (PO decision).)
#[test]
fn ac1_affix_limits_follow_the_rarity_and_implicits_do_not_count() {
    let mut rng = SeededRng::new(SEED);

    // --- The limits by rarity ---
    // Given the rarities Normal, Magic and Rare
    // When asking for their affix limits
    // Then a normal item has room for no prefix or suffix, a magic item for 2 mods with at most
    // 1 prefix and 1 suffix, and a rare item for 6 mods with at most 3 prefixes and 3 suffixes
    assert_eq!(limits_of(Rarity::Normal), Some(NORMAL_LIMITS), "normal");
    assert_eq!(limits_of(Rarity::Magic), Some(MAGIC_LIMITS), "magic");
    assert_eq!(limits_of(Rarity::Rare), Some(RARE_LIMITS), "rare");

    // --- Both limits decide whether a side has room ---
    // Given items without implicits that hold some prefixes and suffixes, and a pool of other
    // families
    let base = invented_base(Vec::new());
    let held_prefixes = held_mods(AffixType::Prefix, 3);
    let held_suffixes = held_mods(AffixType::Suffix, 3);
    let pool = vec![
        prefix("Open Prefix A", "Open Prefix Family A", 1000),
        suffix("of the Open Suffix A", "Open Suffix Family A", 1000),
        prefix("Open Prefix B", "Open Prefix Family B", 1000),
        suffix("of the Open Suffix B", "Open Suffix Family B", 1000),
    ];
    for (rarity, (max_mods, max_prefixes, max_suffixes)) in [
        (Rarity::Normal, NORMAL_LIMITS),
        (Rarity::Magic, MAGIC_LIMITS),
        (Rarity::Rare, RARE_LIMITS),
    ] {
        for held_p in 0..=max_prefixes {
            for held_s in 0..=max_suffixes {
                let prefixes: Vec<&ExplicitModDefinition> =
                    held_prefixes.iter().take(held_p).collect();
                let suffixes: Vec<&ExplicitModDefinition> =
                    held_suffixes.iter().take(held_s).collect();
                let item = item_with(&base, rarity, &prefixes, &suffixes, &mut rng);

                // When building the valid pool
                let valid = valid_pool(&item, &pool);

                // Then a mod is in it only if its side has room under the side's limit and
                // under the total limit
                let total_has_room = held_p + held_s < max_mods;
                let expected: Vec<String> = pool
                    .iter()
                    .filter(|definition| {
                        total_has_room
                            && match definition.affix_type {
                                AffixType::Prefix => held_p < max_prefixes,
                                AffixType::Suffix => held_s < max_suffixes,
                            }
                    })
                    .map(|definition| definition.name.clone())
                    .collect();
                assert_eq!(
                    names(&valid),
                    expected,
                    "{rarity:?} item with {held_p} prefixes and {held_s} suffixes"
                );
            }
        }
    }

    // --- Implicits count neither toward the limits nor for the family rule ---
    // Given a magic item with two implicits, whose families a pool prefix and a pool suffix have
    let two_implicits = invented_base(vec![
        implicit("Implicit Family A"),
        implicit("Implicit Family B"),
    ]);
    let shared_pool = vec![
        prefix("Implicit Family Prefix", "Implicit Family A", 1000),
        suffix("of the Implicit Family", "Implicit Family B", 1000),
        suffix("of Another Family", "Another Family", 1000),
    ];
    let magic = item_with(&two_implicits, Rarity::Magic, &[], &[], &mut rng);
    assert_eq!(magic.implicits.len(), 2, "the item has two implicits");
    // When building the valid pool
    // Then every mod of the pool is in it: the implicits take no room and block no family
    assert_eq!(
        valid_pool(&magic, &shared_pool),
        shared_pool,
        "magic item with two implicits"
    );
    // And with a prefix, both suffixes still have room: the item has 1 mod, not 3
    let held = prefix("Held Prefix", "Held Family", 1000);
    let with_prefix = item_with(&two_implicits, Rarity::Magic, &[&held], &[], &mut rng);
    assert_eq!(
        names(&valid_pool(&with_prefix, &shared_pool)),
        ["of the Implicit Family", "of Another Family"],
        "magic item with two implicits and a prefix"
    );

    // And the PO's evidence: a magic Sapphire Ring can get "of the Polar Bear", although its
    // implicit is Cold Resistance as well
    let sapphire = ring(SAPPHIRE_RING_ID);
    let sapphire_pool = pool_of(SAPPHIRE_RING_ID);
    let polar_bear = find(&sapphire_pool, POLAR_BEAR).clone();
    assert!(
        sapphire
            .implicits
            .iter()
            .any(|implicit| implicit.family == polar_bear.definition.family),
        "the Sapphire Ring's implicit must have the family of {POLAR_BEAR:?} ({:?})",
        polar_bear.definition.family
    );
    let magic_sapphire = item_with(&sapphire, Rarity::Magic, &[], &[], &mut rng);
    assert!(
        names(&valid_pool(&magic_sapphire, &sapphire_pool)).contains(&POLAR_BEAR.to_owned()),
        "the valid pool of a magic Sapphire Ring must contain {POLAR_BEAR:?}"
    );
    let mut with_polar_bear = magic_sapphire.clone();
    assert!(
        add_random_mod(
            &mut with_polar_bear,
            std::slice::from_ref(&polar_bear),
            &mut rng
        ),
        "{POLAR_BEAR:?} can be added to a magic Sapphire Ring"
    );
    let added = assert_one_mod_added(&magic_sapphire, &with_polar_bear, "Sapphire Ring");
    assert_rolled_from(&polar_bear, &added, "Sapphire Ring");

    // --- Both limits hold at all times ---
    // Given magic and rare Iron Rings and the Iron Ring's pool
    let iron = ring(IRON_RING_ID);
    let iron_pool = pool_of(IRON_RING_ID);
    for (rarity, (max_mods, max_prefixes, max_suffixes)) in
        [(Rarity::Magic, MAGIC_LIMITS), (Rarity::Rare, RARE_LIMITS)]
    {
        for trial in 0..FILLED_ITEMS {
            let mut item = item_with(&iron, rarity, &[], &[], &mut rng);
            let mut added = 0;
            // When random mods are added until no mod is added
            while add_random_mod(&mut item, &iron_pool, &mut rng) {
                added += 1;
                // Then after every added mod, both limits hold
                assert!(
                    item.prefixes.len() <= max_prefixes
                        && item.suffixes.len() <= max_suffixes
                        && item.prefixes.len() + item.suffixes.len() <= max_mods,
                    "{rarity:?} Iron Ring {trial}: {} prefixes and {} suffixes after {added} mods",
                    item.prefixes.len(),
                    item.suffixes.len()
                );
            }
        }
    }
}

// Criterion 2: „Regel des PO, Wortlaut für poe-crafting-rules (verbindlich): Adding a random
// mod. 1. Build the valid pool: every mod of the base's pool whose level is at most the item
// level, that is not blocked (Breach rings), whose side still has room under both affix limits,
// and whose family is not already on the item. 2. If the pool is empty, no mod is added. 3.
// Otherwise choose one mod from the pool; its chance is its weight divided by the sum of the
// weights in the pool. Every currency that adds a random mod uses this rule; what happens when
// no mod is added is part of each currency's rule. Evidence: the PO's knowledge of the game (an
// essence mod and the normal mod of the same family never appear together); on the Composite
// Ring, 31 different prefix pairs and never two of the same family."
// (The wording is checked at the acceptance. Here: each filter of step 1 with `valid_pool`, also
// with the game data of all ring bases; step 2 and step 3 with `add_random_mod`. The room under
// the limits is checked in AC1. That both orbs use the rule is checked in AC3 and AC4.)
// (Adapted for AB#189, PO-confirmed: the check with the game data covers the 37 ring bases of
// AB#248; every Heist ring has a side with the magic limit 0, so its valid pool is checked by
// the tests of AB#189.)
#[test]
fn ac2_adding_a_random_mod_builds_the_valid_pool_and_chooses_by_weight() {
    let mut rng = SeededRng::new(SEED);
    let iron = ring(IRON_RING_ID);
    let cryonic = ring(CRYONIC_RING_ID);

    // --- The item level belongs to the item ---
    // Given a new Iron Ring
    let item = Item::new(&iron, &mut rng);
    // Then it is a normal item without prefixes and suffixes, at the default item level of 100
    assert_eq!(
        DEFAULT_ITEM_LEVEL, UI_ITEM_LEVEL,
        "the default of the rule \"Item level\""
    );
    assert_eq!(item.item_level, DEFAULT_ITEM_LEVEL);
    assert_eq!(item.rarity, Rarity::Normal);
    assert!(item.prefixes.is_empty() && item.suffixes.is_empty());

    // --- Step 1: the mod's level is at most the item level ---
    // Given mods of different levels on both sides, in no particular order
    let leveled: Vec<ExplicitModDefinition> = [60, 1, 101, 59, 100, 61, 2]
        .iter()
        .enumerate()
        .map(|(index, level)| {
            let affix_type = if index % 2 == 0 {
                AffixType::Prefix
            } else {
                AffixType::Suffix
            };
            let name = format!("Level {level}");
            test_mod(
                &name,
                affix_type,
                &format!("{name} family"),
                1000,
                *level,
                &["test"],
            )
        })
        .collect();
    for item_level in [1, 2, 58, 59, 60, 61, 99, 100, 101] {
        // When building the valid pool of a magic item of that item level
        let mut item = item_with(&iron, Rarity::Magic, &[], &[], &mut rng);
        item.item_level = item_level;
        let valid = valid_pool(&item, &leveled);
        // Then it holds exactly the mods up to the item level, in the order of the base's pool
        // and unchanged
        let expected: Vec<ExplicitModDefinition> = leveled
            .iter()
            .filter(|definition| definition.definition.level <= item_level)
            .cloned()
            .collect();
        assert_eq!(valid, expected, "item level {item_level}");
    }

    // --- Step 1: blocked mods (Breach rings) ---
    // Given mods with tags of damage types and of other kinds, and whether the Cryonic Ring's
    // lock to cold keeps them
    let tagged: [(&str, AffixType, &[&str], bool); 9] = [
        ("Cold Prefix", AffixType::Prefix, &["cold"], true),
        ("Fire Prefix", AffixType::Prefix, &["fire"], false),
        ("of Lightning", AffixType::Suffix, &["lightning"], false),
        ("of Chaos", AffixType::Suffix, &["chaos"], false),
        ("Physical Prefix", AffixType::Prefix, &["physical"], false),
        (
            "of the Elements",
            AffixType::Suffix,
            &["elemental", "resistance"],
            true,
        ),
        (
            "Weapon Prefix",
            AffixType::Prefix,
            &["physical_damage", "damage"],
            true,
        ),
        ("of Ruin", AffixType::Suffix, &["elemental_damage"], true),
        (
            "of Frost and Flame",
            AffixType::Suffix,
            &["cold", "fire"],
            false,
        ),
    ];
    let tagged_pool: Vec<ExplicitModDefinition> = tagged
        .iter()
        .map(|(name, affix_type, tags, _)| {
            test_mod(name, *affix_type, &format!("{name} family"), 1000, 1, tags)
        })
        .collect();
    let kept: Vec<&str> = tagged
        .iter()
        .filter(|(_, _, _, kept)| *kept)
        .map(|(name, _, _, _)| *name)
        .collect();
    // When building the valid pool of a magic Cryonic Ring
    let on_cryonic = item_with(&cryonic, Rarity::Magic, &[], &[], &mut rng);
    // Then the mods with a tag of another damage type are left out
    assert_eq!(
        names(&valid_pool(&on_cryonic, &tagged_pool)),
        kept,
        "Cryonic Ring"
    );
    // And on the Iron Ring, which has no lock, none is left out
    let on_iron = item_with(&iron, Rarity::Magic, &[], &[], &mut rng);
    assert_eq!(valid_pool(&on_iron, &tagged_pool), tagged_pool, "Iron Ring");
    // And with the game data, the valid pool of every new magic ring of AB#248 is its pool
    // without the mods its lock blocks, at the item level 100
    for base in todays_rings() {
        let whole = pool_of(&base.id);
        let item = item_with(&base, Rarity::Magic, &[], &[], &mut rng);
        assert_eq!(
            valid_pool(&item, &whole),
            valid_mods(&unblocked_mods(&base.id, &whole), UI_ITEM_LEVEL),
            "{} {}",
            base.name,
            base.id
        );
    }

    // --- Step 1: the family is not already on the item ---
    // Given mods of a shared family on both sides, and mods of other families
    let shared_prefix = prefix("Shared Prefix", "Shared Family", 1000);
    let second_shared_prefix = prefix("Second Shared Prefix", "Shared Family", 1000);
    let other_prefix = prefix("Other Prefix", "Other Prefix Family", 1000);
    let shared_suffix = suffix("of the Shared Family", "Shared Family", 1000);
    let other_suffix = suffix("of the Other Family", "Other Suffix Family", 1000);
    let family_pool = vec![
        shared_prefix.clone(),
        second_shared_prefix.clone(),
        other_prefix.clone(),
        shared_suffix.clone(),
        other_suffix.clone(),
    ];
    for (label, rarity, prefixes, suffixes, expected) in [
        (
            "magic item with the shared prefix",
            Rarity::Magic,
            vec![&shared_prefix],
            vec![],
            vec!["of the Other Family"],
        ),
        (
            "rare item with the shared prefix",
            Rarity::Rare,
            vec![&shared_prefix],
            vec![],
            vec!["Other Prefix", "of the Other Family"],
        ),
        (
            "rare item with the shared suffix",
            Rarity::Rare,
            vec![],
            vec![&shared_suffix],
            vec!["Other Prefix", "of the Other Family"],
        ),
    ] {
        // When building the valid pool of an item that holds a mod of the shared family
        let item = item_with(&iron, rarity, &prefixes, &suffixes, &mut rng);
        // Then no mod of that family is in it, on either side
        assert_eq!(names(&valid_pool(&item, &family_pool)), expected, "{label}");
    }

    // --- Step 2: if the pool is empty, no mod is added ---
    let full_rare_prefixes = held_mods(AffixType::Prefix, 3);
    let full_rare_suffixes = held_mods(AffixType::Suffix, 3);
    let mut low_level = item_with(&iron, Rarity::Magic, &[], &[], &mut rng);
    low_level.item_level = 1;
    let above_level_1: Vec<ExplicitModDefinition> = leveled
        .iter()
        .filter(|definition| definition.definition.level > 1)
        .cloned()
        .collect();
    let blocked_on_cryonic: Vec<ExplicitModDefinition> = tagged_pool
        .iter()
        .filter(|definition| !kept.contains(&definition.name.as_str()))
        .cloned()
        .collect();
    let empty_cases: Vec<(&str, Item, Vec<ExplicitModDefinition>)> = vec![
        (
            "an empty base pool",
            item_with(&iron, Rarity::Magic, &[], &[], &mut rng),
            Vec::new(),
        ),
        (
            "a normal item",
            Item::new(&iron, &mut rng),
            family_pool.clone(),
        ),
        (
            "a magic item with a prefix and a suffix",
            item_with(
                &iron,
                Rarity::Magic,
                &[&other_prefix],
                &[&other_suffix],
                &mut rng,
            ),
            family_pool.clone(),
        ),
        (
            "a rare item with 3 prefixes and 3 suffixes",
            item_with(
                &iron,
                Rarity::Rare,
                &full_rare_prefixes.iter().collect::<Vec<_>>(),
                &full_rare_suffixes.iter().collect::<Vec<_>>(),
                &mut rng,
            ),
            family_pool.clone(),
        ),
        ("only mods above the item level", low_level, above_level_1),
        (
            "only mods the lock blocks",
            on_cryonic.clone(),
            blocked_on_cryonic,
        ),
        (
            "only mods of a family on the item",
            item_with(&iron, Rarity::Rare, &[&shared_prefix], &[], &mut rng),
            vec![second_shared_prefix.clone(), shared_suffix.clone()],
        ),
    ];
    for (label, mut item, pool) in empty_cases {
        // Given an item whose valid pool is empty
        assert!(
            valid_pool(&item, &pool).is_empty(),
            "{label}: the valid pool must be empty"
        );
        let before = item.clone();
        // When a random mod is added
        let added = add_random_mod(&mut item, &pool, &mut rng);
        // Then no mod is added, and the item is unchanged
        assert!(!added, "{label}: no mod is added");
        assert_eq!(item, before, "{label}: the item is unchanged");
    }

    // --- Step 3: one mod of the valid pool, chosen by weight ---
    // Given a pool in which each filter of step 1 leaves out a mod
    let weighted_pool = vec![
        test_mod(
            "Alpha Prefix",
            AffixType::Prefix,
            "Alpha",
            3000,
            1,
            &["test"],
        ),
        test_mod("Beta Prefix", AffixType::Prefix, "Beta", 1000, 1, &["test"]),
        test_mod("of Alpha", AffixType::Suffix, "Alpha", 2000, 1, &["test"]),
        test_mod("of Cold", AffixType::Suffix, "Gamma", 4000, 1, &["cold"]),
        test_mod(
            "of the Heights",
            AffixType::Suffix,
            "Delta",
            5000,
            101,
            &["test"],
        ),
        test_mod("of Fire", AffixType::Suffix, "Epsilon", 5000, 1, &["fire"]),
        test_mod(
            "of the Storm",
            AffixType::Suffix,
            "Zeta",
            1000,
            1,
            &["elemental"],
        ),
    ];
    let alpha_prefix = weighted_pool[0].clone();
    let mut stats_rng = SeededRng::new(STATISTICS_SEED);
    let weight_cases = [
        (
            "a new magic Cryonic Ring",
            item_with(&cryonic, Rarity::Magic, &[], &[], &mut stats_rng),
            vec![
                "Alpha Prefix",
                "Beta Prefix",
                "of Alpha",
                "of Cold",
                "of the Storm",
            ],
        ),
        (
            "a magic Cryonic Ring with \"Alpha Prefix\"",
            item_with(
                &cryonic,
                Rarity::Magic,
                &[&alpha_prefix],
                &[],
                &mut stats_rng,
            ),
            vec!["of Cold", "of the Storm"],
        ),
    ];
    for (label, item, expected) in &weight_cases {
        // And the valid pool of the item, from the PO's rule
        assert_eq!(
            names(&valid_pool(item, &weighted_pool)),
            *expected,
            "{label}: valid pool"
        );
        let total: f64 = expected
            .iter()
            .map(|name| f64::from(find(&weighted_pool, name).weight))
            .sum();
        let mut counts: BTreeMap<String, u64> = BTreeMap::new();
        for trial in 0..TRIALS {
            let ctx = format!("{label}, trial {trial}");
            let mut after = item.clone();
            // When a random mod is added
            let was_added = add_random_mod(&mut after, &weighted_pool, &mut stats_rng);
            // Then exactly one mod of the valid pool is added to its side, rolled from its
            // definition, and nothing else changes
            assert!(was_added, "{ctx}: a mod is added");
            let added = assert_one_mod_added(item, &after, &ctx);
            assert!(
                expected.contains(&added.name.as_str()),
                "{ctx}: {:?} is not in the valid pool",
                added.name
            );
            assert_rolled_from(find(&weighted_pool, &added.name), &added, &ctx);
            *counts.entry(added.name.clone()).or_default() += 1;
        }
        // And each mod's chance is its weight divided by the sum of the weights in the pool
        for name in expected {
            assert_share(
                &format!("{label}: {name:?}"),
                counts.get(*name).copied().unwrap_or(0),
                TRIALS,
                f64::from(find(&weighted_pool, name).weight) / total,
            );
        }
    }
}

// Criterion 3: „Regel des PO, Wortlaut für poe-crafting-rules (verbindlich), ersetzt die
// bisherigen Regeln 1 bis 4 von Orb of Transmutation und Orb of Alteration: The orb chooses a
// target number of mods: 1 or 2, each with a 50 % chance. It then adds random mods until the
// item has that many mods or no mod is added. The orb always takes effect, also when it adds no
// mod; the item can become magic with no mods at all. These orbs never show "Failed to apply
// item: Could not generate Mod." Evidence: Focused Amulet (magic limits 0 prefixes and 0
// suffixes), tested by the PO on 2026-10-31: Transmutation and Alteration each gave a magic item
// with no mods and no message. For an item with at most 1 prefix and 1 suffix, this rule gives
// the same chances as the old rules 2 and 3, so the Bone Ring measurements still hold."
// (The wording is checked at the acceptance. Here: both orbs take effect without a valid mod,
// they have no message "Could not generate Mod." in crafting-core and in the game data, and the
// chances of their results follow the rule on a pool where it differs from the old rules 2 and
// 3: a prefix and a suffix share a family. The PO's Focused Amulet needs the lines "Modifier(s)
// allowed" (AB#189); an empty valid pool stands in for it. The same chances as before on
// today's rings are checked in AC6.)
#[test]
fn ac3_both_orbs_add_1_or_2_mods_by_the_rule_and_always_take_effect() {
    let mut rng = SeededRng::new(SEED);
    let iron = ring(IRON_RING_ID);
    let old_prefix = prefix("Old Prefix", "Old Prefix Family", 1000);
    let old_suffix = suffix("of the Old Suffix", "Old Suffix Family", 1000);

    // --- The orb takes effect also when it adds no mod ---
    let empty: Vec<ExplicitModDefinition> = Vec::new();
    let above_level_1 = vec![
        test_mod(
            "High Prefix",
            AffixType::Prefix,
            "High Prefix Family",
            1000,
            2,
            &["test"],
        ),
        test_mod(
            "of Height",
            AffixType::Suffix,
            "High Suffix Family",
            1000,
            2,
            &["test"],
        ),
    ];
    for (label, pool, item_level) in [
        ("an empty pool", &empty, DEFAULT_ITEM_LEVEL),
        ("only mods above the item level", &above_level_1, 1),
    ] {
        for trial in 0..ITEMS {
            let ctx = format!("{label}, item {trial}");
            // Given a normal Iron Ring without a valid mod
            let mut item = Item::new(&iron, &mut rng);
            item.item_level = item_level;
            let normal = item.clone();

            // When the Orb of Transmutation is applied
            let result = apply_orb_of_transmutation(&mut item, pool, &mut rng);
            // Then it takes effect: the ring becomes magic with no mods at all
            assert_eq!(result, Ok(()), "{ctx}: Orb of Transmutation");
            assert_eq!(item.rarity, Rarity::Magic, "{ctx}: the ring is magic");
            assert!(
                item.prefixes.is_empty() && item.suffixes.is_empty(),
                "{ctx}: the ring has no mods, found {:?}",
                outcome(&item)
            );
            assert_eq!(item.base, normal.base, "{ctx}: base");
            assert_eq!(item.implicits, normal.implicits, "{ctx}: implicits");
            assert_eq!(item.item_level, item_level, "{ctx}: item level");

            // When the Orb of Alteration is applied to this magic ring without mods
            let result = apply_orb_of_alteration(&mut item, pool, &mut rng);
            // Then it takes effect too, and the ring stays magic without mods
            assert_eq!(
                result,
                Ok(()),
                "{ctx}: Orb of Alteration on a ring without mods"
            );
            assert_eq!(item.rarity, Rarity::Magic, "{ctx}: the ring stays magic");
            assert!(
                item.prefixes.is_empty() && item.suffixes.is_empty(),
                "{ctx}: the ring has no mods, found {:?}",
                outcome(&item)
            );

            // Given a magic Iron Ring with an old prefix and an old suffix
            let mut altered = item_with(
                &iron,
                Rarity::Magic,
                &[&old_prefix],
                &[&old_suffix],
                &mut rng,
            );
            altered.item_level = item_level;
            let implicits = altered.implicits.clone();
            // When the Orb of Alteration is applied
            let result = apply_orb_of_alteration(&mut altered, pool, &mut rng);
            // Then it takes effect: the old mods are removed, no mod is added, and the ring
            // stays magic
            assert_eq!(
                result,
                Ok(()),
                "{ctx}: Orb of Alteration on a ring with mods"
            );
            assert_eq!(altered.rarity, Rarity::Magic, "{ctx}: the ring stays magic");
            assert!(
                altered.prefixes.is_empty() && altered.suffixes.is_empty(),
                "{ctx}: the old mods are removed and none is added, found {:?}",
                outcome(&altered)
            );
            assert_eq!(altered.implicits, implicits, "{ctx}: implicits");
        }
    }

    // --- These orbs never show "Failed to apply item: Could not generate Mod." ---
    // Given both orbs in crafting-core and in the game data
    let data: Value = serde_json::from_str(&read_text(CURRENCIES_JSON))
        .unwrap_or_else(|e| panic!("{CURRENCIES_JSON} is not valid JSON: {e}"));
    let entries = data
        .get("currencies")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("{CURRENCIES_JSON} must have an array \"currencies\""));
    for (currency, id, not_applicable) in [
        (
            Currency::OrbOfTransmutation,
            TRANSMUTATION_ID,
            CraftError::ItemNotNormal,
        ),
        (
            Currency::OrbOfAlteration,
            ALTERATION_ID,
            CraftError::ItemNotMagic,
        ),
    ] {
        assert_eq!(currency.id(), id);
        // When reading their messages
        let definition = currencies()
            .into_iter()
            .find(|definition| definition.id == id)
            .unwrap_or_else(|| panic!("currencies() must contain {id:?}"));
        let entry = entries
            .iter()
            .find(|entry| entry.get("id").and_then(Value::as_str) == Some(id))
            .unwrap_or_else(|| panic!("{CURRENCIES_JSON} must contain the currency {id:?}"));
        let file_messages = entry
            .get("messages")
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("{CURRENCIES_JSON}: {id:?} must have \"messages\""));
        // Then none of them is that message, so the orb cannot show it
        assert!(
            definition
                .messages
                .values()
                .all(|message| message.as_str() != NO_VALID_MOD_MESSAGE),
            "currencies(): {id:?} must have no message {NO_VALID_MOD_MESSAGE:?}, found {:?}",
            definition.messages
        );
        assert!(
            file_messages
                .values()
                .all(|message| message.as_str() != Some(NO_VALID_MOD_MESSAGE)),
            "{CURRENCIES_JSON}: {id:?} must have no message {NO_VALID_MOD_MESSAGE:?}, found \
             {file_messages:?}"
        );
        // And the message for the item they do not apply to is another one
        assert_ne!(
            currency.failure_message(not_applicable),
            NO_VALID_MOD_MESSAGE,
            "{id:?}: message for {not_applicable:?}"
        );
    }

    // --- 1 or 2 mods, each with a 50 % chance, added one after the other by the rule ---
    // Given a pool in which the prefix and one suffix share a family. The rule gives "Shared
    // Prefix" alone 12.5 %, "of the Shared Family" alone 25 %, "of Another Family" alone 25 %,
    // and "Shared Prefix" with "of Another Family" 37.5 %; never the prefix with the suffix of
    // its own family, which the old rule 3 gave in 16.7 % of the results.
    let rule_pool = vec![
        prefix("Shared Prefix", "Shared Family", 1000),
        suffix("of the Shared Family", "Shared Family", 1000),
        suffix("of Another Family", "Another Family", 2000),
    ];
    let expected = orb_outcomes(&rule_pool);
    let mut orb_rng = SeededRng::new(ORB_SEED);
    let mut transmuted: BTreeMap<Vec<String>, u64> = BTreeMap::new();
    let mut altered: BTreeMap<Vec<String>, u64> = BTreeMap::new();
    let mut ring_in_a_row = item_with(
        &iron,
        Rarity::Magic,
        &[&old_prefix],
        &[&old_suffix],
        &mut orb_rng,
    );
    for trial in 0..TRIALS {
        // When the Orb of Transmutation is applied to a new ring, and the Orb of Alteration
        // again and again to the same ring
        let mut item = Item::new(&iron, &mut orb_rng);
        apply_orb_of_transmutation(&mut item, &rule_pool, &mut orb_rng)
            .unwrap_or_else(|e| panic!("trial {trial}: transmutation failed with {e:?}"));
        apply_orb_of_alteration(&mut ring_in_a_row, &rule_pool, &mut orb_rng)
            .unwrap_or_else(|e| panic!("trial {trial}: alteration failed with {e:?}"));
        for (orb, result, counts) in [
            ("Orb of Transmutation", &item, &mut transmuted),
            ("Orb of Alteration", &ring_in_a_row, &mut altered),
        ] {
            let ctx = format!("{orb}, trial {trial}");
            assert_eq!(result.rarity, Rarity::Magic, "{ctx}: magic");
            for explicit in explicits(result) {
                assert_rolled_from(find(&rule_pool, &explicit.name), explicit, &ctx);
            }
            *counts.entry(outcome(result)).or_default() += 1;
        }
    }
    for (orb, counts) in [
        ("Orb of Transmutation", &transmuted),
        ("Orb of Alteration", &altered),
    ] {
        // Then every result is one the rule can give
        for (result, count) in counts {
            assert!(
                expected.contains_key(result),
                "{orb}: {count} results {result:?}, which the rule cannot give"
            );
        }
        // And each result occurs with the chance the rule gives
        for (result, chance) in &expected {
            assert_share(
                &format!("{orb}: {result:?}"),
                counts.get(result).copied().unwrap_or(0),
                TRIALS,
                *chance,
            );
        }
    }
}

// Criterion 4: „In crafting-core gibt es genau eine Umsetzung von Kriterium 2. Orb of
// Transmutation und Orb of Alteration verwenden sie; die Webseite baut den Pool nicht selbst."
// (Tech lead's plan and ADR 0021: `valid_pool` is the only implementation of step 1,
// `add_random_mod` builds it and adds the mod; both orbs take the base's pool and filter it
// themselves, so an already filtered pool gives the same result; the page passes the pool from
// `mod_pool` and calls neither `valid_mods` nor `unblocked_mods`. ADR 0004: lean text checks for
// the structure.)
#[test]
fn ac4_one_implementation_in_crafting_core_used_by_both_orbs_and_not_by_the_page() {
    // --- Both orbs build the valid pool from the base's pool themselves ---
    // Given every ring base at the item levels 100 and 40, with its whole pool and the same pool
    // already filtered by the item level and the lock
    let bases = Slot::Ring.base_items();
    assert_eq!(bases.len(), RING_BASES, "ring bases");
    for base in &bases {
        let whole = pool_of(&base.id);
        for item_level in [UI_ITEM_LEVEL, 40] {
            let filtered = valid_mods(&unblocked_mods(&base.id, &whole), item_level);
            for seed in 0..EQUAL_SEEDS {
                // When both orbs are applied with the same seed, once with each pool
                let craft = |pool: &[ExplicitModDefinition]| {
                    let mut rng = SeededRng::new(seed);
                    let mut item = Item::new(base, &mut rng);
                    item.item_level = item_level;
                    apply_orb_of_transmutation(&mut item, pool, &mut rng)
                        .unwrap_or_else(|e| panic!("{}: transmutation failed with {e:?}", base.id));
                    let transmuted = item.clone();
                    apply_orb_of_alteration(&mut item, pool, &mut rng)
                        .unwrap_or_else(|e| panic!("{}: alteration failed with {e:?}", base.id));
                    (transmuted, item)
                };
                // Then the results are the same
                assert_eq!(
                    craft(&whole),
                    craft(&filtered),
                    "{} {} at item level {item_level}, seed {seed}: the whole pool and the \
                     filtered pool",
                    base.name,
                    base.id
                );
            }
        }
    }

    // --- The orbs take the item level from the item and the lock from the base ---
    // Given Iron Rings of item level 40 and Cryonic Rings of item level 100, with their whole
    // pools
    let mut rng = SeededRng::new(SEED);
    for (id, item_level) in [(IRON_RING_ID, 40), (CRYONIC_RING_ID, UI_ITEM_LEVEL)] {
        let base = ring(id);
        let whole = pool_of(id);
        let allowed: BTreeSet<String> = names(&valid_mods(&unblocked_mods(id, &whole), item_level))
            .into_iter()
            .collect();
        assert!(
            allowed.len() < whole.len(),
            "{id}: the item level or the lock must leave out mods of the pool"
        );
        let mut altered = Item::new(&base, &mut rng);
        altered.item_level = item_level;
        apply_orb_of_transmutation(&mut altered, &whole, &mut rng).expect("the orb applies");
        for trial in 0..RING_ITEMS {
            let mut transmuted = Item::new(&base, &mut rng);
            transmuted.item_level = item_level;
            // When both orbs are applied
            apply_orb_of_transmutation(&mut transmuted, &whole, &mut rng)
                .unwrap_or_else(|e| panic!("{id}: transmutation failed with {e:?}"));
            apply_orb_of_alteration(&mut altered, &whole, &mut rng)
                .unwrap_or_else(|e| panic!("{id}: alteration failed with {e:?}"));
            for (orb, item) in [
                ("Orb of Transmutation", &transmuted),
                ("Orb of Alteration", &altered),
            ] {
                let ctx = format!("{} at item level {item_level}, {orb}, {trial}", base.name);
                // Then the ring keeps its item level and gets 1 or 2 mods within the limits,
                // of different families, each allowed at the item level and by the lock
                assert_eq!(item.item_level, item_level, "{ctx}: item level");
                assert!(
                    matches!(
                        (item.prefixes.len(), item.suffixes.len()),
                        (1, 0) | (0, 1) | (1, 1)
                    ),
                    "{ctx}: 1 or 2 mods, at most 1 prefix and 1 suffix, found {:?}",
                    outcome(item)
                );
                assert_different_families(item, &ctx);
                for explicit in explicits(item) {
                    assert!(
                        allowed.contains(&explicit.name),
                        "{ctx}: {:?} is above the item level or blocked by the lock",
                        explicit.name
                    );
                    assert_rolled_from(find(&whole, &explicit.name), explicit, &ctx);
                }
            }
        }
    }

    // --- crafting-core has exactly one implementation of the rule ---
    // Given the sources of crafting-core
    let core = core_src_files();
    // Then they define `valid_pool` and `add_random_mod` once each
    for name in ["valid_pool", "add_random_mod"] {
        let count = definitions_of(&core, name);
        assert_eq!(
            count, 1,
            "crafting-core/src/ must define `{name}` exactly once, found {count}"
        );
    }

    // --- The page does not build the pool itself ---
    // Given the sources of the page
    let web = web_src_files();
    // Then they use neither `valid_mods` nor `unblocked_mods`, not even in comments or tests
    for name in ["valid_mods", "unblocked_mods"] {
        let found = files_containing(&web, name);
        assert!(
            found.is_empty(),
            "web/src/ must not build the pool with `{name}`: {found:#?}"
        );
    }
    // And they pass the base's pool from crafting-core to both orbs
    for needle in [
        "mod_pool",
        "apply_orb_of_transmutation",
        "apply_orb_of_alteration",
    ] {
        assert!(
            !files_containing(&web, needle).is_empty(),
            "a file under web/src/ must use `{needle}`: the page passes the base's pool to \
             the orbs"
        );
    }
}

// Criterion 5: „Ein Item kann bis zu so viele Prefixe und Suffixe halten, wie die Grenzen aus
// Kriterium 1 erlauben."
// (Tech lead's plan: `prefixes` and `suffixes` of `Item`, in the order the mods were added.
// Here: magic and rare items filled with `add_random_mod` until no mod is added, with invented
// pools and with the Iron Ring's pool; the level requirement counts every mod the item holds.)
#[test]
fn ac5_an_item_holds_as_many_prefixes_and_suffixes_as_the_limits_allow() {
    let mut rng = SeededRng::new(SEED);

    // --- Invented pools ---
    // Given a pool with four prefix families and four suffix families, and a pool with only
    // five prefix families
    let base = invented_base(Vec::new());
    let both_sides: Vec<ExplicitModDefinition> = (1..=4)
        .flat_map(|n| {
            [
                prefix(&format!("Prefix {n}"), &format!("Prefix Family {n}"), 1000),
                suffix(
                    &format!("of Suffix {n}"),
                    &format!("Suffix Family {n}"),
                    1000,
                ),
            ]
        })
        .collect();
    let only_prefixes: Vec<ExplicitModDefinition> = (1..=5)
        .map(|n| {
            prefix(
                &format!("Lone Prefix {n}"),
                &format!("Lone Prefix Family {n}"),
                1000,
            )
        })
        .collect();
    for (label, rarity, pool, expected) in [
        ("magic, both sides", Rarity::Magic, &both_sides, (1, 1)),
        ("rare, both sides", Rarity::Rare, &both_sides, (3, 3)),
        (
            "magic, only prefixes",
            Rarity::Magic,
            &only_prefixes,
            (1, 0),
        ),
        ("rare, only prefixes", Rarity::Rare, &only_prefixes, (3, 0)),
    ] {
        for trial in 0..FILLED_ITEMS {
            let ctx = format!("{label}, item {trial}");
            // Given an item of the rarity without prefixes and suffixes
            let mut item = item_with(&base, rarity, &[], &[], &mut rng);
            // When random mods are added until no mod is added
            let mut added = 0;
            loop {
                let before = item.clone();
                if !add_random_mod(&mut item, pool, &mut rng) {
                    assert_eq!(item, before, "{ctx}: no mod added, the item is unchanged");
                    break;
                }
                let explicit = assert_one_mod_added(&before, &item, &ctx);
                assert_rolled_from(find(pool, &explicit.name), &explicit, &ctx);
                added += 1;
                assert!(
                    added <= expected.0 + expected.1,
                    "{ctx}: more mods than the limits allow: {:?}",
                    outcome(&item)
                );
            }
            // Then the item holds as many prefixes and suffixes as the limits allow
            assert_eq!(
                (item.prefixes.len(), item.suffixes.len()),
                expected,
                "{ctx}: prefixes and suffixes, found {:?}",
                outcome(&item)
            );
            assert!(
                item.prefixes
                    .iter()
                    .all(|explicit| explicit.affix_type == AffixType::Prefix)
                    && item
                        .suffixes
                        .iter()
                        .all(|explicit| explicit.affix_type == AffixType::Suffix),
                "{ctx}: every mod sits on its own side"
            );
            assert_different_families(&item, &ctx);
        }
    }

    // --- The game data: rare Iron Rings ---
    // Given rare Iron Rings and the Iron Ring's pool
    let iron = ring(IRON_RING_ID);
    let iron_pool = pool_of(IRON_RING_ID);
    for trial in 0..FILLED_ITEMS {
        let ctx = format!("rare Iron Ring {trial}");
        let mut item = item_with(&iron, Rarity::Rare, &[], &[], &mut rng);
        // When random mods are added until no mod is added
        while add_random_mod(&mut item, &iron_pool, &mut rng) {
            assert!(
                explicits(&item).len() <= RARE_LIMITS.0,
                "{ctx}: more mods than the limits allow: {:?}",
                outcome(&item)
            );
        }
        // Then it holds 3 prefixes and 3 suffixes of different families from the pool
        assert_eq!(
            (item.prefixes.len(), item.suffixes.len()),
            (RARE_LIMITS.1, RARE_LIMITS.2),
            "{ctx}: found {:?}",
            outcome(&item)
        );
        assert_different_families(&item, &ctx);
        for explicit in explicits(&item) {
            assert_rolled_from(find(&iron_pool, &explicit.name), explicit, &ctx);
        }
        // And its level requirement counts all of its mods
        assert_eq!(
            item.required_level(),
            expected_required_level(&item),
            "{ctx}: level requirement of {:?}",
            outcome(&item)
        );
    }
}

// Criterion 6: „Auf der Seite bleibt das Verhalten der heutigen Ringe gleich: 1 oder 2 Mods je
// 50 %, bei einem Mod Wahl nach Gewicht aus allen Prefixen und Suffixen, bei zwei Mods ein
// Prefix und ein Suffix."
// (Tech lead's plan: the page passes the base's pool from `mod_pool`; the mods that can roll on
// a ring are its pool without the mods its lock blocks, at the item level 100, as the unchanged
// `valid_mods` and `unblocked_mods` give them. Here: all 37 rings with both orbs. The page itself
// is checked by the PO in the local preview.)
// (Adapted for AB#189, PO-confirmed: the 37 rings are the ring bases without the six Heist rings
// that AB#189 adds. Their limit changes replace this behaviour for them, AB#189 criterion 3.)
#[test]
fn ac6_todays_rings_roll_as_before_with_the_pool_the_page_passes() {
    let mut rng = SeededRng::new(RING_SEED);
    let bases = todays_rings();
    let (mut results, mut one_mod, mut one_mod_prefixes) = (0_u64, 0_u64, 0_u64);
    let (mut expected_prefixes, mut variance) = (0.0_f64, 0.0_f64);

    for base in &bases {
        let what = format!("{} {}", base.name, base.id);
        // Given the ring's pool as the page passes it, and the mods that can roll on the ring
        let whole = pool_of(&base.id);
        let rollable = valid_mods(&unblocked_mods(&base.id, &whole), UI_ITEM_LEVEL);
        let by_name: BTreeMap<&str, &ExplicitModDefinition> = rollable
            .iter()
            .map(|definition| (definition.name.as_str(), definition))
            .collect();
        let weight = |affix_type: AffixType| -> f64 {
            rollable
                .iter()
                .filter(|definition| definition.affix_type == affix_type)
                .map(|definition| f64::from(definition.weight))
                .sum()
        };
        let (prefix_weight, suffix_weight) = (weight(AffixType::Prefix), weight(AffixType::Suffix));
        assert!(
            prefix_weight > 0.0 && suffix_weight > 0.0,
            "{what}: the ring must have prefixes and suffixes"
        );
        let prefix_families: BTreeSet<&str> = rollable
            .iter()
            .filter(|definition| definition.affix_type == AffixType::Prefix)
            .map(|definition| definition.definition.family.as_str())
            .collect();
        assert!(
            rollable
                .iter()
                .filter(|definition| definition.affix_type == AffixType::Suffix)
                .all(|definition| !prefix_families.contains(definition.definition.family.as_str())),
            "{what}: the test expects no family with prefixes and suffixes (ADR 0021)"
        );
        let prefix_share = prefix_weight / (prefix_weight + suffix_weight);

        let mut base_one_mod = 0_u64;
        let mut base_one_mod_prefixes = 0_u64;
        let mut altered = Item::new(base, &mut rng);
        apply_orb_of_transmutation(&mut altered, &whole, &mut rng).expect("the orb applies");
        for trial in 0..ORBS_PER_BASE {
            // When the Orb of Transmutation is applied to a new ring, and the Orb of
            // Alteration again and again to the same ring
            let mut transmuted = Item::new(base, &mut rng);
            apply_orb_of_transmutation(&mut transmuted, &whole, &mut rng)
                .unwrap_or_else(|e| panic!("{what}: transmutation failed with {e:?}"));
            apply_orb_of_alteration(&mut altered, &whole, &mut rng)
                .unwrap_or_else(|e| panic!("{what}: alteration failed with {e:?}"));
            for (orb, item) in [
                ("Orb of Transmutation", &transmuted),
                ("Orb of Alteration", &altered),
            ] {
                let ctx = format!("{what}, {orb}, {trial}");
                // Then the ring is magic with 1 or 2 mods; two mods are a prefix and a suffix
                assert_eq!(item.rarity, Rarity::Magic, "{ctx}: magic");
                assert!(
                    matches!(
                        (item.prefixes.len(), item.suffixes.len()),
                        (1, 0) | (0, 1) | (1, 1)
                    ),
                    "{ctx}: 1 mod, or a prefix and a suffix, found {:?}",
                    outcome(item)
                );
                // And every mod is one that can roll on the ring
                for explicit in explicits(item) {
                    let definition = by_name.get(explicit.name.as_str()).unwrap_or_else(|| {
                        panic!("{ctx}: {:?} cannot roll on this ring", explicit.name)
                    });
                    assert_rolled_from(definition, explicit, &ctx);
                }
                if item.prefixes.len() + item.suffixes.len() == 1 {
                    base_one_mod += 1;
                    if item.prefixes.len() == 1 {
                        base_one_mod_prefixes += 1;
                    }
                }
            }
        }

        let base_results = 2 * ORBS_PER_BASE;
        // And 1 and 2 mods each occur about half the time
        assert_share(
            &format!("{what}: results with one mod"),
            base_one_mod,
            base_results,
            0.5,
        );
        // And with one mod, it is chosen by weight from all prefixes and suffixes together
        assert_share(
            &format!("{what}: prefixes among results with one mod"),
            base_one_mod_prefixes,
            base_one_mod,
            prefix_share,
        );
        results += base_results;
        one_mod += base_one_mod;
        one_mod_prefixes += base_one_mod_prefixes;
        expected_prefixes += base_one_mod as f64 * prefix_share;
        variance += base_one_mod as f64 * prefix_share * (1.0 - prefix_share);
    }

    // And over all rings together, the shares hold as well
    assert_share("all rings: results with one mod", one_mod, results, 0.5);
    let tolerance = 5.0 * variance.sqrt();
    assert!(
        (one_mod_prefixes as f64 - expected_prefixes).abs() <= tolerance,
        "all rings: {one_mod_prefixes} prefixes among {one_mod} results with one mod, expected \
         {expected_prefixes:.1} \u{b1} {tolerance:.1}"
    );
}
