//! Acceptance tests for AB#306 "Exalted Orb auf seltene Items anwenden".
//!
//! The tests use the public interface of `crafting-core` from the tech lead's plan on AB#306:
//! - `Currency::ExaltedOrb` with the id `exalted_orb`; its name, description and messages are
//!   only in `data/currencies.json`.
//! - `CraftError::ItemNotRare` with the code `item_not_rare`.
//! - `can_apply_exalted_orb(item, pool) -> Result<(), CraftError>`: `pool` is the base's pool.
//!   In this order: `ItemNotRare` for a normal, magic or unique item; `NoSpaceForMods` for a rare
//!   item on which neither side has room under both affix limits; `NoValidMod` if the valid pool
//!   is empty; otherwise `Ok`.
//! - `apply_exalted_orb(item, pool, rng) -> Result<(), CraftError>`: first the same checks, then
//!   exactly one `add_random_mod`. The item stays rare, the new mod comes at the end of its side,
//!   and on an error the item is unchanged.
//! - unchanged: `currencies`, `Currency::{id, name, description, failure_message}`,
//!   `CraftError::code`, `add_random_mod`, `valid_pool`, `mod_pool`, `base_item`,
//!   `Slot::Ring.base_items()`, `Item::{affix_limits, advanced_mods, tier}`, `Mod::roll`,
//!   `SeededRng`. Following ADR 0004, web criteria are checked through the file contents under
//!   `web/` (without the build output in `web/dist` and `web/target`).
//!
//! The PO's rules are `poe-crafting-rules`: `currencies/exalted-orb.md` (committed in 5e69790,
//! before these tests), "Affix limits" in "Item basics", "Advanced mod descriptions" and "Adding
//! a random mod".
//!
//! Criterion 1 gives the PO's wording for `poe-crafting-rules`; the wording is checked byte for
//! byte at the acceptance (step 2 of the `story` skill), so its test checks only that the file
//! exists and is linked. It is already met by 5e69790, so it is red only because the module does
//! not compile. The behaviour of the rule is checked by the tests of criteria 3 to 6.
//!
//! Game texts (the orb's name, its description and its messages) and UI texts are not compared
//! word for word here (`writing-acceptance-tests`, "What permanent tests check"): their wording is
//! checked at the acceptance. The tests check that crafting-core shows the texts of the orb's
//! entry in `data/currencies.json` under the reason codes of the plan, that the three reasons have
//! messages of their own, and that the texts are not written into the code of `crafting-core` or
//! `web`.
//!
//! Every random test uses a fixed seed. Statistical tests compare observed shares with the
//! shares that follow from the weights and the rule, with a tolerance of 5 standard deviations.
//! The tests depend only on frequencies and invariants, never on the order in which the
//! generator is used.
//!
//! Red state before the implementation: the module does not compile, because
//! `Currency::ExaltedOrb`, `CraftError::ItemNotRare`, `can_apply_exalted_orb` and
//! `apply_exalted_orb` do not exist yet.
//!
//! What is checked outside `cargo test`, by the PO in the local preview:
//! - AC2: the fifth button next to the other orbs and its tooltip.
//! - AC4 to AC6: how the inapplicable orb looks, its tooltip and the message after a click.
//!
//! Not testable with today's game data: the PO's evidence of a rare Spectral Sword with the
//! crafted suffix "Cannot roll Attack Modifiers", because the simulator has neither weapons nor
//! crafted mods. The rare Composite Ring of item level 1 or 2 from the PO's rule stands in for it
//! in AC6.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crafting_core::{
    add_random_mod, apply_exalted_orb, base_item, can_apply_exalted_orb, currencies, mod_pool,
    valid_pool, AffixLimits, AffixType, BaseItem, CraftError, Currency, CurrencyDefinition,
    ExplicitMod, ExplicitModDefinition, Item, Mod, ModBlock, ModDefinition, ModKind, Rarity,
    SeededRng, Slot, StatDefinition, DEFAULT_ITEM_LEVEL,
};
use serde_json::Value;

// ---------------------------------------------------------------------------------------
// Values from the story, the plan and the PO's rules
// ---------------------------------------------------------------------------------------

const IRON_RING_ID: &str = "Metadata/Items/Rings/Ring1";
/// "+3 Prefix Modifiers allowed", "-3 Suffix Modifiers allowed" (AB#189): as a rare item at most
/// 6 prefixes and no suffix.
const COMPOSITE_RING_ID: &str = "Metadata/Items/Rings/RingE3";
/// "-1 Prefix Modifier allowed", "+1 Suffix Modifier allowed" (AB#189).
const COGWORK_RING_ID: &str = "Metadata/Items/Rings/RingE1";
/// The Unset Ring of the PO's evidence in `currencies/exalted-orb.md`.
const UNSET_RING_ID: &str = "Metadata/Items/Rings/Ring15";

const ORB_ID: &str = "exalted_orb";
const TRANSMUTATION_ID: &str = "orb_of_transmutation";
const ALTERATION_ID: &str = "orb_of_alteration";
const AUGMENTATION_ID: &str = "orb_of_augmentation";
const REGAL_ID: &str = "regal_orb";

/// Reason codes from the tech lead's plan and ADR 0012, the keys of `messages`.
const NOT_RARE_CODE: &str = "item_not_rare";
const NO_SPACE_CODE: &str = "no_space_for_mods";
const NO_VALID_MOD_CODE: &str = "no_valid_mod";

/// Mods for a rare Iron Ring with a prefix and a suffix (AC3).
const STOUT: &str = "Stout";
const PHILOSOPHER: &str = "of the Philosopher";
/// Three prefixes of the Composite Ring, of three families, for the example of criterion 3.
const INCANDESCENT: &str = "Incandescent";
const DRAGONS: &str = "Dragon's";
const GLINTING: &str = "Glinting";
/// The PO's evidence in `currencies/exalted-orb.md`: a rare Unset Ring with 1 prefix and 2
/// suffixes got the prefix "Dancer's" (Tier 6).
const DANCERS: &str = "Dancer's";
const DANCERS_TIER: u32 = 6;

/// Criterion 6 and the PO's rule: on item level 1 the Composite Ring's base has only 4 prefix
/// families, on item level 2 five; both are fewer than its prefix limit of 6.
const SMALL_POOL_LEVELS: [(u32, usize); 2] = [(1, 4), (2, 5)];

const CURRENCIES_JSON: &str = "data/currencies.json";
const CRAFTING_RULES: &str = ".claude/skills/poe-crafting-rules/SKILL.md";
const ORB_RULE_FILE: &str = ".claude/skills/poe-crafting-rules/currencies/exalted-orb.md";
const ORB_RULE_LINK: &str = "currencies/exalted-orb.md";
const CURRENCY_RULES_HEADING: &str = "Currency rules:";

