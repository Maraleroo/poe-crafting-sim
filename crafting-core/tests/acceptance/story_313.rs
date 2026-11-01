//! Acceptance tests for AB#313 "Chaos Orb auf seltene Items anwenden".
//!
//! The tests use the public interface of `crafting-core` from the tech lead's plan on AB#313:
//! - `Currency::ChaosOrb` with the id `chaos_orb`; its name, description and message are only in
//!   `data/currencies.json`, which has only the message `item_not_rare` for it.
//! - `CraftError::ItemNotRare` with the code `item_not_rare` (unchanged).
//! - `can_apply_chaos_orb(item) -> Result<(), CraftError>`: checks only the rarity. `ItemNotRare`
//!   for a normal, magic or unique item, otherwise `Ok`, whatever the pool and the room.
//! - `apply_chaos_orb(item, pool, rng) -> Result<(), CraftError>`: first the same check; on an
//!   error the item is unchanged. Otherwise all prefixes and suffixes are removed and random mods
//!   are added by `add_random_mod` up to a target of 4, 5 or 6; always `Ok` on a rare item.
//! - unchanged: `currencies`, `Currency::{id, name, description, failure_message}`,
//!   `CraftError::code`, `add_random_mod`, `valid_pool`, `mod_pool`, `base_item`,
//!   `Slot::Ring.base_items()`, `Item::affix_limits`, `Mod::roll`, `SeededRng`. Following
//!   ADR 0004, web criteria are checked through the file contents under `web/` (without the
//!   build output in `web/dist` and `web/target`).
//!
//! The PO's rules are `poe-crafting-rules`: `currencies/chaos-orb.md` (committed in 5fdab15,
//! before these tests), "Affix limits" in "Item basics", "Item level" and "Adding a random mod".
//!
//! Criterion 1 gives the PO's wording for `poe-crafting-rules`. It is already met by 5fdab15 and
//! its wording is checked byte for byte at the acceptance (step 2 of the `story` skill), so it
//! has no test here. The behaviour of the rule is checked by the tests of criteria 3 and 5.
//!
//! Game texts (the orb's name, its description and its message) are not compared word for word
//! here (`writing-acceptance-tests`, "What permanent tests check"): their wording is checked at
//! the acceptance. The tests check that crafting-core shows the texts of the orb's entry in
//! `data/currencies.json`, and that the texts are not written into the code of `crafting-core`
//! or `web`.
//!
//! Every random test uses a fixed seed. Statistical tests compare observed shares with the
//! shares that follow from the weights and the rule, with a tolerance of 5 standard deviations.
//! The tests depend only on frequencies and invariants, never on the order in which the
//! generator is used.
//!
//! Red state before the implementation: the module does not compile, because
//! `Currency::ChaosOrb`, `can_apply_chaos_orb` and `apply_chaos_orb` do not exist yet.
//!
//! What is checked outside `cargo test`, by the PO in the local preview:
//! - AC2: the sixth button next to the other orbs and its tooltip.
//! - AC4: how the inapplicable orb looks, its tooltip and the message after a click.
//! - AC5: that no message appears after a click.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crafting_core::{
    add_random_mod, apply_chaos_orb, base_item, can_apply_chaos_orb, currencies, mod_pool,
    valid_pool, AffixLimits, AffixType, BaseItem, CraftError, Currency, CurrencyDefinition,
    ExplicitMod, ExplicitModDefinition, Item, Mod, ModDefinition, Rarity, SeededRng, Slot,
    StatDefinition, DEFAULT_ITEM_LEVEL,
};
use serde_json::Value;

// ---------------------------------------------------------------------------------------
// Values from the story, the plan and the PO's rules
// ---------------------------------------------------------------------------------------

const IRON_RING_ID: &str = "Metadata/Items/Rings/Ring1";
/// "+3 Prefix Modifiers allowed", "-3 Suffix Modifiers allowed" (AB#189): as a rare item at most
/// 6 prefixes and no suffix.
const COMPOSITE_RING_ID: &str = "Metadata/Items/Rings/RingE3";
/// "-1 Prefix Modifier allowed", "+1 Suffix Modifier allowed" (AB#189): as a rare item at most
/// 2 prefixes and 4 suffixes.
const COGWORK_RING_ID: &str = "Metadata/Items/Rings/RingE1";

const ORB_ID: &str = "chaos_orb";
/// The orbs the page already has (criterion 2: "Neben den bisherigen Orbs").
const EARLIER_ORB_IDS: [&str; 5] = [
    "orb_of_transmutation",
    "orb_of_alteration",
    "orb_of_augmentation",
    "regal_orb",
    "exalted_orb",
];
/// The Exalted Orb has the game's messages for a rare item without room and for an empty valid
/// pool; the Chaos Orb never shows them (the PO's rule).
const EXALTED_ID: &str = "exalted_orb";

/// Reason codes from the tech lead's plan and ADR 0012, the keys of `messages`.
const NOT_RARE_CODE: &str = "item_not_rare";
const NO_SPACE_CODE: &str = "no_space_for_mods";
const NO_VALID_MOD_CODE: &str = "no_valid_mod";

/// The PO's rule: the orb chooses a target number of mods, 4, 5 or 6, with chances 8 : 3 : 1.
const TARGETS: [(usize, u32); 3] = [(4, 8), (5, 3), (6, 1)];

/// Criterion 3: "Beispiel: seltene Iron Rings auf Item-Level 100".
const EXAMPLE_ITEM_LEVEL: u32 = 100;
/// Item level of the items with invented pools; a mod of this level can roll, one above not.
const INVENTED_ITEM_LEVEL: u32 = 50;
/// Criterion 5: on item level 1 the Composite Ring's base has only 4 prefix families. On item
/// level 2 it has 5 (the PO's rule for AB#306), so there a target of 5 or 6 stops at 5.
const SMALL_POOL_LEVELS: [(u32, usize); 2] = [(1, 4), (2, 5)];

const CURRENCIES_JSON: &str = "data/currencies.json";

/// Fixed seeds, so every run is identical.
const SEED: u64 = 313;
const STATISTICS_SEED: u64 = 31_301;
const REFERENCE_SEED: u64 = 31_302;
const TEST_POOL_SEED: u64 = 31_303;

/// Items per ring and rarity for the checks of applicability (AC2, AC4) and of the removed mods
/// (AC3).
const ITEMS: u64 = 100;
/// Chaos Orbs in a row on the story's rare Iron Ring of item level 100 (AC3). 5 standard
/// deviations of the share of 6 mods (1/12) are 0.025, of 4 mods (8/12) 0.043: a uniform choice
/// of the target (1/3 each), a fixed target, swapped chances (6 mods 8/12) or an orb that does
/// not remove the old mods first (always 6 mods) fail clearly. Each orb builds the valid pool of
/// about 230 mods up to six times, so more orbs would take longer than a few seconds.
const IRON_TRIALS: u64 = 3_000;
/// Rare Iron Rings filled by `add_random_mod` to 6 mods, as the reference for the prefix/suffix
/// split on the game data (AC3).
const REFERENCE_TRIALS: u64 = 3_000;
/// Chaos Orbs in a row per invented pool (AC3). The rule's exact chances are computed for every
/// result. The prefixes weigh four times as much as the suffixes, so a coin flip between the
/// sides gives other splits than the weights: on the Iron Ring 1 prefix and 3 suffixes 0.208
/// instead of 0.028, 3 prefixes and 1 suffix 0.208 instead of 0.348; on the Cogwork Ring 1 prefix
/// and 3 suffixes 0.167 instead of 0.028. 5 standard deviations of a share are at most 0.025, so
/// it fails clearly.
const TEST_POOL_TRIALS: u64 = 10_000;
/// Rare rings per ring base, with 0 to 6 mods (AC3): every number twice. The result's checks
/// build the valid pool, so more rings would take longer than a few seconds.
const ITEMS_PER_RING: u64 = 14;
/// Chaos Orbs per item level on the Composite Ring of criterion 5. On item level 2, 5 standard
/// deviations of the share of 4 prefixes (8/12) are 0.053; an orb that stops at 4 or always fills
/// 5 fails clearly.
const SMALL_POOL_TRIALS: u64 = 2_000;

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

/// Asserts that some file under `web/src/` contains each of the needles.
fn assert_web_src_uses(needles: &[&str], why: &str) {
    let src = web_src_files();
    for needle in needles {
        assert!(
            !files_containing(&src, needle).is_empty(),
            "a file under web/src/ must use `{needle}`: {why}"
        );
    }
}

/// Asserts that some line under `web/src/` uses `apply_chaos_orb` other than as part of
/// `can_apply_chaos_orb`: the page applies the orb on a click, not only asks whether it applies.
fn assert_web_src_applies_the_orb() {
    let applies = web_src_files().iter().any(|path| {
        read_path(path).lines().any(|line| {
            line.replace("can_apply_chaos_orb", "")
                .contains("apply_chaos_orb")
        })
    });
    assert!(
        applies,
        "a file under web/src/ must call `apply_chaos_orb`: a click applies the orb"
    );
}

/// Asserts that a game text lives only in the game data: not in the code of `crafting-core`
/// and not in any file under `web/` (ADR 0011, ADR 0012).
fn assert_only_in_game_data(text: &str) {
    let in_code = [
        files_containing(&core_src_files(), text),
        files_containing(&web_files(), text),
    ]
    .concat();
    assert!(
        in_code.is_empty(),
        "{text:?} must only be in {CURRENCIES_JSON}, also found in: {in_code:#?}"
    );
}

// ---------------------------------------------------------------------------------------
// Helpers: game data
// ---------------------------------------------------------------------------------------

/// The entry of the currency `id` in `data/currencies.json`.
fn entry_in_file(id: &str) -> Value {
    let content = read_text(CURRENCIES_JSON);
    let data: Value = serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("{CURRENCIES_JSON} is not valid JSON: {e}"));
    data.get("currencies")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("{CURRENCIES_JSON} must have an array \"currencies\""))
        .iter()
        .find(|entry| entry.get("id").and_then(Value::as_str) == Some(id))
        .unwrap_or_else(|| panic!("{CURRENCIES_JSON} must contain the currency {id:?}"))
        .clone()
}

/// The text of the currency `id`'s entry in `data/currencies.json` at the given path of keys. A
/// game text is never empty.
fn text_in_file(id: &str, path: &[&str]) -> String {
    let entry = entry_in_file(id);
    let mut value = &entry;
    for key in path {
        value = value.get(key).unwrap_or_else(|| {
            panic!("{CURRENCIES_JSON}: the entry {id:?} must have {path:?}, found {entry}")
        });
    }
    let text = value
        .as_str()
        .unwrap_or_else(|| panic!("{CURRENCIES_JSON}: {id:?} {path:?} must be a string"))
        .to_owned();
    assert!(
        !text.trim().is_empty(),
        "{CURRENCIES_JSON}: {id:?} {path:?} must be a game text, found {text:?}"
    );
    text
}