/// Fixed seeds, so every run is identical.
const SEED: u64 = 306;
const STATISTICS_SEED: u64 = 30_601;
const TEST_POOL_SEED: u64 = 30_602;

/// Items for invariant checks, per ring of AC2 and AC4.
const ITEMS: u64 = 300;
/// Rare items per ring base (AC3, AC5).
const ITEMS_PER_RING: u64 = 30;
/// Of the items of AC3, those whose advanced view is checked: on the Iron, Composite and Cogwork
/// Ring the first six (0 to 5 mods), on every other ring base one. Each advanced view reads the
/// base's pool from the game data, so checking the view of every item would take longer than a
/// few seconds.
const VIEW_RINGS: [&str; 3] = [IRON_RING_ID, COMPOSITE_RING_ID, COGWORK_RING_ID];
/// Orbs on a ring with the ring's whole pool (AC3). The valid pools hold about 200 mods of both
/// sides or about 80 prefixes, weighted 50 to 2000. A uniform choice gives the mods of weight
/// 1000 or 2000 clearly other shares than their weights; 5 standard deviations of a share are at
/// most 0.025, so it fails clearly. Each orb builds the valid pool of about 230 mods, so more orbs
/// would take longer than a few seconds.
const POOL_TRIALS: u64 = 10_000;
/// Orbs per invented pool (AC3). 5 standard deviations of a share are at most 0.018, so a
/// uniform choice (for example 50 % instead of 20 %) or a coin flip between the sides (50 %
/// instead of 20 % or 80 %) fails clearly, as does a choice that ignores one of the filters (a
/// left-out mod of weight 9000 would take most of the orbs).
const TEST_POOL_TRIALS: u64 = 20_000;
/// Rare Composite Rings per item level in AC6, each filled with other prefixes.
const SMALL_POOL_ITEMS: u64 = 100;

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

/// Asserts that some line under `web/src/` uses `apply_exalted_orb` other than as part of
/// `can_apply_exalted_orb`: the page applies the orb on a click, not only asks whether it
/// applies.
fn assert_web_src_applies_the_orb() {
    let applies = web_src_files().iter().any(|path| {
        read_path(path).lines().any(|line| {
            line.replace("can_apply_exalted_orb", "")
                .contains("apply_exalted_orb")
        })
    });
    assert!(
        applies,
        "a file under web/src/ must call `apply_exalted_orb`: a click applies the orb"
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

/// The text of the Exalted Orb's entry in `data/currencies.json` at the given path of keys. A
/// game text is never empty.
fn text_in_file(path: &[&str]) -> String {
    let entry = entry_in_file(ORB_ID);
    let mut value = &entry;
    for key in path {
        value = value.get(key).unwrap_or_else(|| {
            panic!("{CURRENCIES_JSON}: the entry {ORB_ID:?} must have {path:?}, found {entry}")
        });
    }
    let text = value
        .as_str()
        .unwrap_or_else(|| panic!("{CURRENCIES_JSON}: {ORB_ID:?} {path:?} must be a string"))
        .to_owned();
    assert!(
        !text.trim().is_empty(),
        "{CURRENCIES_JSON}: {ORB_ID:?} {path:?} must be a game text, found {text:?}"
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

/// Asserts that the Exalted Orb shows the game data's message for `error`: `code()` is the plan's
/// reason code, and `failure_message` is the text under that code in `currencies()` and in
/// `data/currencies.json`. Returns the message.
fn assert_message_is_game_data(error: CraftError, code: &str) -> String {
    assert_eq!(error.code(), code, "the reason code of {error:?}");
    let in_file = text_in_file(&["messages", code]);
    assert_eq!(
        definition(ORB_ID).messages.get(code).map(String::as_str),
        Some(in_file.as_str()),
        "currencies(): message of {ORB_ID:?} for {code:?}"
    );
    assert_eq!(
        Currency::ExaltedOrb.failure_message(error),
        in_file,
        "the message of {ORB_ID:?} for {error:?} is the game data's"
    );
    in_file
}

/// Asserts that the three reasons of the PO's rule have messages of their own.
fn assert_reasons_have_own_messages() {
    let messages: BTreeSet<String> = [
        CraftError::ItemNotRare,
        CraftError::NoSpaceForMods,
        CraftError::NoValidMod,
    ]
    .into_iter()
    .map(|error| Currency::ExaltedOrb.failure_message(error))
    .collect();
    assert_eq!(
        messages.len(),
        3,
        "the three reasons of the PO's rule have messages of their own, found {messages:#?}"
    );
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

/// Invented mods of `count` different families on one side, for items whose families are held.
fn held_mods(affix_type: AffixType, count: usize) -> Vec<ExplicitModDefinition> {
    (0..count)
        .map(|n| {
            test_mod(
                &format!("Held {affix_type:?} {n}"),
                affix_type,
                &format!("Held {affix_type:?} family {n}"),
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

/// A new item of `base` at the default item level with the given rarity, prefixes and suffixes,
/// rolled from the definitions, in this order.
fn item_with(
    base: &BaseItem,
    rarity: Rarity,
    prefixes: &[&ExplicitModDefinition],
    suffixes: &[&ExplicitModDefinition],
    rng: &mut SeededRng,
) -> Item {
    let mut item = Item::new(base, rng);
    item.item_level = DEFAULT_ITEM_LEVEL;
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

/// An item of `base` with the given rarity at the default item level with `mods` random mods of
/// `pool`, or as many as the limits of its rarity allow.
fn item_of_rarity(
    base: &BaseItem,
    rarity: Rarity,
    pool: &[ExplicitModDefinition],
    mods: usize,
    rng: &mut SeededRng,
) -> Item {
    let mut item = item_with(base, rarity, &[], &[], rng);
    while explicits(&item).len() < mods && add_random_mod(&mut item, pool, rng) {}
    item
}

/// A rare item of `base` at the default item level with up to `mods` random mods of `pool`, with
/// room for one more: if the limits of the base leave no room, its last mods are removed until a
/// side has room again.
fn rare_with_room(
    base: &BaseItem,
    pool: &[ExplicitModDefinition],
    mods: usize,
    rng: &mut SeededRng,
) -> Item {
    let mut item = item_of_rarity(base, Rarity::Rare, pool, mods, rng);
    while !has_room(&item) {
        if item.suffixes.pop().is_none() {
            item.prefixes
                .pop()
                .expect("a rare item without room has a mod");
        }
    }
    item
}

/// A rare item of `base` at the default item level, filled with random mods of `pool` until the
/// rule adds no more.
fn full_rare(base: &BaseItem, pool: &[ExplicitModDefinition], rng: &mut SeededRng) -> Item {
    let mut item = item_with(base, Rarity::Rare, &[], &[], rng);
    while add_random_mod(&mut item, pool, rng) {}
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

fn names<'a>(pool: impl IntoIterator<Item = &'a ExplicitModDefinition>) -> Vec<String> {
    pool.into_iter()
        .map(|definition| definition.name.clone())
        .collect()
}

fn limits(item: &Item) -> AffixLimits {
    item.affix_limits()
        .unwrap_or_else(|| panic!("a {:?} item has affix limits", item.rarity))
}

/// Whether a side of the item has room under both affix limits of its rarity ("Affix limits").
fn side_has_room(item: &Item, side: AffixType) -> bool {
    let limits = limits(item);
    let total = item.prefixes.len() + item.suffixes.len();
    let (held, side_limit) = match side {
        AffixType::Prefix => (item.prefixes.len(), limits.prefixes),
        AffixType::Suffix => (item.suffixes.len(), limits.suffixes),
    };
    total < limits.mods && held < side_limit
}

/// Whether some side of the item has room under both affix limits.
fn has_room(item: &Item) -> bool {
    side_has_room(item, AffixType::Prefix) || side_has_room(item, AffixType::Suffix)
}

/// Step 1 of "Adding a random mod" for a base without a lock (no Breach ring): every mod of
/// `pool` whose level is at most the item level, whose side has room under both affix limits,
/// and whose family is not already on the item, in the order of `pool`.
fn rule_pool<'a>(item: &Item, pool: &'a [ExplicitModDefinition]) -> Vec<&'a ExplicitModDefinition> {
    let held: BTreeSet<&str> = explicits(item)
        .iter()
        .map(|explicit| explicit.modifier.family.as_str())
        .collect();
    pool.iter()
        .filter(|definition| {
            definition.definition.level <= item.item_level
                && side_has_room(item, definition.affix_type)
                && !held.contains(definition.definition.family.as_str())
        })
        .collect()
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

/// Asserts that `after` is the rare item `before`, still rare, with exactly one more mod at the
/// end of its side and all other mods unchanged in value and order, and returns that mod.
fn assert_one_mod_added(before: &Item, after: &Item, ctx: &str) -> ExplicitMod {
    assert_eq!(before.rarity, Rarity::Rare, "{ctx}: the test starts rare");
    assert_eq!(after.rarity, Rarity::Rare, "{ctx}: the item stays rare");
    assert_eq!(after.base, before.base, "{ctx}: the base stays");
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
        "{ctx}: the mods on the side of the new mod stay unchanged with their values, in their \
         order"
    );
    assert_eq!(
        other_after, other_before,
        "{ctx}: the other side stays unchanged with its values"
    );
    let added = side.last().expect("one mod was added").clone();
    assert_eq!(
        added.affix_type, affix_type,
        "{ctx}: {:?} sits on its own side",
        added.name
    );
    added
}

/// Applies the orb to a copy of the rare `item`, checks the rule for one result and returns the
/// item and the new mod: the orb applies on hover, takes effect on click, and exactly one mod of
/// `expected` (the valid pool) is added at the end of its side, rolled from its definition, and
/// nothing else changes.
fn exalt_once(
    item: &Item,
    pool: &[ExplicitModDefinition],
    expected: &[&ExplicitModDefinition],
    rng: &mut SeededRng,
    ctx: &str,
) -> (Item, ExplicitMod) {
    assert_eq!(
        can_apply_exalted_orb(item, pool),
        Ok(()),
        "{ctx}: the orb applies to a rare item with room and a valid mod, found {:?}",
        outcome(item)
    );
    let mut after = item.clone();
    let result = apply_exalted_orb(&mut after, pool, rng);
    assert_eq!(result, Ok(()), "{ctx}: the orb takes effect");
    let added = assert_one_mod_added(item, &after, ctx);
    let definition = expected
        .iter()
        .find(|definition| definition.name == added.name)
        .unwrap_or_else(|| {
            panic!(
                "{ctx}: {:?} is not in the valid pool {:?}",
                added.name,
                names(expected.iter().copied())
            )
        });
    assert_rolled_from(definition, &added, ctx);
    (after, added)
}

/// Asserts that hover and click both fail with `error` on `item` with `pool`, and that the click
/// leaves the item unchanged.
fn assert_not_applicable(
    item: &Item,
    pool: &[ExplicitModDefinition],
    error: CraftError,
    rng: &mut SeededRng,
    ctx: &str,
) {
    // When the page asks whether the orb applies (hover)
    let hover = can_apply_exalted_orb(item, pool);
    // And the orb is clicked anyway
    let mut clicked = item.clone();
    let click = apply_exalted_orb(&mut clicked, pool, rng);
    // Then both fail with the reason
    assert_eq!(hover, Err(error), "{ctx}: hover");
    assert_eq!(click, Err(error), "{ctx}: click");
    // And the item is unchanged
    assert_eq!(&clicked, item, "{ctx}: the item is unchanged");
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

/// Applies the orb `trials` times to copies of the rare `item` and asserts that the new mod is
/// chosen from the valid pool of the rule, each mod with the chance of its weight divided by the
/// sum of the weights in the pool. Also checks the shares of the mods of each weight and of each
/// side, so that a choice that ignores the weights or flips a coin between the sides fails
/// clearly. Returns how often each side got the new mod.
fn assert_exalts_by_weight(
    item: &Item,
    pool: &[ExplicitModDefinition],
    trials: u64,
    rng: &mut SeededRng,
    label: &str,
) -> BTreeMap<String, u64> {
    let expected = rule_pool(item, pool);
    assert!(
        !expected.is_empty(),
        "{label}: the test needs a valid pool with mods"
    );
    // The rule has one place (AB#248): the valid pool of crafting-core is the rule's.
    assert_eq!(
        names(&valid_pool(item, pool)),
        names(expected.iter().copied()),
        "{label}: valid pool"
    );
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    for trial in 0..trials {
        let (_, added) = exalt_once(item, pool, &expected, rng, &format!("{label}, {trial}"));
        *counts.entry(added.name).or_default() += 1;
    }
    let count = |name: &str| counts.get(name).copied().unwrap_or(0);
    let total: f64 = expected
        .iter()
        .map(|definition| f64::from(definition.weight))
        .sum();
    for definition in &expected {
        assert_share(
            &format!("{label}: {:?}", definition.name),
            count(&definition.name),
            trials,
            f64::from(definition.weight) / total,
        );
    }
    let mut by_weight: BTreeMap<u32, (u64, f64)> = BTreeMap::new();
    let mut by_side: BTreeMap<String, (u64, f64)> = BTreeMap::new();
    for definition in &expected {
        let weight = f64::from(definition.weight);
        let entry = by_weight.entry(definition.weight).or_default();
        entry.0 += count(&definition.name);
        entry.1 += weight;
        let entry = by_side
            .entry(format!("{:?}", definition.affix_type))
            .or_default();
        entry.0 += count(&definition.name);
        entry.1 += weight;
    }
    for (weight, (hits, weights)) in by_weight {
        assert_share(
            &format!("{label}: mods with weight {weight}"),
            hits,
            trials,
            weights / total,
        );
    }
    let mut sides = BTreeMap::new();
    for (side, (hits, weights)) in by_side {
        assert_share(
            &format!("{label}: {side} mods"),
            hits,
            trials,
            weights / total,
        );
        sides.insert(side, hits);
    }
    sides
}

// ---------------------------------------------------------------------------------------
// Helpers: views
// ---------------------------------------------------------------------------------------

fn block_kind(affix_type: AffixType) -> ModKind {
    match affix_type {
        AffixType::Prefix => ModKind::Prefix,
        AffixType::Suffix => ModKind::Suffix,
    }
}

fn blocks_of(blocks: &[ModBlock], kind: ModKind) -> Vec<ModBlock> {
    blocks
        .iter()
        .filter(|block| block.kind == kind)
        .cloned()
        .collect()
}

/// Asserts that the advanced view of `after` keeps every block `before` showed, in its order,
/// and shows `added` as the last mod of its side, with its name, tier, tags and lines. Returns
/// the new block.
fn assert_new_mod_last_of_its_side(
    before: &Item,
    after: &Item,
    added: &ExplicitMod,
    ctx: &str,
) -> ModBlock {
    let blocks = after.advanced_mods();
    let before_blocks = before.advanced_mods();

    // Implicits, then prefixes, then suffixes
    let order: Vec<ModKind> = blocks.iter().map(|block| block.kind).collect();
    let mut sorted = order.clone();
    sorted.sort_by_key(|kind| match kind {
        ModKind::Implicit => 0,
        ModKind::Prefix => 1,
        ModKind::Suffix => 2,
    });
    assert_eq!(
        order, sorted,
        "{ctx}: implicits, then prefixes, then suffixes"
    );

    // The blocks shown before stay, in their order; the new mod is the last of its side
    let kind = block_kind(added.affix_type);
    for other in [ModKind::Implicit, ModKind::Prefix, ModKind::Suffix] {
        if other != kind {
            assert_eq!(
                blocks_of(&blocks, other),
                blocks_of(&before_blocks, other),
                "{ctx}: the {other:?} blocks stay"
            );
        }
    }
    let side = blocks_of(&blocks, kind);
    let kept = blocks_of(&before_blocks, kind);
    assert_eq!(side.len(), kept.len() + 1, "{ctx}: one more {kind:?} block");
    assert_eq!(
        &side[..kept.len()],
        &kept[..],
        "{ctx}: the {kind:?} blocks shown before stay, in their order"
    );
    let last = side.last().expect("one block was added").clone();
    assert_eq!(
        last.name.as_deref(),
        Some(added.name.as_str()),
        "{ctx}: the new mod is the last {kind:?} block"
    );
    assert_eq!(last.tier, Some(after.tier(added)), "{ctx}: its tier");
    assert_eq!(last.tags, added.modifier.tag_text(), "{ctx}: its tags");
    assert_eq!(
        last.lines,
        added.modifier.extended_lines(),
        "{ctx}: its lines"
    );
    last
}

// ---------------------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------------------

// AC1: „Regel des PO, Wortlaut für `poe-crafting-rules` (verbindlich): Die neue Datei
// `currencies/exalted-orb.md` hat genau den Inhalt unten und wird in der Liste „Currency rules“
// verlinkt."
// (The wording is checked byte for byte at the acceptance, step 2 of the `story` skill. Here:
// the file exists and the list "Currency rules" links it. The behaviour of the rule is checked
// by AC3 to AC6.)
#[test]
fn ac1_exalted_orb_rule_file_exists_and_is_linked() {
    // Given the PO's rules
    let rules = read_text(CRAFTING_RULES);

    // When reading the rule file of the Exalted Orb
    let orb_rules = read_text(ORB_RULE_FILE);

    // Then it is not empty
    assert!(
        !orb_rules.trim().is_empty(),
        "{ORB_RULE_FILE} must hold the PO's rule"
    );

    // And the list "Currency rules" links it
    let list: Vec<&str> = rules
        .lines()
        .map(str::trim_end)
        .skip_while(|line| *line != CURRENCY_RULES_HEADING)
        .skip(1)
        .take_while(|line| line.starts_with("- "))
        .collect();
    assert!(
        !list.is_empty(),
        "{CRAFTING_RULES} must have the list {CURRENCY_RULES_HEADING:?}"
    );
    assert!(
        list.iter()
            .any(|line| line.contains(&format!("]({ORB_RULE_LINK})"))),
        "the list {CURRENCY_RULES_HEADING:?} in {CRAFTING_RULES} must link {ORB_RULE_LINK}, \
         found {list:#?}"
    );
}

// AC2: „Neben den bisherigen Orbs gibt es einen Exalted Orb. Sein Name ist ein Spieltext. Ist
// der Orb anwendbar, zeigt das Überfahren mit der Maus genau den Text `Augments a rare item with
// a new random modifier` (Spieldaten)."
// (The button's place and the tooltip are checked by the PO in the local preview; the wording of
// the name and the description at the acceptance. Here: the five orbs exist, the new orb's name
// and description are the texts of its entry in the game data and are not written into the
// code, the orb applies to rare items with room, and the page uses the new orb from crafting-core
// and shows its description.)
#[test]
fn ac2_exalted_orb_exists_with_its_game_name_and_description() {
    // Given the currencies of crafting-core
    let ids: Vec<String> = currencies().into_iter().map(|entry| entry.id).collect();

    // Then the Exalted Orb exists next to the other orbs
    for id in [
        TRANSMUTATION_ID,
        ALTERATION_ID,
        AUGMENTATION_ID,
        REGAL_ID,
        ORB_ID,
    ] {
        assert!(
            ids.iter().any(|found| found == id),
            "currencies() must contain {id:?}, found {ids:?}"
        );
    }
    assert_eq!(Currency::ExaltedOrb.id(), ORB_ID);

    // And its name is the game text of its entry in the game data
    let name = text_in_file(&["name"]);
    assert_eq!(Currency::ExaltedOrb.name(), name);
    assert_eq!(definition(ORB_ID).name, name);
    // And it is not written into the code of crafting-core or web
    assert_only_in_game_data(&name);

    // And its description is the game text of its entry in the game data
    let description = text_in_file(&["description"]);
    assert_eq!(Currency::ExaltedOrb.description(), description);
    assert_eq!(definition(ORB_ID).description, description);
    // And it is not written into the code of crafting-core or web
    assert_only_in_game_data(&description);

    // Given rare Iron Rings, Composite Rings and Cogwork Rings with 0 to 5 mods and room
    let mut rng = SeededRng::new(SEED);
    for ring_id in [IRON_RING_ID, COMPOSITE_RING_ID, COGWORK_RING_ID] {
        let ring = base(ring_id);
        let pool = pool_of(ring_id);
        for trial in 0..ITEMS {
            let item = rare_with_room(&ring, &pool, (trial % 6) as usize, &mut rng);
            // When the page asks whether the orb applies
            // Then it does, so the tooltip is the description
            assert_eq!(
                can_apply_exalted_orb(&item, &pool),
                Ok(()),
                "rare {} with {:?}",
                ring.name,
                outcome(&item)
            );
        }
    }

    // And the page shows the new orb from crafting-core and takes its tooltip from there
    assert_web_src_uses(
        &[
            "OrbOfTransmutation",
            "OrbOfAlteration",
            "OrbOfAugmentation",
            "RegalOrb",
            "ExaltedOrb",
            "can_apply_exalted_orb",
            "description()",
        ],
        "the Exalted Orb's button comes from crafting-core, next to the other orbs, and its \
         tooltip is its description",
    );
    assert_web_src_applies_the_orb();
}

// AC3: „Auf einem seltenen Item mit Platz fügt der Orb genau einen Mod nach Kriterium 1 hinzu.
// Die vorhandenen Mods bleiben mit ihren Werten unverändert. In der Advanced-Ansicht steht der
// neue Mod als letzter seiner Seite. Beispiel: Ein seltener Composite Ring mit 3 Prefixes bekommt
// einen vierten Prefix, nie einen Suffix."
// (Covered: the example with the ring's whole pool from the game data, by weight and by the rule
// "Adding a random mod"; a rare Iron Ring with a prefix and a suffix the same way; the PO's
// evidence of the Unset Ring with "Dancer's" (Tier 6) in the advanced view; invented pools in
// which every filter of the rule leaves out a mod with a high weight, on the Iron, Composite and
// Cogwork Ring; and every ring base, including the Breach and Heist rings, with 0 to 5 mods and
// room, with the new mod last of its side in the advanced view.)
#[test]
fn ac3_the_orb_adds_exactly_one_mod_by_the_rule_and_shows_it_last_of_its_side() {
    let mut rng = SeededRng::new(STATISTICS_SEED);
    let iron = base(IRON_RING_ID);
    let iron_pool = pool_of(IRON_RING_ID);
    let composite = base(COMPOSITE_RING_ID);
    let composite_pool = pool_of(COMPOSITE_RING_ID);
    let cogwork = base(COGWORK_RING_ID);

    // --- The story's example with the game data ---
    // Given a rare Composite Ring with 3 prefixes
    let composite_three = item_with(
        &composite,
        Rarity::Rare,
        &[
            find(&composite_pool, INCANDESCENT),
            find(&composite_pool, DRAGONS),
            find(&composite_pool, GLINTING),
        ],
        &[],
        &mut rng,
    );
    assert!(
        composite_pool
            .iter()
            .any(|definition| definition.affix_type == AffixType::Suffix),
        "the Composite Ring's pool must hold suffixes, so the limits of a rare item are checked"
    );
    // When the orb is applied many times
    // Then it always gets a fourth prefix, never a suffix, chosen by weight, and the three
    // prefixes stay unchanged
    let sides = assert_exalts_by_weight(
        &composite_three,
        &composite_pool,
        POOL_TRIALS,
        &mut rng,
        "rare Composite Ring with 3 prefixes",
    );
    assert_eq!(
        sides.keys().collect::<Vec<_>>(),
        ["Prefix"],
        "the Composite Ring always gets a prefix, found {sides:?}"
    );
    // And the advanced view shows the fourth prefix after the three
    let (after, added) = exalt_once(
        &composite_three,
        &composite_pool,
        &rule_pool(&composite_three, &composite_pool),
        &mut rng,
        "rare Composite Ring with 3 prefixes",
    );
    assert_eq!(after.prefixes.len(), 4, "{:?}", outcome(&after));
    assert!(after.suffixes.is_empty(), "{:?}", outcome(&after));
    assert_new_mod_last_of_its_side(&composite_three, &after, &added, "Composite Ring");

    // Given a rare Iron Ring with a prefix and a suffix
    let iron_two = item_with(
        &iron,
        Rarity::Rare,
        &[find(&iron_pool, STOUT)],
        &[find(&iron_pool, PHILOSOPHER)],
        &mut rng,
    );
    // When the orb is applied many times
    // Then it gets one new mod of either side, chosen by weight, and the two stay unchanged
    let sides = assert_exalts_by_weight(
        &iron_two,
        &iron_pool,
        POOL_TRIALS,
        &mut rng,
        "rare Iron Ring with a prefix and a suffix",
    );
    assert_eq!(
        sides.len(),
        2,
        "the Iron Ring gets prefixes and suffixes, found {sides:?}"
    );

    // --- The PO's evidence: the Unset Ring gets "Dancer's" (Tier 6) ---
    // Given a rare Unset Ring with 1 prefix and 2 suffixes of other families than "Dancer's"
    let unset = base(UNSET_RING_ID);
    let unset_pool = pool_of(UNSET_RING_ID);
    let dancers = find(&unset_pool, DANCERS);
    let prefix = unset_pool
        .iter()
        .find(|definition| {
            definition.affix_type == AffixType::Prefix
                && definition.definition.family != dancers.definition.family
        })
        .expect("the Unset Ring has prefixes of other families");
    let first_suffix = unset_pool
        .iter()
        .find(|definition| definition.affix_type == AffixType::Suffix)
        .expect("the Unset Ring has suffixes");
    let second_suffix = unset_pool
        .iter()
        .find(|definition| {
            definition.affix_type == AffixType::Suffix
                && definition.definition.family != first_suffix.definition.family
        })
        .expect("the Unset Ring has suffixes of two families");
    let before = item_with(
        &unset,
        Rarity::Rare,
        &[prefix],
        &[first_suffix, second_suffix],
        &mut rng,
    );
    // When the orb adds "Dancer's"
    let (after, added) = exalt_once(
        &before,
        std::slice::from_ref(dancers),
        &[dancers],
        &mut rng,
        "the PO's evidence",
    );
    // Then the other mods and their values do not change, and the advanced view shows
    // "Dancer's" with Tier 6 as the last prefix
    let block = assert_new_mod_last_of_its_side(&before, &after, &added, "the PO's evidence");
    assert_eq!(
        (block.name.as_deref(), block.tier),
        (Some(DANCERS), Some(DANCERS_TIER)),
        "the PO's evidence in the advanced view"
    );

    // --- Invented pools: each filter of the rule leaves out a mod with a high weight ---
    let mut test_rng = SeededRng::new(TEST_POOL_SEED);
    let held_prefixes = held_mods(AffixType::Prefix, 3);
    let held_suffixes = held_mods(AffixType::Suffix, 3);
    let held_prefix_family = |n: usize| held_prefixes[n].definition.family.clone();
    let held_suffix_family = |n: usize| held_suffixes[n].definition.family.clone();
    let prefixes = |n: usize| held_prefixes.iter().take(n).collect::<Vec<_>>();
    let suffixes = |n: usize| held_suffixes.iter().take(n).collect::<Vec<_>>();
    let mut cases = vec![
        // A rare Iron Ring with 3 prefixes and 2 suffixes has room for a suffix only: 20 % and
        // 80 %; no prefix, no mod of a held family, no mod above the item level, a mod at the
        // item level
        (
            "invented pool, rare Iron Ring with 3 prefixes and 2 suffixes",
            item_with(
                &iron,
                Rarity::Rare,
                &prefixes(3),
                &suffixes(2),
                &mut test_rng,
            ),
            vec![
                test_mod("New Prefix", AffixType::Prefix, "New Prefixes", 9000, 1),
                test_mod(
                    "of the Held Suffix",
                    AffixType::Suffix,
                    &held_suffix_family(1),
                    9000,
                    1,
                ),
                test_mod("of Heights", AffixType::Suffix, "Heights", 9000, 51),
                test_mod("of B", AffixType::Suffix, "B", 1000, 50),
                test_mod("of C", AffixType::Suffix, "C", 4000, 1),
            ],
        ),
        // A rare Iron Ring with 1 prefix and 1 suffix has room on both sides: 30 %, 10 % and
        // 60 %; no mod of a held family, no mod above the item level
        (
            "invented pool, rare Iron Ring with a prefix and a suffix",
            item_with(
                &iron,
                Rarity::Rare,
                &prefixes(1),
                &suffixes(1),
                &mut test_rng,
            ),
            vec![
                test_mod(
                    "Held Prefix 2",
                    AffixType::Prefix,
                    &held_prefix_family(0),
                    9000,
                    1,
                ),
                test_mod(
                    "of the Held Suffix 2",
                    AffixType::Suffix,
                    &held_suffix_family(0),
                    9000,
                    1,
                ),
                test_mod("High Prefix", AffixType::Prefix, "Heights", 9000, 51),
                test_mod("Prefix A", AffixType::Prefix, "A", 3000, 1),
                test_mod("of B", AffixType::Suffix, "B", 1000, 50),
                test_mod("of C", AffixType::Suffix, "C", 6000, 1),
            ],
        ),
        // A rare Composite Ring with 3 prefixes has room for prefixes only: 60 % and 40 %; no
        // suffix, no mod of a held family
        (
            "invented pool, rare Composite Ring with 3 prefixes",
            item_with(&composite, Rarity::Rare, &prefixes(3), &[], &mut test_rng),
            vec![
                test_mod("of the Suffix", AffixType::Suffix, "Suffixes", 9000, 1),
                test_mod(
                    "Held Prefix 2",
                    AffixType::Prefix,
                    &held_prefix_family(2),
                    9000,
                    1,
                ),
                test_mod("Prefix A", AffixType::Prefix, "A", 3000, 1),
                test_mod("Prefix B", AffixType::Prefix, "B", 2000, 1),
            ],
        ),
        // A rare Cogwork Ring (at most 2 prefixes) with 2 prefixes and 1 suffix has room for
        // suffixes only: 75 % and 25 %; no prefix, no mod of a held family
        (
            "invented pool, rare Cogwork Ring with 2 prefixes and a suffix",
            item_with(
                &cogwork,
                Rarity::Rare,
                &prefixes(2),
                &suffixes(1),
                &mut test_rng,
            ),
            vec![
                test_mod("New Prefix", AffixType::Prefix, "New Prefixes", 9000, 1),
                test_mod(
                    "of the Held Suffix 2",
                    AffixType::Suffix,
                    &held_suffix_family(0),
                    9000,
                    1,
                ),
                test_mod("of A", AffixType::Suffix, "A", 3000, 1),
                test_mod("of B", AffixType::Suffix, "B", 1000, 1),
            ],
        ),
    ];
    for (label, item, pool) in &mut cases {
        // Given a rare ring of item level 50 and the invented pool
        item.item_level = 50;
        assert!(
            has_room(item),
            "{label}: the test needs a rare item with room"
        );
        // When the orb is applied many times
        // Then it adds exactly one mod of the valid pool, chosen by weight, and the other mods
        // stay unchanged
        assert_exalts_by_weight(item, pool, TEST_POOL_TRIALS, &mut test_rng, label);
    }

    // --- Every ring base ---
    let rings = Slot::Ring.base_items();
    assert!(!rings.is_empty(), "the slot \"Ring\" must have ring bases");
    for ring in &rings {
        let pool = pool_of(&ring.id);
        for trial in 0..ITEMS_PER_RING {
            // Given a rare ring with 0 to 5 mods and room
            let item = rare_with_room(ring, &pool, (trial % 6) as usize, &mut rng);
            let ctx = format!("rare {} with {:?}", ring.name, outcome(&item));
            // When the orb is applied
            // Then the ring gets one more mod of the valid pool (`valid_pool`, which also
            // applies the lock of the Breach rings), and the other mods stay unchanged
            let expected = valid_pool(&item, &pool);
            let expected: Vec<&ExplicitModDefinition> = expected.iter().collect();
            let (after, added) = exalt_once(&item, &pool, &expected, &mut rng, &ctx);
            // And the advanced view shows it as the last mod of its side
            let views = if VIEW_RINGS.contains(&ring.id.as_str()) {
                0..6
            } else {
                5..6
            };
            if views.contains(&trial) {
                assert_new_mod_last_of_its_side(&item, &after, &added, &ctx);
            }
        }
    }
}

// AC4: „Auf einem normalen und auf einem magischen Item ist der Orb nicht anwendbar. Überfahren
// mit der Maus und Klick zeigen genau die Meldung `Failed to apply item: Target is not Rare.` Das
// Item bleibt unverändert."
// (Tooltip, look and message area are checked by the PO in the local preview, the wording of the
// message at the acceptance. Here: the reason on hover and on click for normal items, magic items
// with 0 to 2 mods (also without room) and, by the PO's rule "the item is not rare", unique
// items, with the ring's pool and with an empty pool; its message from the game data; the
// unchanged item; and that the page marks the orb and shows the message.)
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
            let magic = item_of_rarity(&ring, Rarity::Magic, &pool, (trial % 3) as usize, &mut rng);
            let unique = item_with(&ring, Rarity::Unique, &[], &[], &mut rng);

            for item in [normal, magic, unique] {
                for click_pool in [&pool, &empty] {
                    let ctx = format!(
                        "{:?} {} {trial} with {:?}",
                        item.rarity,
                        ring.name,
                        outcome(&item)
                    );
                    // When the page asks whether the orb applies, and it is clicked anyway
                    // Then both fail because the item is not rare, whatever the pool and the
                    // room, and the item is unchanged
                    assert_not_applicable(
                        &item,
                        click_pool,
                        CraftError::ItemNotRare,
                        &mut rng,
                        &ctx,
                    );
                }
            }
        }
    }
    // And hover and click show the orb's message for this reason from the game data
    let not_rare = assert_message_is_game_data(CraftError::ItemNotRare, NOT_RARE_CODE);
    // And it is not written into the code of crafting-core or web
    assert_only_in_game_data(&not_rare);
    // And it is another message than those of the other two reasons
    assert_reasons_have_own_messages();
    // And the page marks the orb as not applicable and shows the message on hover and click
    assert_web_src_uses(
        &["can_apply_exalted_orb", "failure_message", "aria-disabled"],
        "an inapplicable Exalted Orb is marked and shows its failure message",
    );
    assert_web_src_applies_the_orb();
}

// AC5: „Auf einem seltenen Item ohne Platz ist der Orb nicht anwendbar. Überfahren mit der Maus
// und Klick zeigen genau die Meldung `Failed to apply item: Item has no space for more Mods.` Das
// Item bleibt unverändert. Beispiele:
// - ein seltener Iron Ring mit 3 Prefixes und 3 Suffixes;
// - ein seltener Composite Ring mit 6 Prefixes."
// (Tooltip, look and message area are checked by the PO in the local preview, the wording of the
// message at the acceptance. Here: both examples; every ring base, including the Breach and
// Heist rings with their limits, filled to the limits of a rare item; each with the ring's pool
// and with an empty pool, so the missing room is reported before the pool; with one mod less, the
// orb applies again. And the message from the game data, the unchanged item, and that the page
// shows the message.)
#[test]
fn ac5_the_orb_on_a_rare_item_without_room_shows_the_game_message_and_keeps_the_item() {
    let mut rng = SeededRng::new(SEED);
    let empty: Vec<ExplicitModDefinition> = Vec::new();

    // --- The story's examples ---
    let iron = base(IRON_RING_ID);
    let iron_pool = pool_of(IRON_RING_ID);
    let composite = base(COMPOSITE_RING_ID);
    let composite_pool = pool_of(COMPOSITE_RING_ID);
    // A rare Iron Ring with 3 prefixes and 3 suffixes
    let iron_full = full_rare(&iron, &iron_pool, &mut rng);
    assert_eq!(
        (iron_full.prefixes.len(), iron_full.suffixes.len()),
        (3, 3),
        "the example needs a rare Iron Ring with 3 prefixes and 3 suffixes, found {:?}",
        outcome(&iron_full)
    );
    // A rare Composite Ring with 6 prefixes
    let composite_full = full_rare(&composite, &composite_pool, &mut rng);
    assert_eq!(
        (composite_full.prefixes.len(), composite_full.suffixes.len()),
        (6, 0),
        "the example needs a rare Composite Ring with 6 prefixes, found {:?}",
        outcome(&composite_full)
    );
    let mut examples = vec![(iron_full, iron_pool), (composite_full, composite_pool)];

    // --- Every ring base, filled to the limits of a rare item ---
    let rings = Slot::Ring.base_items();
    assert!(!rings.is_empty(), "the slot \"Ring\" must have ring bases");
    for ring in &rings {
        let pool = pool_of(&ring.id);
        for _ in 0..ITEMS_PER_RING {
            examples.push((full_rare(ring, &pool, &mut rng), pool.clone()));
        }
    }

    for (item, pool) in &examples {
        let ctx = format!("rare {} with {:?}", item.base.name, outcome(item));
        // Given a rare item on which neither side has room under both affix limits
        assert!(
            !has_room(item),
            "{ctx}: the test needs an item without room, limits {:?}",
            limits(item)
        );
        for click_pool in [pool, &empty] {
            // When the page asks whether the orb applies, and it is clicked anyway
            // Then both fail because the item has no space for more mods, whatever the pool,
            // and the item is unchanged
            assert_not_applicable(item, click_pool, CraftError::NoSpaceForMods, &mut rng, &ctx);
        }
        // And with one mod less, the side of that mod has room again and the orb applies
        let mut one_less = item.clone();
        if one_less.suffixes.pop().is_none() {
            one_less.prefixes.pop().expect("a full rare ring has a mod");
        }
        assert_eq!(
            can_apply_exalted_orb(&one_less, pool),
            Ok(()),
            "{ctx}: with {:?} the orb applies",
            outcome(&one_less)
        );
    }

    // And hover and click show the orb's message for this reason from the game data
    let no_space = assert_message_is_game_data(CraftError::NoSpaceForMods, NO_SPACE_CODE);
    // And it is not written into the code of crafting-core or web
    assert_only_in_game_data(&no_space);
    // And it is another message than those of the other two reasons
    assert_reasons_have_own_messages();
    // And the page shows the message on hover and click
    assert_web_src_uses(
        &["can_apply_exalted_orb", "failure_message", "aria-disabled"],
        "an Exalted Orb without room is marked and shows its failure message",
    );
    assert_web_src_applies_the_orb();
}

// AC6: „Auf einem seltenen Item mit Platz, aber leerem gültigem Pool, kommt kein Mod hinzu.
// Überfahren mit der Maus und Klick zeigen genau die Meldung `Failed to apply item: Could not
// generate Mod.` Das Item bleibt unverändert. Beispiel: ein seltener Composite Ring auf
// Item-Level 1 mit 4 Prefixes. Auf Item-Level 1 hat seine Basis nur 4 Prefix-Familien."
// (Tooltip, look and message area are checked by the PO in the local preview, the wording of the
// message at the acceptance. Here: the example on item level 1 with 4 prefixes and, by the PO's
// rule, on item level 2 with 5 prefixes, each with other prefixes; invented pools in which every
// mod is left out by a filter of the rule, and an empty pool; with one valid mod more, the orb
// applies. And the message from the game data, the unchanged item, and that the page asks
// crafting-core with the base's pool on hover.)
#[test]
fn ac6_the_orb_on_a_rare_item_with_an_empty_valid_pool_shows_the_game_message() {
    let mut rng = SeededRng::new(SEED);
    let empty: Vec<ExplicitModDefinition> = Vec::new();
    let composite = base(COMPOSITE_RING_ID);
    let composite_pool = pool_of(COMPOSITE_RING_ID);

    // --- The story's example and the PO's rule ---
    for (item_level, families) in SMALL_POOL_LEVELS {
        for trial in 0..SMALL_POOL_ITEMS {
            // Given a rare Composite Ring of item level 1 (or 2), filled with prefixes until the
            // rule adds no more
            let mut item = item_with(&composite, Rarity::Rare, &[], &[], &mut rng);
            item.item_level = item_level;
            while add_random_mod(&mut item, &composite_pool, &mut rng) {}
            let ctx = format!(
                "rare Composite Ring of item level {item_level}, {trial}, with {:?}",
                outcome(&item)
            );
            // It has one prefix per family of its base on that item level, and room
            assert_eq!(
                (item.prefixes.len(), item.suffixes.len()),
                (families, 0),
                "{ctx}: the example needs {families} prefixes"
            );
            assert!(
                side_has_room(&item, AffixType::Prefix),
                "{ctx}: the test needs a rare item with room"
            );
            // And its valid pool is empty
            assert!(
                rule_pool(&item, &composite_pool).is_empty(),
                "{ctx}: the rule leaves no mod"
            );
            assert!(
                valid_pool(&item, &composite_pool).is_empty(),
                "{ctx}: valid_pool leaves no mod"
            );
            // When the page asks whether the orb applies, and it is clicked anyway
            // Then both fail because no mod could be generated, and no mod is added
            assert_not_applicable(
                &item,
                &composite_pool,
                CraftError::NoValidMod,
                &mut rng,
                &ctx,
            );
            // And on a higher item level, with more prefix families, the orb applies again
            let mut higher = item.clone();
            higher.item_level = DEFAULT_ITEM_LEVEL;
            assert_eq!(
                can_apply_exalted_orb(&higher, &composite_pool),
                Ok(()),
                "{ctx}: on item level {DEFAULT_ITEM_LEVEL} the orb applies"
            );
        }
    }

    // --- Invented pools: every mod is left out by a filter of the rule ---
    let iron = base(IRON_RING_ID);
    let held_prefixes = held_mods(AffixType::Prefix, 2);
    let held_suffixes = held_mods(AffixType::Suffix, 3);
    let valid_extra = test_mod("Prefix A", AffixType::Prefix, "A", 1000, 1);
    let mut cases = vec![
        // A rare Iron Ring with 1 prefix and 3 suffixes: prefixes of the held family or above
        // the item level, suffixes without room
        (
            "invented pool, rare Iron Ring with a prefix and 3 suffixes",
            item_with(
                &iron,
                Rarity::Rare,
                &[&held_prefixes[0]],
                &held_suffixes.iter().collect::<Vec<_>>(),
                &mut rng,
            ),
            vec![
                test_mod(
                    "Held Prefix 2",
                    AffixType::Prefix,
                    &held_prefixes[0].definition.family,
                    1000,
                    1,
                ),
                test_mod("High Prefix", AffixType::Prefix, "Heights", 1000, 51),
                test_mod("of the New Suffix", AffixType::Suffix, "New", 1000, 1),
            ],
        ),
        // A rare Composite Ring with 2 prefixes: only suffixes, for which it has no room
        (
            "invented pool, rare Composite Ring with 2 prefixes",
            item_with(
                &composite,
                Rarity::Rare,
                &held_prefixes.iter().collect::<Vec<_>>(),
                &[],
                &mut rng,
            ),
            vec![test_mod(
                "of the Suffix",
                AffixType::Suffix,
                "Suffixes",
                1000,
                1,
            )],
        ),
        // A rare Iron Ring with a prefix and a suffix and an empty pool
        (
            "empty pool, rare Iron Ring with a prefix and a suffix",
            item_with(
                &iron,
                Rarity::Rare,
                &[&held_prefixes[0]],
                &[&held_suffixes[0]],
                &mut rng,
            ),
            empty.clone(),
        ),
    ];
    for (label, item, pool) in &mut cases {
        // Given a rare ring of item level 50 with room and a pool without valid mods
        item.item_level = 50;
        assert!(
            has_room(item),
            "{label}: the test needs a rare item with room"
        );
        assert!(
            rule_pool(item, pool).is_empty(),
            "{label}: the rule leaves no mod"
        );
        // When the page asks whether the orb applies, and it is clicked anyway
        // Then both fail because no mod could be generated, and no mod is added
        assert_not_applicable(item, pool, CraftError::NoValidMod, &mut rng, label);
        // And with one valid mod more in the pool, the orb applies
        let mut with_valid = pool.clone();
        with_valid.push(valid_extra.clone());
        assert_eq!(
            can_apply_exalted_orb(item, &with_valid),
            Ok(()),
            "{label}: with {:?} in the pool the orb applies",
            valid_extra.name
        );
    }

    // And hover and click show the orb's message for this reason from the game data
    let no_valid_mod = assert_message_is_game_data(CraftError::NoValidMod, NO_VALID_MOD_CODE);
    // And it is not written into the code of crafting-core or web
    assert_only_in_game_data(&no_valid_mod);
    // And it is another message than those of the other two reasons
    assert_reasons_have_own_messages();
    // And the page asks crafting-core with the base's pool, so hover already shows the message
    assert_web_src_uses(
        &["can_apply_exalted_orb", "failure_message", "aria-disabled"],
        "an Exalted Orb with an empty valid pool is marked and shows its failure message",
    );
    assert_web_src_applies_the_orb();
}