/// The entry of the currency `id` in `currencies()`.
fn definition(id: &str) -> CurrencyDefinition {
    currencies()
        .into_iter()
        .find(|entry| entry.id == id)
        .unwrap_or_else(|| panic!("currencies() must contain {id:?}"))
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

/// An invented mod with one stat whose range is real, so values are rolled.
fn test_mod(
    name: &str,
    affix_type: AffixType,
    family: &str,
    weight: u32,
    level: u32,
) -> ExplicitModDefinition {
    let stat_id = format!("{name} stat");
    ExplicitModDefinition {
        name: name.to_owned(),
        affix_type,
        weight,
        definition: ModDefinition {
            family: family.to_owned(),
            level,
            tags: vec!["test".to_owned()],
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

fn prefix(name: &str, family: &str, weight: u32, level: u32) -> ExplicitModDefinition {
    test_mod(name, AffixType::Prefix, family, weight, level)
}

fn suffix(name: &str, family: &str, weight: u32, level: u32) -> ExplicitModDefinition {
    test_mod(name, AffixType::Suffix, family, weight, level)
}

/// Invented old mods of `count` different families on one side, which are in no pool.
fn old_mods(affix_type: AffixType, count: usize) -> Vec<ExplicitModDefinition> {
    (0..count)
        .map(|n| {
            test_mod(
                &format!("Old {affix_type:?} {n}"),
                affix_type,
                &format!("Old {affix_type:?} family {n}"),
                1000,
                1,
            )
        })
        .collect()
}

fn rolled(definition: &ExplicitModDefinition, rng: &mut SeededRng) -> ExplicitMod {
    ExplicitMod {
        name: definition.name.clone(),
        affix_type: definition.affix_type,
        modifier: Mod::roll(&definition.definition, rng),
    }
}

/// A new item of `base` at `item_level` with the given rarity, prefixes and suffixes, rolled
/// from the definitions, in this order.
fn item_with(
    base: &BaseItem,
    rarity: Rarity,
    item_level: u32,
    prefixes: &[&ExplicitModDefinition],
    suffixes: &[&ExplicitModDefinition],
    rng: &mut SeededRng,
) -> Item {
    let mut item = Item::new(base, rng);
    item.item_level = item_level;
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

/// An item of `base` with the given rarity at `item_level` with `mods` random mods of `pool`, or
/// as many as the rule adds.
fn item_of_rarity(
    base: &BaseItem,
    rarity: Rarity,
    item_level: u32,
    pool: &[ExplicitModDefinition],
    mods: usize,
    rng: &mut SeededRng,
) -> Item {
    let mut item = item_with(base, rarity, item_level, &[], &[], rng);
    while explicits(&item).len() < mods && add_random_mod(&mut item, pool, rng) {}
    item
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

/// The names of the item's prefixes and suffixes, sorted: the result of an orb, whatever the
/// order in which the mods were added.
fn sorted_outcome(item: &Item) -> Vec<String> {
    let mut names = outcome(item);
    names.sort();
    names
}

/// The number of prefixes and suffixes of an item.
fn split(item: &Item) -> (usize, usize) {
    (item.prefixes.len(), item.suffixes.len())
}

fn limits(item: &Item) -> AffixLimits {
    item.affix_limits()
        .unwrap_or_else(|| panic!("a {:?} item has affix limits", item.rarity))
}

/// The chance of a target of `mods` mods (the PO's rule: 8 : 3 : 1).
fn target_chance(mods: usize) -> f64 {
    let total: u32 = TARGETS.iter().map(|(_, weight)| weight).sum();
    TARGETS
        .iter()
        .find(|(target, _)| *target == mods)
        .map_or(0.0, |(_, weight)| f64::from(*weight) / f64::from(total))
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

/// Asserts that `after` is a result the rule allows for the rare item `before` reforged with
/// `pool`: still rare, with the same base, item level and implicits (with their values); every
/// prefix and suffix on its own side, under both affix limits of a rare item, of different
/// families, and rolled from a mod of `pool` that can roll on the item (item level, Breach lock,
/// side with room on the empty item); and 4, 5 or 6 mods, or fewer only if no mod can be added
/// any more.
fn assert_reforged(before: &Item, after: &Item, pool: &[ExplicitModDefinition], ctx: &str) {
    let ctx = format!("{ctx}, result {:?}", outcome(after));
    assert_eq!(before.rarity, Rarity::Rare, "{ctx}: the test starts rare");
    assert_eq!(after.rarity, Rarity::Rare, "{ctx}: the item stays rare");
    assert_eq!(after.base, before.base, "{ctx}: the base stays");
    assert_eq!(
        after.item_level, before.item_level,
        "{ctx}: the item level stays"
    );
    assert_eq!(
        after.implicits, before.implicits,
        "{ctx}: the implicits stay unchanged with their values"
    );
    for (side, mods) in [
        (AffixType::Prefix, &after.prefixes),
        (AffixType::Suffix, &after.suffixes),
    ] {
        for explicit in mods {
            assert_eq!(
                explicit.affix_type, side,
                "{ctx}: {:?} sits on its own side",
                explicit.name
            );
        }
    }
    let limits = limits(after);
    let mods = explicits(after);
    assert!(
        after.prefixes.len() <= limits.prefixes
            && after.suffixes.len() <= limits.suffixes
            && mods.len() <= limits.mods,
        "{ctx}: both affix limits of a rare item hold, limits {limits:?}"
    );
    let families: BTreeSet<&str> = mods
        .iter()
        .map(|explicit| explicit.modifier.family.as_str())
        .collect();
    assert_eq!(
        families.len(),
        mods.len(),
        "{ctx}: no two mods of the same family"
    );

    // Every mod can roll on this item: the valid pool of the item without prefixes and suffixes
    let mut cleared = after.clone();
    cleared.prefixes.clear();
    cleared.suffixes.clear();
    let rollable = valid_pool(&cleared, pool);
    for explicit in &mods {
        let definition = rollable
            .iter()
            .find(|definition| {
                definition.name == explicit.name
                    && definition.affix_type == explicit.affix_type
                    && definition.definition.family == explicit.modifier.family
                    && definition.definition.level == explicit.modifier.level
            })
            .unwrap_or_else(|| {
                panic!(
                    "{ctx}: {:?} is not a mod of the pool that can roll on this item",
                    explicit.name
                )
            });
        assert_rolled_from(definition, explicit, &ctx);
    }

    // 4 to 6 mods, or fewer only if the rule adds no more
    let smallest_target = TARGETS.iter().map(|(target, _)| *target).min().unwrap_or(0);
    if mods.len() < smallest_target {
        assert!(
            valid_pool(after, pool).is_empty(),
            "{ctx}: with {} mods the orb stops only when no mod can be added",
            mods.len()
        );
    }
}

/// Applies the orb to the rare `item` and checks the result: the orb applies on hover, takes
/// effect on click, and the result is one the rule allows. Returns the item before the orb.
fn reforge(
    item: &mut Item,
    pool: &[ExplicitModDefinition],
    rng: &mut SeededRng,
    ctx: &str,
) -> Item {
    let before = item.clone();
    assert_eq!(
        can_apply_chaos_orb(item),
        Ok(()),
        "{ctx}: the orb applies to a rare item, found {:?}",
        outcome(item)
    );
    assert_eq!(
        apply_chaos_orb(item, pool, rng),
        Ok(()),
        "{ctx}: the orb takes effect on a rare item, without a message"
    );
    assert_reforged(&before, item, pool, ctx);
    before
}

/// Asserts that hover and click both fail because the item is not rare, and that the click
/// leaves the item unchanged.
fn assert_not_applicable(
    item: &Item,
    pool: &[ExplicitModDefinition],
    rng: &mut SeededRng,
    ctx: &str,
) {
    // When the page asks whether the orb applies (hover)
    let hover = can_apply_chaos_orb(item);
    // And the orb is clicked anyway
    let mut clicked = item.clone();
    let click = apply_chaos_orb(&mut clicked, pool, rng);
    // Then both fail because the item is not rare
    assert_eq!(hover, Err(CraftError::ItemNotRare), "{ctx}: hover");
    assert_eq!(click, Err(CraftError::ItemNotRare), "{ctx}: click");
    // And the item is unchanged
    assert_eq!(&clicked, item, "{ctx}: the item is unchanged");
}

// ---------------------------------------------------------------------------------------
// Helpers: the PO's rules as expected chances
// ---------------------------------------------------------------------------------------

/// The exact chance of every result of "Adding a random mod", repeated until the item has
/// `target` prefixes and suffixes or no mod is added, starting from an item without prefixes and
/// suffixes. For invented pools on an item without a lock: step 1 leaves out mods above
/// `item_level`, mods whose side has no room under both `limits`, and mods of a family already
/// on the item. A result is the sorted names of its mods.
fn rule_outcomes(
    pool: &[ExplicitModDefinition],
    limits: &AffixLimits,
    item_level: u32,
    target: usize,
) -> BTreeMap<Vec<String>, f64> {
    fn step<'a>(
        pool: &'a [ExplicitModDefinition],
        limits: &AffixLimits,
        item_level: u32,
        target: usize,
        chosen: &mut Vec<&'a ExplicitModDefinition>,
        chance: f64,
        out: &mut BTreeMap<Vec<String>, f64>,
    ) {
        let prefixes = chosen
            .iter()
            .filter(|definition| definition.affix_type == AffixType::Prefix)
            .count();
        let suffixes = chosen.len() - prefixes;
        let valid: Vec<&'a ExplicitModDefinition> = pool
            .iter()
            .filter(|definition| {
                let side_has_room = match definition.affix_type {
                    AffixType::Prefix => prefixes < limits.prefixes,
                    AffixType::Suffix => suffixes < limits.suffixes,
                };
                definition.definition.level <= item_level
                    && side_has_room
                    && chosen.len() < limits.mods
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
            step(pool, limits, item_level, target, chosen, next, out);
            chosen.pop();
        }
    }
    let mut out = BTreeMap::new();
    step(
        pool,
        limits,
        item_level,
        target,
        &mut Vec::new(),
        1.0,
        &mut out,
    );
    out
}

/// The exact chance of every result of the Chaos Orb (the PO's rule): a target of 4, 5 or 6 mods
/// with chances 8 : 3 : 1, then random mods are added until the item has that many mods or no
/// mod is added.
fn chaos_outcomes(
    pool: &[ExplicitModDefinition],
    limits: &AffixLimits,
    item_level: u32,
) -> BTreeMap<Vec<String>, f64> {
    let mut out: BTreeMap<Vec<String>, f64> = BTreeMap::new();
    for (target, _) in TARGETS {
        for (key, chance) in rule_outcomes(pool, limits, item_level, target) {
            *out.entry(key).or_default() += target_chance(target) * chance;
        }
    }
    out
}

/// The prefix/suffix splits of `trials` rare items of `base` at `item_level` without prefixes and
/// suffixes, each filled by "Adding a random mod" (`add_random_mod`, AB#248) to the largest
/// target, keyed by the number of mods. The rule adds one mod after the other and the target
/// takes no part in it, so the split after the 4th mod of a ring filled to 6 is that of a ring
/// filled to 4. Asserts that every item reaches the largest target.
fn reference_splits(
    base: &BaseItem,
    pool: &[ExplicitModDefinition],
    item_level: u32,
    trials: u64,
    rng: &mut SeededRng,
) -> BTreeMap<usize, BTreeMap<(usize, usize), u64>> {
    let largest = TARGETS.iter().map(|(target, _)| *target).max().unwrap_or(0);
    let mut splits: BTreeMap<usize, BTreeMap<(usize, usize), u64>> = BTreeMap::new();
    for trial in 0..trials {
        let mut item = item_with(base, Rarity::Rare, item_level, &[], &[], rng);
        for mods in 1..=largest {
            assert!(
                add_random_mod(&mut item, pool, rng),
                "reference {trial}: a rare {} of item level {item_level} reaches {largest} mods, \
                 found {:?}",
                base.name,
                outcome(&item)
            );
            *splits
                .entry(mods)
                .or_default()
                .entry(split(&item))
                .or_default() += 1;
        }
    }
    splits
}

// ---------------------------------------------------------------------------------------
// Helpers: statistics
// ---------------------------------------------------------------------------------------

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

/// Asserts that the share `count / n` of the orb lies within 5 standard deviations of the share
/// `reference / m` of the reference. Both shares are observed, so the standard deviation of their
/// difference is sqrt(p * (1 - p) * (1 / n + 1 / m)), with p the share of both together.
fn assert_same_share(what: &str, count: u64, n: u64, reference: u64, m: u64) {
    assert!(n > 0 && m > 0, "{what}: no trials to compare");
    let p = (count + reference) as f64 / (n + m) as f64;
    let observed = count as f64 / n as f64;
    let expected = reference as f64 / m as f64;
    let tolerance = 5.0 * (p * (1.0 - p) * (1.0 / n as f64 + 1.0 / m as f64)).sqrt();
    assert!(
        (observed - expected).abs() <= tolerance,
        "{what}: observed {observed:.4} ({count} of {n}), reference {expected:.4} ({reference} \
         of {m}) \u{b1} {tolerance:.4}"
    );
}

// ---------------------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------------------

// AC2: „Neben den bisherigen Orbs gibt es einen Chaos Orb. Sein Name ist ein Spieltext. Ist der
// Orb anwendbar, zeigt das Überfahren mit der Maus genau den Text `Reforges a rare item with new
// random modifiers` (Spieldaten)."
// (The button's place and the tooltip are checked by the PO in the local preview; the wording of
// the name and the description at the acceptance. Here: the six orbs exist, the new orb's name
// and description are the texts of its entry in the game data and are not written into the
// code, the orb applies to every rare ring, also without room or valid mods, and the page uses
// the new orb from crafting-core and shows its description.)
#[test]
fn ac2_chaos_orb_exists_with_its_game_name_and_description() {
    // Given the currencies of crafting-core
    let ids: Vec<String> = currencies().into_iter().map(|entry| entry.id).collect();

    // Then the Chaos Orb exists next to the other orbs
    for id in EARLIER_ORB_IDS.into_iter().chain([ORB_ID]) {
        assert!(
            ids.iter().any(|found| found == id),
            "currencies() must contain {id:?}, found {ids:?}"
        );
    }
    assert_eq!(Currency::ChaosOrb.id(), ORB_ID);

    // And its name is the game text of its entry in the game data
    let name = text_in_file(ORB_ID, &["name"]);
    assert_eq!(Currency::ChaosOrb.name(), name);
    assert_eq!(definition(ORB_ID).name, name);
    // And it is not written into the code of crafting-core or web
    assert_only_in_game_data(&name);

    // And its description is the game text of its entry in the game data
    let description = text_in_file(ORB_ID, &["description"]);
    assert_eq!(Currency::ChaosOrb.description(), description);
    assert_eq!(definition(ORB_ID).description, description);
    // And it is not written into the code of crafting-core or web
    assert_only_in_game_data(&description);

    // Given rare Iron, Composite and Cogwork Rings with 0 to 6 mods, also full ones, and rare
    // Composite Rings of item level 1 whose valid pool is empty
    let mut rng = SeededRng::new(SEED);
    let mut rare_items = Vec::new();
    for ring_id in [IRON_RING_ID, COMPOSITE_RING_ID, COGWORK_RING_ID] {
        let ring = base(ring_id);
        let pool = pool_of(ring_id);
        for trial in 0..ITEMS {
            rare_items.push(item_of_rarity(
                &ring,
                Rarity::Rare,
                DEFAULT_ITEM_LEVEL,
                &pool,
                (trial % 7) as usize,
                &mut rng,
            ));
        }
    }
    let composite = base(COMPOSITE_RING_ID);
    let composite_pool = pool_of(COMPOSITE_RING_ID);
    let (small_level, small_families) = SMALL_POOL_LEVELS[0];
    let small = item_of_rarity(
        &composite,
        Rarity::Rare,
        small_level,
        &composite_pool,
        small_families,
        &mut rng,
    );
    assert!(
        valid_pool(&small, &composite_pool).is_empty(),
        "the test needs a rare Composite Ring with an empty valid pool, found {:?}",
        outcome(&small)
    );
    rare_items.push(small);

    for item in &rare_items {
        // When the page asks whether the orb applies
        // Then it does, so the tooltip is the description
        assert_eq!(
            can_apply_chaos_orb(item),
            Ok(()),
            "rare {} of item level {} with {:?}",
            item.base.name,
            item.item_level,
            outcome(item)
        );
    }

    // And the page shows the new orb from crafting-core and takes its tooltip from there
    assert_web_src_uses(
        &[
            "OrbOfTransmutation",
            "OrbOfAlteration",
            "OrbOfAugmentation",
            "RegalOrb",
            "ExaltedOrb",
            "ChaosOrb",
            "can_apply_chaos_orb",
            "description()",
        ],
        "the Chaos Orb's button comes from crafting-core, next to the other orbs, and its tooltip \
         is its description",
    );
    assert_web_src_applies_the_orb();
}

// AC3: „Auf einem seltenen Item ersetzt der Orb alle Prefixes und Suffixes nach Kriterium 1. Die
// Implicits bleiben unverändert, das Item bleibt selten. Die Anzahl der Mods ist 4, 5 oder 6 im
// Verhältnis 8:3:1, die Verteilung auf Prefix und Suffix folgt den Gewichten. Beispiel: seltene
// Iron Rings auf Item-Level 100."
// (Covered: the example with the Iron Ring's whole pool from the game data, Chaos Orbs in a row
// starting from a full ring: 4, 5 and 6 mods with the shares 8 : 3 : 1, and for each number of
// mods the prefix/suffix split of rings filled by "Adding a random mod" (`add_random_mod`), as in
// the PO's evidence against a simulation of the rule; old mods that are in no pool are always
// removed, on the Iron, Composite and Cogwork Ring; invented pools on the Iron and the Cogwork
// Ring with the rule's exact chance of every result, in which prefixes weigh more than suffixes
// and the filters of the rule leave out mods; and every ring base, including the Breach and
// Heist rings, with 0 to 6 mods. Every result keeps the implicits, stays rare, holds both affix
// limits and only mods that can roll on the item, of different families.)
#[test]
fn ac3_the_orb_replaces_all_mods_with_4_5_or_6_by_8_3_1_split_by_weight() {
    let mut rng = SeededRng::new(STATISTICS_SEED);

    // --- The story's example: rare Iron Rings of item level 100 ---
    let iron = base(IRON_RING_ID);
    let iron_pool = pool_of(IRON_RING_ID);
    // Given a rare Iron Ring of item level 100 with 6 mods
    let mut item = item_of_rarity(
        &iron,
        Rarity::Rare,
        EXAMPLE_ITEM_LEVEL,
        &iron_pool,
        6,
        &mut rng,
    );
    assert_eq!(
        split(&item),
        (3, 3),
        "the example starts with a full Iron Ring"
    );
    // When the orb is applied again and again to the same ring
    let mut splits: BTreeMap<(usize, usize), u64> = BTreeMap::new();
    for trial in 0..IRON_TRIALS {
        reforge(
            &mut item,
            &iron_pool,
            &mut rng,
            &format!("rare Iron Ring, orb {trial}"),
        );
        *splits.entry(split(&item)).or_default() += 1;
    }
    let with_mods = |mods: usize| -> u64 {
        splits
            .iter()
            .filter(|((prefixes, suffixes), _)| prefixes + suffixes == mods)
            .map(|(_, count)| count)
            .sum()
    };
    // Then it always has 4, 5 or 6 mods (the Iron Ring always reaches the target)
    let reached: u64 = TARGETS.iter().map(|(target, _)| with_mods(*target)).sum();
    assert_eq!(
        reached, IRON_TRIALS,
        "the Iron Ring of item level 100 always gets 4, 5 or 6 mods, found {splits:?}"
    );
    // And 4, 5 and 6 mods come with the shares 8 : 3 : 1
    for (target, _) in TARGETS {
        assert_share(
            &format!("rare Iron Ring: {target} mods"),
            with_mods(target),
            IRON_TRIALS,
            target_chance(target),
        );
    }
    // And for each number of mods, the split on prefixes and suffixes is that of rings filled by
    // "Adding a random mod" to that many mods
    let mut reference_rng = SeededRng::new(REFERENCE_SEED);
    let references = reference_splits(
        &iron,
        &iron_pool,
        EXAMPLE_ITEM_LEVEL,
        REFERENCE_TRIALS,
        &mut reference_rng,
    );
    for (target, _) in TARGETS {
        let reference = references.get(&target).cloned().unwrap_or_default();
        let observed: BTreeMap<(usize, usize), u64> = splits
            .iter()
            .filter(|((prefixes, suffixes), _)| prefixes + suffixes == target)
            .map(|(class, count)| (*class, *count))
            .collect();
        let classes: BTreeSet<(usize, usize)> =
            reference.keys().chain(observed.keys()).copied().collect();
        for class in classes {
            assert_same_share(
                &format!(
                    "rare Iron Ring with {target} mods: {} prefixes and {} suffixes",
                    class.0, class.1
                ),
                observed.get(&class).copied().unwrap_or(0),
                with_mods(target),
                reference.get(&class).copied().unwrap_or(0),
                REFERENCE_TRIALS,
            );
        }
    }

    // --- All old prefixes and suffixes are removed ---
    for ring_id in [IRON_RING_ID, COMPOSITE_RING_ID, COGWORK_RING_ID] {
        let ring = base(ring_id);
        let pool = pool_of(ring_id);
        let ring_limits = limits(&item_with(
            &ring,
            Rarity::Rare,
            EXAMPLE_ITEM_LEVEL,
            &[],
            &[],
            &mut rng,
        ));
        let prefix_count = ring_limits.prefixes.min(ring_limits.mods);
        let suffix_count = ring_limits.suffixes.min(ring_limits.mods - prefix_count);
        let old_prefixes = old_mods(AffixType::Prefix, prefix_count);
        let old_suffixes = old_mods(AffixType::Suffix, suffix_count);
        for trial in 0..ITEMS {
            // Given a rare ring of item level 100 full of old mods that are in no pool
            let mut item = item_with(
                &ring,
                Rarity::Rare,
                EXAMPLE_ITEM_LEVEL,
                &old_prefixes.iter().collect::<Vec<_>>(),
                &old_suffixes.iter().collect::<Vec<_>>(),
                &mut rng,
            );
            let ctx = format!("rare {} full of old mods, {trial}", ring.name);
            // When the orb is applied
            reforge(&mut item, &pool, &mut rng, &ctx);
            // Then none of the old mods is left, and it has 4 to 6 new mods of the ring's pool
            let mods = explicits(&item).len();
            assert!(
                (4..=6).contains(&mods),
                "{ctx}: the old mods are removed and 4 to 6 new ones added, found {:?}",
                outcome(&item)
            );
        }
    }

    // --- Invented pools: the rule's exact chance of every result ---
    // Prefixes weigh 6000 together and suffixes 1500. "Prefix A" and "Prefix A2" share a family,
    // "Prefix A2" has the item level, "High Prefix" is above it.
    let invented_pool = vec![
        prefix("Prefix A", "A", 3000, 1),
        prefix("Prefix A2", "A", 1000, INVENTED_ITEM_LEVEL),
        prefix("Prefix B", "B", 1000, 1),
        prefix("Prefix C", "C", 1000, 1),
        prefix("High Prefix", "High", 9000, INVENTED_ITEM_LEVEL + 1),
        suffix("of D", "D", 500, 1),
        suffix("of E", "E", 500, 1),
        suffix("of F", "F", 250, 1),
        suffix("of G", "G", 250, 1),
    ];
    // Old mods of the families of "Prefix A" and "of D", which must not block them
    let old_prefix = prefix("Old Prefix A", "A", 1000, 1);
    let old_suffix = suffix("of the Old D", "D", 1000, 1);
    let mut test_rng = SeededRng::new(TEST_POOL_SEED);
    for ring_id in [IRON_RING_ID, COGWORK_RING_ID] {
        let ring = base(ring_id);
        // Given a rare ring of item level 50 with old mods and the invented pool
        let mut item = item_with(
            &ring,
            Rarity::Rare,
            INVENTED_ITEM_LEVEL,
            &[&old_prefix],
            &[&old_suffix],
            &mut test_rng,
        );
        let label = format!("invented pool, rare {}", ring.name);
        let expected = chaos_outcomes(&invented_pool, &limits(&item), INVENTED_ITEM_LEVEL);
        // (In this pool every target can be reached, so the numbers of mods are 8 : 3 : 1.)
        for (target, _) in TARGETS {
            let chance: f64 = expected
                .iter()
                .filter(|(key, _)| key.len() == target)
                .map(|(_, chance)| chance)
                .sum();
            assert!(
                (chance - target_chance(target)).abs() < 1e-9,
                "{label}: the test needs a pool in which {target} mods can be reached"
            );
        }
        // When the orb is applied again and again to the same ring
        let mut counts: BTreeMap<Vec<String>, u64> = BTreeMap::new();
        for trial in 0..TEST_POOL_TRIALS {
            reforge(
                &mut item,
                &invented_pool,
                &mut test_rng,
                &format!("{label}, orb {trial}"),
            );
            *counts.entry(sorted_outcome(&item)).or_default() += 1;
        }
        // Then every result is one the rule can give
        for (result, count) in &counts {
            assert!(
                expected.contains_key(result),
                "{label}: {count} results {result:?}, which the rule cannot give"
            );
        }
        // And each result occurs with the chance the rule gives
        for (result, chance) in &expected {
            assert_share(
                &format!("{label}: {result:?}"),
                counts.get(result).copied().unwrap_or(0),
                TEST_POOL_TRIALS,
                *chance,
            );
        }
        // And so do the numbers of mods and the splits on prefixes and suffixes
        let prefix_names: BTreeSet<&str> = invented_pool
            .iter()
            .filter(|definition| definition.affix_type == AffixType::Prefix)
            .map(|definition| definition.name.as_str())
            .collect();
        let split_of = |result: &[String]| -> (usize, usize) {
            let prefixes = result
                .iter()
                .filter(|name| prefix_names.contains(name.as_str()))
                .count();
            (prefixes, result.len() - prefixes)
        };
        let mut expected_splits: BTreeMap<(usize, usize), f64> = BTreeMap::new();
        for (result, chance) in &expected {
            *expected_splits.entry(split_of(result)).or_default() += chance;
        }
        let mut observed_splits: BTreeMap<(usize, usize), u64> = BTreeMap::new();
        for (result, count) in &counts {
            *observed_splits.entry(split_of(result)).or_default() += count;
        }
        for ((prefixes, suffixes), chance) in &expected_splits {
            assert_share(
                &format!("{label}: {prefixes} prefixes and {suffixes} suffixes"),
                observed_splits
                    .get(&(*prefixes, *suffixes))
                    .copied()
                    .unwrap_or(0),
                TEST_POOL_TRIALS,
                *chance,
            );
        }
        for (target, _) in TARGETS {
            let observed: u64 = observed_splits
                .iter()
                .filter(|((prefixes, suffixes), _)| prefixes + suffixes == target)
                .map(|(_, count)| count)
                .sum();
            assert_share(
                &format!("{label}: {target} mods"),
                observed,
                TEST_POOL_TRIALS,
                target_chance(target),
            );
        }
    }

    // --- Every ring base ---
    let rings = Slot::Ring.base_items();
    assert!(!rings.is_empty(), "the slot \"Ring\" must have ring bases");
    for ring in &rings {
        let pool = pool_of(&ring.id);
        for trial in 0..ITEMS_PER_RING {
            // Given a rare ring with 0 to 6 mods
            let mut item = item_of_rarity(
                ring,
                Rarity::Rare,
                DEFAULT_ITEM_LEVEL,
                &pool,
                (trial % 7) as usize,
                &mut rng,
            );
            // When the orb is applied
            // Then the result is one the rule allows (`valid_pool`, which also applies the lock
            // of the Breach rings), the implicits stay and the ring stays rare
            let ctx = format!("rare {} with {:?}", ring.name, outcome(&item));
            reforge(&mut item, &pool, &mut rng, &ctx);
        }
    }
}

// AC4: „Auf einem normalen und auf einem magischen Item ist der Orb nicht anwendbar. Überfahren
// mit der Maus und Klick zeigen genau die Meldung `Failed to apply item: Target is not Rare.` Das
// Item bleibt unverändert."
// (Tooltip, look and message area are checked by the PO in the local preview, the wording of the
// message at the acceptance. Here: the reason on hover and on click for normal items, magic items
// with 0 to 2 mods and, by the PO's rule "the item is not rare", unique items, with the ring's
// pool and with an empty pool; its message from the game data; the unchanged item; and that the
// page marks the orb and shows the message.)
#[test]
fn ac4_the_orb_on_a_normal_or_magic_item_shows_the_game_message_and_keeps_the_item() {
    let mut rng = SeededRng::new(SEED);
    let empty: Vec<ExplicitModDefinition> = Vec::new();

    for ring_id in [IRON_RING_ID, COMPOSITE_RING_ID, COGWORK_RING_ID] {
        let ring = base(ring_id);
        let pool = pool_of(ring_id);
        for trial in 0..ITEMS {
            // Given a normal ring, a magic ring with 0 to 2 mods and a unique ring
            let normal = Item::new(&ring, &mut rng);
            assert_eq!(normal.rarity, Rarity::Normal);
            let magic = item_of_rarity(
                &ring,
                Rarity::Magic,
                DEFAULT_ITEM_LEVEL,
                &pool,
                (trial % 3) as usize,
                &mut rng,
            );
            let unique = item_with(
                &ring,
                Rarity::Unique,
                DEFAULT_ITEM_LEVEL,
                &[],
                &[],
                &mut rng,
            );

            for item in [normal, magic, unique] {
                for click_pool in [&pool, &empty] {
                    let ctx = format!(
                        "{:?} {} {trial} with {:?}",
                        item.rarity,
                        ring.name,
                        outcome(&item)
                    );
                    // When the page asks whether the orb applies, and it is clicked anyway
                    // Then both fail because the item is not rare, whatever the pool, and the
                    // item is unchanged
                    assert_not_applicable(&item, click_pool, &mut rng, &ctx);
                }
            }
        }
    }

    // And hover and click show the orb's message for this reason from the game data
    assert_eq!(CraftError::ItemNotRare.code(), NOT_RARE_CODE);
    let not_rare = text_in_file(ORB_ID, &["messages", NOT_RARE_CODE]);
    assert_eq!(
        definition(ORB_ID)
            .messages
            .get(NOT_RARE_CODE)
            .map(String::as_str),
        Some(not_rare.as_str()),
        "currencies(): message of {ORB_ID:?} for {NOT_RARE_CODE:?}"
    );
    assert_eq!(
        Currency::ChaosOrb.failure_message(CraftError::ItemNotRare),
        not_rare,
        "the message of {ORB_ID:?} for an item that is not rare is the game data's"
    );
    // And it is not written into the code of crafting-core or web
    assert_only_in_game_data(&not_rare);
    // And the page marks the orb as not applicable and shows the message on hover and click
    assert_web_src_uses(
        &["can_apply_chaos_orb", "failure_message", "aria-disabled"],
        "an inapplicable Chaos Orb is marked and shows its failure message",
    );
    assert_web_src_applies_the_orb();
}

// AC5: „Kann der Orb weniger Mods hinzufügen als seine Ziel-Anzahl, hört er ohne Meldung auf.
// Beispiel: Ein seltener Composite Ring auf Item-Level 1 bekommt immer genau 4 Prefixes und 0
// Suffixes. Seine Basis hat dort nur 4 Prefix-Familien und darf keine Suffixes haben."
// (The look of the page after the click is checked by the PO in the local preview. Here: the
// example with the ring's pool, starting from 0 to 4 prefixes; by the PO's rule, on item level 2
// with 5 prefix families, 4 prefixes for a target of 4 and 5 prefixes for a target of 5 or 6;
// invented pools with only 2 valid mods, and an empty pool, which leaves no mod at all; on hover
// and click the orb takes effect without an error; and the orb has no message for a rare item
// without room or without a valid mod, neither in crafting-core nor in the game data.)
#[test]
fn ac5_the_orb_stops_without_a_message_when_it_cannot_reach_its_target() {
    let mut rng = SeededRng::new(SEED);
    let composite = base(COMPOSITE_RING_ID);
    let composite_pool = pool_of(COMPOSITE_RING_ID);

    // --- The story's example and the PO's rule ---
    for (item_level, families) in SMALL_POOL_LEVELS {
        // Given the Composite Ring's base on item level 1 (or 2): only `families` prefix
        // families and no suffix allowed
        let empty_ring = item_with(&composite, Rarity::Rare, item_level, &[], &[], &mut rng);
        assert_eq!(
            limits(&empty_ring).suffixes,
            0,
            "a rare Composite Ring may have no suffixes"
        );
        let rollable = valid_pool(&empty_ring, &composite_pool);
        let prefix_families: BTreeSet<&str> = rollable
            .iter()
            .filter(|definition| definition.affix_type == AffixType::Prefix)
            .map(|definition| definition.definition.family.as_str())
            .collect();
        assert_eq!(
            prefix_families.len(),
            families,
            "the Composite Ring's base has {families} prefix families on item level {item_level}"
        );
        assert!(
            limits(&empty_ring).prefixes > families,
            "the test needs a prefix limit above {families}"
        );

        let mut splits: BTreeMap<(usize, usize), u64> = BTreeMap::new();
        for trial in 0..SMALL_POOL_TRIALS {
            // Given a rare Composite Ring of that item level with 0 to `families` prefixes
            let mut item = item_of_rarity(
                &composite,
                Rarity::Rare,
                item_level,
                &composite_pool,
                (trial % (families as u64 + 1)) as usize,
                &mut rng,
            );
            let ctx = format!(
                "rare Composite Ring of item level {item_level}, {trial}, with {:?}",
                outcome(&item)
            );
            // When the orb is applied
            // Then it applies on hover and takes effect on click, without a message
            reforge(&mut item, &composite_pool, &mut rng, &ctx);
            *splits.entry(split(&item)).or_default() += 1;
        }
        if families == 4 {
            // Then the ring of item level 1 always gets exactly 4 prefixes and 0 suffixes
            assert_eq!(
                splits,
                BTreeMap::from([((4, 0), SMALL_POOL_TRIALS)]),
                "rare Composite Ring of item level {item_level}"
            );
        } else {
            // Then the ring of item level 2 gets 4 prefixes for a target of 4 and 5 for a target
            // of 5 or 6
            let five: f64 = TARGETS
                .iter()
                .filter(|(target, _)| *target >= families)
                .map(|(target, _)| target_chance(*target))
                .sum();
            for (class, chance) in [((families - 1, 0), 1.0 - five), ((families, 0), five)] {
                assert_share(
                    &format!(
                        "rare Composite Ring of item level {item_level}: {} prefixes",
                        class.0
                    ),
                    splits.get(&class).copied().unwrap_or(0),
                    SMALL_POOL_TRIALS,
                    chance,
                );
            }
            assert!(
                splits
                    .keys()
                    .all(|class| [(families - 1, 0), (families, 0)].contains(class)),
                "rare Composite Ring of item level {item_level}: only {} or {families} prefixes, \
                 found {splits:?}",
                families - 1
            );
        }
    }

    // --- Invented pools with fewer valid mods than any target ---
    let iron = base(IRON_RING_ID);
    let empty: Vec<ExplicitModDefinition> = Vec::new();
    let two_valid = vec![
        prefix("Prefix A", "A", 1000, 1),
        prefix("Prefix A2", "A", 1000, 1),
        prefix("High Prefix", "High", 9000, INVENTED_ITEM_LEVEL + 1),
        suffix("of B", "B", 1000, INVENTED_ITEM_LEVEL),
    ];
    let old_prefix = prefix("Old Prefix", "Old", 1000, 1);
    let old_suffix = suffix("of the Old", "Old Suffix", 1000, 1);
    for (label, pool, mods) in [
        ("an invented pool with 2 valid families", &two_valid, 2),
        ("an empty pool", &empty, 0),
    ] {
        for trial in 0..ITEMS {
            // Given a rare Iron Ring of item level 50 with old mods
            let mut item = item_with(
                &iron,
                Rarity::Rare,
                INVENTED_ITEM_LEVEL,
                &[&old_prefix],
                &[&old_suffix],
                &mut rng,
            );
            let ctx = format!("rare Iron Ring with {label}, {trial}");
            // When the orb is applied
            // Then it takes effect without a message: the old mods are removed, and it adds as
            // many mods as it can and stops
            reforge(&mut item, pool, &mut rng, &ctx);
            assert_eq!(
                explicits(&item).len(),
                mods,
                "{ctx}: the orb adds {mods} mods, found {:?}",
                outcome(&item)
            );
            assert!(
                valid_pool(&item, pool).is_empty(),
                "{ctx}: no mod can be added any more"
            );
        }
    }

    // --- The orb has no message for a rare item without room or without a valid mod ---
    // Given the game's messages for these reasons, from the Exalted Orb's entry
    let reasons = [NO_SPACE_CODE, NO_VALID_MOD_CODE];
    let game_messages: Vec<String> = reasons
        .iter()
        .map(|code| text_in_file(EXALTED_ID, &["messages", code]))
        .collect();
    // When reading the Chaos Orb's messages in the game data and in crafting-core
    let entry = entry_in_file(ORB_ID);
    let file_messages = entry
        .get("messages")
        .and_then(Value::as_object)
        .unwrap_or_else(|| panic!("{CURRENCIES_JSON}: {ORB_ID:?} must have \"messages\""));
    let core_messages = definition(ORB_ID).messages;
    // Then it has none for these reasons and none of these messages
    for code in reasons {
        assert!(
            !file_messages.contains_key(code),
            "{CURRENCIES_JSON}: {ORB_ID:?} must have no message for {code:?}"
        );
        assert!(
            !core_messages.contains_key(code),
            "currencies(): {ORB_ID:?} must have no message for {code:?}"
        );
    }
    for message in &game_messages {
        assert!(
            file_messages
                .values()
                .all(|value| value.as_str() != Some(message.as_str())),
            "{CURRENCIES_JSON}: {ORB_ID:?} must not have the message {message:?}, found \
             {file_messages:?}"
        );
        assert!(
            core_messages.values().all(|value| value != message),
            "currencies(): {ORB_ID:?} must not have the message {message:?}, found \
             {core_messages:?}"
        );
    }
}
