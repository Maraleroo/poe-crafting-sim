//! Acceptance tests for AB#299 "Regal Orb auf magische Items anwenden".
//!
//! The tests use the public interface of `crafting-core` from the tech lead's plan on AB#299:
//! - `Currency::RegalOrb` with the id `regal_orb`; its name, description and messages are only
//!   in `data/currencies.json`.
//! - `can_apply_regal_orb(item) -> Result<(), CraftError>`: `ItemNotMagic` (code
//!   `item_not_magic`) for a normal, rare or unique item; `NoSpaceForMods` for a magic item on
//!   which neither side has room under the affix limits of a rare item. The rarity is checked
//!   first.
//! - `apply_regal_orb(item, pool, rng) -> Result<(), CraftError>`: `pool` is the base's pool;
//!   first the same checks, then the item becomes rare and exactly one `add_random_mod` adds the
//!   new mod at the end of its side.
//! - `Item::name()`: for a rare item only the base name; unchanged for the other rarities.
//! - unchanged: `currencies`, `Currency::{id, name, description, failure_message}`,
//!   `CraftError::code`, `add_random_mod`, `valid_pool`, `mod_pool`, `base_item`,
//!   `Slot::Ring.base_items()`, `Item::{affix_limits, explicit_texts, advanced_mods, tier}`,
//!   the other three orbs, `Mod::roll`, `SeededRng`. Following ADR 0004, web criteria are
//!   checked through the file contents under `web/` (without the build output in `web/dist` and
//!   `web/target`).
//!
//! The PO's rules are `poe-crafting-rules`: `currencies/regal-orb.md` (committed in a8a4637,
//! before these tests), "Affix limits" in "Item basics", "Advanced mod descriptions" and "Adding
//! a random mod". The decision on the names of rare items is the PO's, in the story.
//!
//! Criterion 1 gives the PO's wording for `poe-crafting-rules`; the wording is checked byte for
//! byte at the acceptance (step 2 of the `story` skill), so its test checks only that the file
//! exists and is linked. Its behaviour is checked by the tests of criteria 3 to 6. Criterion 7
//! has no test (PO decision on the story and in the task): with the ring bases in the data, a
//! magic item always has room under the limits of a rare item and a valid mod; the tech lead
//! covers both cases with unit tests.
//!
//! Game texts (the orb's name, its description and its messages) and UI texts are not compared
//! word for word here (`writing-acceptance-tests`, "What permanent tests check"): their wording is
//! checked at the acceptance. The tests check that crafting-core shows the texts of the orbs'
//! entries in `data/currencies.json` under the reason codes of the plan, and that the Regal Orb's
//! texts are not written into the code of `crafting-core` or `web`.
//!
//! Every random test uses a fixed seed. Statistical tests compare observed shares with the
//! shares that follow from the weights and the rule, with a tolerance of 5 standard deviations.
//! The tests depend only on frequencies and invariants, never on the order in which the
//! generator is used.
//!
//! Red state before the implementation: the module does not compile, because
//! `Currency::RegalOrb`, `can_apply_regal_orb` and `apply_regal_orb` do not exist yet.
//!
//! What is checked outside `cargo test`, by the PO in the local preview:
//! - AC2: the fourth button next to the other orbs and its tooltip.
//! - AC4: the yellow colour of the rare item's name and how the header looks.
//! - AC5: how the inapplicable orb looks, its tooltip and the message after a click.
//! - AC6: the tooltips and messages of the other orbs on a rare item in the page.
//!
//! Not testable with today's game data: the PO's evidence of a magic Focused Amulet, because the
//! simulator has no amulets. The Composite Ring (no suffix under the limits of a rare item) and
//! the Cogwork Ring (2 prefixes and 4 suffixes) stand in for changed limits.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crafting_core::{
    add_random_mod, apply_orb_of_alteration, apply_orb_of_augmentation, apply_orb_of_transmutation,
    apply_regal_orb, base_item, can_apply_orb_of_alteration, can_apply_orb_of_augmentation,
    can_apply_orb_of_transmutation, can_apply_regal_orb, currencies, mod_pool, valid_pool,
    AffixLimits, AffixType, BaseItem, CraftError, Currency, CurrencyDefinition, ExplicitMod,
    ExplicitModDefinition, Item, Mod, ModBlock, ModDefinition, ModKind, Rarity, SeededRng, Slot,
    StatDefinition, DEFAULT_ITEM_LEVEL,
};
use serde_json::Value;

// ---------------------------------------------------------------------------------------
// Values from the story, the plan and the PO's rules
// ---------------------------------------------------------------------------------------

const IRON_RING_ID: &str = "Metadata/Items/Rings/Ring1";
/// "+3 Prefix Modifiers allowed", "-3 Suffix Modifiers allowed" (AB#189).
const COMPOSITE_RING_ID: &str = "Metadata/Items/Rings/RingE3";
/// "-1 Prefix Modifier allowed", "+1 Suffix Modifier allowed" (AB#189).
const COGWORK_RING_ID: &str = "Metadata/Items/Rings/RingE1";
/// The base name of the example in criterion 4.
const COMPOSITE_RING_NAME: &str = "Composite Ring";

const ORB_ID: &str = "regal_orb";
const TRANSMUTATION_ID: &str = "orb_of_transmutation";
const ALTERATION_ID: &str = "orb_of_alteration";
const AUGMENTATION_ID: &str = "orb_of_augmentation";

/// Reason codes from the tech lead's plan and ADR 0012, the keys of `messages`.
const NOT_MAGIC_CODE: &str = "item_not_magic";
const NOT_NORMAL_CODE: &str = "item_not_normal";

/// Mods for the Iron Ring of criterion 3 (a prefix and a suffix).
const STOUT: &str = "Stout";
const PHILOSOPHER: &str = "of the Philosopher";
/// The PO's evidence in `currencies/regal-orb.md`: a magic Composite Ring with "Incandescent"
/// (Tier 2) and "Dragon's" (Tier 2) became rare with the new prefix "Glinting" (Tier 6).
const INCANDESCENT: &str = "Incandescent";
const DRAGONS: &str = "Dragon's";
const GLINTING: &str = "Glinting";
const EVIDENCE_TIERS: [(&str, u32); 3] = [(INCANDESCENT, 2), (DRAGONS, 2), (GLINTING, 6)];

const CURRENCIES_JSON: &str = "data/currencies.json";
const CRAFTING_RULES: &str = ".claude/skills/poe-crafting-rules/SKILL.md";
const ORB_RULE_FILE: &str = ".claude/skills/poe-crafting-rules/currencies/regal-orb.md";
const ORB_RULE_LINK: &str = "currencies/regal-orb.md";
const CURRENCY_RULES_HEADING: &str = "Currency rules:";
const TRANSLATIONS: &str = "web/i18n/en.json";
const RARE_KEY: &str = "item.rarity.rare";

/// Fixed seeds, so every run is identical.
const SEED: u64 = 299;
const STATISTICS_SEED: u64 = 29_901;
const TEST_POOL_SEED: u64 = 29_902;

/// Items for invariant checks.
const ITEMS: u64 = 300;
/// Magic items per ring base (AC3, AC4): without mods, with one mod and filled to the magic
/// limits, in turn.
const ITEMS_PER_RING: u64 = 30;
/// Of these, the items whose two views are checked (AC4): on the Iron, Composite and Cogwork
/// Ring one of each kind, on every other ring base the one filled to its magic limits. Each
/// advanced view reads the base's pool from the game data, so checking the views of every item
/// would take longer than a few seconds.
const VIEW_RINGS: [&str; 3] = [IRON_RING_ID, COMPOSITE_RING_ID, COGWORK_RING_ID];
/// Orbs on a ring with the ring's whole pool (AC3). The valid pools hold about 220 mods of both
/// sides or about 85 prefixes, weighted 50 to 2000. A uniform choice gives the mods of weight
/// 1000 or 2000 clearly other shares than their weights; 5 standard deviations of a share are at
/// most 0.025, so it fails clearly. Each orb builds the valid pool of about 230 mods, so more orbs
/// would take longer than a few seconds.
const POOL_TRIALS: u64 = 10_000;
/// Orbs per invented pool (AC3). 5 standard deviations of a share are at most 0.018, so a
/// uniform choice (for example 33 % instead of 60 %) or a coin flip between the sides (50 %
/// instead of 30 % or 75 %) fails clearly.
const TEST_POOL_TRIALS: u64 = 20_000;

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

/// The text of the entry of the currency `id` in `data/currencies.json` at the given path of
/// keys. A game text is never empty.
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

/// Asserts that `currency` shows the game data's message for `error`: `code()` is the plan's
/// reason code, and `failure_message` is the text under that code in `currencies()` and in
/// `data/currencies.json`. Returns the message.
fn assert_message_is_game_data(currency: Currency, error: CraftError, code: &str) -> String {
    let id = currency.id();
    assert_eq!(error.code(), code, "the reason code of {error:?}");
    let in_file = text_in_file(id, &["messages", code]);
    assert_eq!(
        definition(id).messages.get(code).map(String::as_str),
        Some(in_file.as_str()),
        "currencies(): message of {id:?} for {code:?}"
    );
    assert_eq!(
        currency.failure_message(error),
        in_file,
        "the message of {id:?} for {error:?} is the game data's"
    );
    in_file
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

fn rolled(definition: &ExplicitModDefinition, rng: &mut SeededRng) -> ExplicitMod {
    ExplicitMod {
        name: definition.name.clone(),
        affix_type: definition.affix_type,
        modifier: Mod::roll(&definition.definition, rng),
    }
}

/// A new item of `base` with the given rarity, prefixes and suffixes, rolled from the
/// definitions, in this order.
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

/// A magic item of `base` at the default item level with `mods` random mods of `pool`, or as
/// many as the limits of a magic item allow.
fn magic_item(
    base: &BaseItem,
    pool: &[ExplicitModDefinition],
    mods: usize,
    rng: &mut SeededRng,
) -> Item {
    let mut item = item_with(base, Rarity::Magic, &[], &[], rng);
    item.item_level = DEFAULT_ITEM_LEVEL;
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

fn names<'a>(pool: impl IntoIterator<Item = &'a ExplicitModDefinition>) -> Vec<String> {
    pool.into_iter()
        .map(|definition| definition.name.clone())
        .collect()
}

/// The same item as a rare item: its limits are the limits of a rare item, changed by the lines
/// of its base ("Affix limits").
fn as_rare(item: &Item) -> Item {
    let mut rare = item.clone();
    rare.rarity = Rarity::Rare;
    rare
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

/// Step 1 of "Adding a random mod" for a base without a lock (no Breach ring): every mod of
/// `pool` whose level is at most the item level, whose side has room under both affix limits of
/// the item's rarity, and whose family is not already on the item, in the order of `pool`.
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

/// Asserts that `after` is the magic item `before` made rare, with exactly one more mod at the
/// end of its side and all other mods unchanged in value and order, and returns that mod.
fn assert_upgraded_with_one_mod(before: &Item, after: &Item, ctx: &str) -> ExplicitMod {
    assert_eq!(before.rarity, Rarity::Magic, "{ctx}: the test starts magic");
    assert_eq!(after.rarity, Rarity::Rare, "{ctx}: the item becomes rare");
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

/// Applies the orb to a copy of the magic `item`, checks the rule for one result and returns the
/// rare item and the new mod: the orb applies, the item becomes rare, exactly one mod of
/// `expected` (the valid pool under the limits of a rare item) is added at the end of its side,
/// rolled from its definition, and nothing else changes.
fn regal_once(
    item: &Item,
    pool: &[ExplicitModDefinition],
    expected: &[&ExplicitModDefinition],
    rng: &mut SeededRng,
    ctx: &str,
) -> (Item, ExplicitMod) {
    let mut after = item.clone();
    assert_eq!(
        can_apply_regal_orb(&after),
        Ok(()),
        "{ctx}: the orb applies to a magic item, found {:?}",
        outcome(item)
    );
    let result = apply_regal_orb(&mut after, pool, rng);
    assert_eq!(result, Ok(()), "{ctx}: the orb takes effect");
    let added = assert_upgraded_with_one_mod(item, &after, ctx);
    let definition = expected
        .iter()
        .find(|definition| definition.name == added.name)
        .unwrap_or_else(|| {
            panic!(
                "{ctx}: {:?} is not in the valid pool under the limits of a rare item {:?}",
                added.name,
                names(expected.iter().copied())
            )
        });
    assert_rolled_from(definition, &added, ctx);
    (after, added)
}

/// A rare item made by the orb from a magic item of `base` with `mods` mods (AC5, AC6).
fn rare_by_orb(
    base: &BaseItem,
    pool: &[ExplicitModDefinition],
    mods: usize,
    rng: &mut SeededRng,
) -> Item {
    let mut item = magic_item(base, pool, mods, rng);
    assert_eq!(
        apply_regal_orb(&mut item, pool, rng),
        Ok(()),
        "the orb makes the magic {} rare",
        base.name
    );
    assert_eq!(item.rarity, Rarity::Rare);
    item
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

/// Applies the orb `trials` times to copies of the magic `item` and asserts that the new mod is
/// chosen from the valid pool of the rule under the limits of a rare item, each mod with the
/// chance of its weight divided by the sum of the weights in the pool. Also checks the shares of
/// the mods of each weight and of each side, so that a choice that ignores the weights or flips
/// a coin between the sides fails clearly. Returns how often each side got the new mod.
fn assert_regals_by_weight(
    item: &Item,
    pool: &[ExplicitModDefinition],
    trials: u64,
    rng: &mut SeededRng,
    label: &str,
) -> BTreeMap<String, u64> {
    let rare = as_rare(item);
    let expected = rule_pool(&rare, pool);
    assert!(
        !expected.is_empty(),
        "{label}: the test needs a valid pool with mods"
    );
    // The rule has one place (AB#248): the valid pool of crafting-core is the rule's.
    assert_eq!(
        names(&valid_pool(&rare, pool)),
        names(expected.iter().copied()),
        "{label}: valid pool under the limits of a rare item"
    );
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    for trial in 0..trials {
        let (_, added) = regal_once(item, pool, &expected, rng, &format!("{label}, {trial}"));
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

/// Asserts that both views of the rare item `after` follow the existing rules, which do not
/// depend on the rarity, and that the advanced view shows `added` as the last mod of its side,
/// after the blocks `before` already showed.
fn assert_views_follow_the_rules(before: &Item, after: &Item, added: &ExplicitMod, ctx: &str) {
    // The normal view shows the same as for the same mods on an item of another rarity
    let mut as_magic = after.clone();
    as_magic.rarity = Rarity::Magic;
    assert_eq!(
        after.explicit_texts(),
        as_magic.explicit_texts(),
        "{ctx}: the normal view follows the existing rules"
    );
    let blocks = after.advanced_mods();

    // The advanced view: implicits, prefixes, suffixes
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
    let before_blocks = before.advanced_mods();
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
    let last = side.last().expect("one block was added");
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
}

// ---------------------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------------------

// AC1: „Regel des PO, Wortlaut für `poe-crafting-rules` (verbindlich): Die neue Datei
// `currencies/regal-orb.md` hat genau den Inhalt unten und wird in der Liste „Currency rules“
// verlinkt."
// (The wording is checked byte for byte at the acceptance, step 2 of the `story` skill. Here:
// the file exists and the list "Currency rules" links it. The behaviour of the rule is checked
// by AC3 to AC6.)
#[test]
fn ac1_regal_orb_rule_file_exists_and_is_linked() {
    // Given the PO's rules
    let rules = read_text(CRAFTING_RULES);

    // When reading the rule file of the Regal Orb
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

// AC2: „Neben den bisherigen Orbs gibt es einen Regal Orb. Sein Name ist ein Spieltext. Ist der
// Orb anwendbar, zeigt das Überfahren mit der Maus genau den Text `Upgrades a magic item to a
// rare item` (Spieldaten)."
// (The button's place and the tooltip are checked by the PO in the local preview; the wording of
// the name and the description at the acceptance. Here: the four orbs exist, the new orb's name
// and description are the texts of its entry in the game data and are not written into the
// code, the orb applies to magic items, and the page uses the new orb from crafting-core and
// shows its description.)
#[test]
fn ac2_regal_orb_exists_with_its_game_name_and_description() {
    // Given the currencies of crafting-core
    let ids: Vec<String> = currencies().into_iter().map(|entry| entry.id).collect();

    // Then the Regal Orb exists next to the other orbs
    for id in [TRANSMUTATION_ID, ALTERATION_ID, AUGMENTATION_ID, ORB_ID] {
        assert!(
            ids.iter().any(|found| found == id),
            "currencies() must contain {id:?}, found {ids:?}"
        );
    }
    assert_eq!(Currency::RegalOrb.id(), ORB_ID);

    // And its name is the game text of its entry in the game data
    let name = text_in_file(ORB_ID, &["name"]);
    assert_eq!(Currency::RegalOrb.name(), name);
    assert_eq!(definition(ORB_ID).name, name);
    // And it is not written into the code of crafting-core or web
    assert_only_in_game_data(&name);

    // And its description is the game text of its entry in the game data
    let description = text_in_file(ORB_ID, &["description"]);
    assert_eq!(Currency::RegalOrb.description(), description);
    assert_eq!(definition(ORB_ID).description, description);
    // And it is not written into the code of crafting-core or web
    assert_only_in_game_data(&description);

    // Given magic Iron Rings, Composite Rings and Cogwork Rings: without mods, with one mod and
    // filled to the limits of a magic item
    let mut rng = SeededRng::new(SEED);
    for ring_id in [IRON_RING_ID, COMPOSITE_RING_ID, COGWORK_RING_ID] {
        let ring = base(ring_id);
        let pool = pool_of(ring_id);
        for trial in 0..ITEMS {
            let item = magic_item(&ring, &pool, (trial % 3) as usize, &mut rng);
            // When the page asks whether the orb applies
            // Then it does, so the tooltip is the description
            assert_eq!(
                can_apply_regal_orb(&item),
                Ok(()),
                "magic {} with {:?}",
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
            "can_apply_regal_orb",
            "apply_regal_orb",
            "description()",
        ],
        "the Regal Orb's button comes from crafting-core, next to the other orbs, and its \
         tooltip is its description",
    );
}

// AC3: „Auf einem magischen Item wird das Item selten. Alle vorhandenen Mods bleiben mit ihren
// Werten unverändert. Genau ein Mod kommt nach Kriterium 1 hinzu, unter den Grenzen eines
// seltenen Items. Beispiele:
// - Ein magischer Iron Ring mit Prefix und Suffix wird ein seltener Ring mit 3 Mods.
// - Ein magischer Composite Ring mit 2 Prefixes wird immer ein seltener Ring mit 3 Prefixes und
//   0 Suffixes."
// (Covered: both examples with the ring's whole pool from the game data, by weight and by the
// rule "Adding a random mod" under the limits of a rare item, the Composite Ring with the PO's
// evidence "Incandescent" and "Dragon's"; invented pools in which every filter of the rule leaves
// out a mod with a high weight, on the Iron Ring, the Composite Ring and the Cogwork Ring; and
// every ring base, including the Breach and Heist rings, without mods, with one mod and filled to
// the limits of a magic item.)
#[test]
fn ac3_the_orb_makes_a_magic_item_rare_and_adds_exactly_one_mod_by_the_rule() {
    let mut rng = SeededRng::new(STATISTICS_SEED);
    let iron = base(IRON_RING_ID);
    let iron_pool = pool_of(IRON_RING_ID);
    let composite = base(COMPOSITE_RING_ID);
    let composite_pool = pool_of(COMPOSITE_RING_ID);
    let cogwork = base(COGWORK_RING_ID);

    // --- The story's examples with the game data ---
    // Given a magic Iron Ring with a prefix and a suffix
    let iron_full = item_with(
        &iron,
        Rarity::Magic,
        &[find(&iron_pool, STOUT)],
        &[find(&iron_pool, PHILOSOPHER)],
        &mut rng,
    );
    // When the orb is applied many times
    // Then it becomes a rare ring with 3 mods: one new mod of either side, chosen by weight under
    // the limits of a rare item, and the prefix and the suffix stay unchanged
    let sides = assert_regals_by_weight(
        &iron_full,
        &iron_pool,
        POOL_TRIALS,
        &mut rng,
        "Iron Ring with a prefix and a suffix",
    );
    assert_eq!(
        sides.len(),
        2,
        "the Iron Ring gets prefixes and suffixes, found {sides:?}"
    );

    // Given a magic Composite Ring with 2 prefixes, the PO's evidence
    let composite_full = item_with(
        &composite,
        Rarity::Magic,
        &[
            find(&composite_pool, INCANDESCENT),
            find(&composite_pool, DRAGONS),
        ],
        &[],
        &mut rng,
    );
    // When the orb is applied many times
    // Then it always becomes a rare ring with 3 prefixes and 0 suffixes, the new prefix chosen
    // by weight, and the two prefixes stay unchanged
    let sides = assert_regals_by_weight(
        &composite_full,
        &composite_pool,
        POOL_TRIALS,
        &mut rng,
        "Composite Ring with 2 prefixes",
    );
    assert_eq!(
        sides.keys().collect::<Vec<_>>(),
        ["Prefix"],
        "the Composite Ring always gets a prefix, found {sides:?}"
    );
    assert!(
        composite_pool
            .iter()
            .any(|definition| definition.affix_type == AffixType::Suffix),
        "the Composite Ring's pool must hold suffixes, so the limits of a rare item are checked"
    );

    // --- Invented pools: each filter of the rule leaves out a mod with a high weight ---
    let mut test_rng = SeededRng::new(TEST_POOL_SEED);
    let held_prefix = test_mod("Held Prefix", AffixType::Prefix, "Held Prefixes", 1000, 1);
    let other_prefix = test_mod("Other Prefix", AffixType::Prefix, "Other Prefixes", 1000, 1);
    let held_suffix = test_mod(
        "of the Held Suffix",
        AffixType::Suffix,
        "Held Suffixes",
        1000,
        1,
    );
    let other_suffix = test_mod(
        "of the Other Suffix",
        AffixType::Suffix,
        "Other Suffixes",
        1000,
        1,
    );
    // An Iron Ring with a prefix and a suffix: 30 %, 10 % and 60 %; no mod of a held family, no
    // mod above the item level
    let iron_test_pool = vec![
        test_mod("Held Prefix 2", AffixType::Prefix, "Held Prefixes", 9000, 1),
        test_mod(
            "of the Held Suffix 2",
            AffixType::Suffix,
            "Held Suffixes",
            9000,
            1,
        ),
        test_mod("High Prefix", AffixType::Prefix, "Heights", 9000, 51),
        test_mod("Prefix A", AffixType::Prefix, "A", 3000, 1),
        test_mod("of B", AffixType::Suffix, "B", 1000, 50),
        test_mod("of C", AffixType::Suffix, "C", 6000, 1),
    ];
    // A Composite Ring with 2 prefixes has room for prefixes only: 60 % and 40 %; no suffix, no
    // mod of a held family
    let composite_test_pool = vec![
        test_mod("of the Suffix", AffixType::Suffix, "Suffixes", 9000, 1),
        test_mod("Held Prefix 2", AffixType::Prefix, "Held Prefixes", 9000, 1),
        test_mod("Prefix A", AffixType::Prefix, "A", 3000, 1),
        test_mod("Prefix B", AffixType::Prefix, "B", 2000, 1),
    ];
    // A Cogwork Ring with 2 suffixes has room on both sides: 20 % and 80 %; no mod of a held
    // family
    let cogwork_test_pool = vec![
        test_mod(
            "of the Held Suffix 2",
            AffixType::Suffix,
            "Held Suffixes",
            9000,
            1,
        ),
        test_mod("Prefix A", AffixType::Prefix, "A", 1000, 1),
        test_mod("of B", AffixType::Suffix, "B", 4000, 1),
    ];
    let mut cases = vec![
        (
            "invented pool, Iron Ring with a prefix and a suffix",
            item_with(
                &iron,
                Rarity::Magic,
                &[&held_prefix],
                &[&held_suffix],
                &mut test_rng,
            ),
            iron_test_pool,
        ),
        (
            "invented pool, Composite Ring with 2 prefixes",
            item_with(
                &composite,
                Rarity::Magic,
                &[&held_prefix, &other_prefix],
                &[],
                &mut test_rng,
            ),
            composite_test_pool,
        ),
        (
            "invented pool, Cogwork Ring with 2 suffixes",
            item_with(
                &cogwork,
                Rarity::Magic,
                &[],
                &[&held_suffix, &other_suffix],
                &mut test_rng,
            ),
            cogwork_test_pool,
        ),
    ];
    for (label, item, pool) in &mut cases {
        // Given a magic ring of item level 50 filled to the limits of a magic item, and the
        // invented pool
        item.item_level = 50;
        assert!(
            !side_has_room(item, AffixType::Prefix) && !side_has_room(item, AffixType::Suffix),
            "{label}: the test needs a magic item without room as a magic item"
        );
        // When the orb is applied many times
        // Then it adds exactly one mod of the valid pool under the limits of a rare item, chosen
        // by weight, and the other mods stay unchanged
        assert_regals_by_weight(item, pool, TEST_POOL_TRIALS, &mut test_rng, label);
    }

    // --- Every ring base ---
    let rings = Slot::Ring.base_items();
    assert!(!rings.is_empty(), "the slot \"Ring\" must have ring bases");
    for ring in &rings {
        let pool = pool_of(&ring.id);
        for trial in 0..ITEMS_PER_RING {
            // Given a magic ring without mods, with one mod or filled to its magic limits
            let item = magic_item(ring, &pool, (trial % 3) as usize, &mut rng);
            let ctx = format!("magic {} with {:?}", ring.name, outcome(&item));
            // When the orb is applied
            // Then the ring becomes rare with one more mod of the valid pool under the limits of
            // a rare item (`valid_pool`, which also applies the lock of the Breach rings)
            let expected = valid_pool(&as_rare(&item), &pool);
            let expected: Vec<&ExplicitModDefinition> = expected.iter().collect();
            let (after, _) = regal_once(&item, &pool, &expected, &mut rng, &ctx);
            assert_eq!(
                explicits(&after).len(),
                explicits(&item).len() + 1,
                "{ctx}: {:?}",
                outcome(&after)
            );
        }
    }
}

// AC4: „Ein seltenes Item zeigt im Kopf nur den Namen seines Basistyps, zum Beispiel „Composite
// Ring“, in Gelb wie seltene Items im Spiel. Die Rarity-Zeile zeigt „Rare“. Einen Zufallsnamen
// gibt es nicht (Entscheidung des PO). Die Mods zeigen beide Ansichten nach den bestehenden
// Regeln. In der Advanced-Ansicht steht der neue Mod als letzter seiner Seite."
// (The colour and the header are checked by the PO in the local preview, the wording of the
// rarity line at the acceptance. Here: the PO's evidence with its tiers; the name, both views
// and the place of the new mod after the orb on every ring base; rare items with up to 6 mods;
// and that the page shows the item's name and the translated rarity.)
#[test]
fn ac4_a_rare_item_shows_its_base_name_and_the_new_mod_last_of_its_side() {
    let mut rng = SeededRng::new(SEED);

    // --- The PO's evidence ---
    // Given a magic Composite Ring with "Incandescent" and "Dragon's"
    let composite = base(COMPOSITE_RING_ID);
    let composite_pool = pool_of(COMPOSITE_RING_ID);
    let before = item_with(
        &composite,
        Rarity::Magic,
        &[
            find(&composite_pool, INCANDESCENT),
            find(&composite_pool, DRAGONS),
        ],
        &[],
        &mut rng,
    );
    // When the orb adds "Glinting"
    let glinting = find(&composite_pool, GLINTING);
    let mut item = before.clone();
    let result = apply_regal_orb(&mut item, std::slice::from_ref(glinting), &mut rng);
    assert_eq!(result, Ok(()), "the orb adds {GLINTING:?}");
    let added = assert_upgraded_with_one_mod(&before, &item, "the PO's evidence");
    // Then the header shows only the base name, "Composite Ring"
    assert_eq!(item.name(), COMPOSITE_RING_NAME, "{:?}", outcome(&item));
    assert_eq!(item.name(), composite.name);
    // And the advanced view shows the three prefixes in this order, with the PO's tiers
    let prefixes: Vec<(Option<String>, Option<u32>)> =
        blocks_of(&item.advanced_mods(), ModKind::Prefix)
            .into_iter()
            .map(|block| (block.name, block.tier))
            .collect();
    let expected: Vec<(Option<String>, Option<u32>)> = EVIDENCE_TIERS
        .iter()
        .map(|(name, tier)| (Some((*name).to_owned()), Some(*tier)))
        .collect();
    assert_eq!(prefixes, expected, "the PO's evidence in the advanced view");
    assert_views_follow_the_rules(&before, &item, &added, "the PO's evidence");

    // --- Every ring base ---
    let rings = Slot::Ring.base_items();
    assert!(!rings.is_empty(), "the slot \"Ring\" must have ring bases");
    for ring in &rings {
        let pool = pool_of(&ring.id);
        for trial in 0..ITEMS_PER_RING {
            // Given a magic ring without mods, with one mod or filled to its magic limits
            let before = magic_item(ring, &pool, (trial % 3) as usize, &mut rng);
            let ctx = format!("{} with {:?}", ring.name, outcome(&before));
            if !explicits(&before).is_empty() {
                assert_ne!(
                    before.name(),
                    ring.name,
                    "{ctx}: the magic name holds the names of the mods"
                );
            }
            // When the orb is applied
            let mut after = before.clone();
            assert_eq!(
                apply_regal_orb(&mut after, &pool, &mut rng),
                Ok(()),
                "{ctx}"
            );
            let added = assert_upgraded_with_one_mod(&before, &after, &ctx);
            // Then the rare ring shows only its base name, without a random name
            assert_eq!(after.name(), ring.name, "{ctx}: {:?}", outcome(&after));
            // And both views follow the existing rules, the new mod last of its side
            let views = if VIEW_RINGS.contains(&ring.id.as_str()) {
                0..3
            } else {
                2..3
            };
            if views.contains(&trial) {
                assert_views_follow_the_rules(&before, &after, &added, &ctx);
            }
            // And with more mods, up to 6, the name stays the base name
            let mut full = after.clone();
            while add_random_mod(&mut full, &pool, &mut rng) {
                assert_eq!(full.name(), ring.name, "{ctx}: {:?}", outcome(&full));
            }
        }
    }

    // And the page shows the item's name and the translated rarity, also for rare items
    let translations = read_text(TRANSLATIONS);
    assert!(
        translations
            .lines()
            .any(|line| line.contains(&format!("\"{RARE_KEY}\":"))),
        "{TRANSLATIONS} must translate {RARE_KEY:?}"
    );
    assert_web_src_uses(
        &[".name()", RARE_KEY, "Rarity::Rare"],
        "the header shows the item's name, and the rarity line the translated rarity",
    );
}

// AC5: „Auf einem normalen und auf einem seltenen Item ist der Orb nicht anwendbar. Überfahren
// mit der Maus und Klick zeigen genau die Meldung `Failed to apply item: Target is not Magic.`
// Das Item bleibt unverändert."
// (Tooltip, look and message area are checked by the PO in the local preview, the wording of the
// message at the acceptance. Here: the reason on hover and on click for normal items, rare items
// made by the orb, full rare items and, by the PO's rule "the item is not magic", unique items,
// with the ring's pool and with an empty pool; its message from the game data; the unchanged
// item; and that the page marks the orb and shows the message.)
#[test]
fn ac5_the_orb_on_a_normal_or_rare_item_shows_the_game_message_and_keeps_the_item() {
    let mut rng = SeededRng::new(SEED);
    let empty: Vec<ExplicitModDefinition> = Vec::new();

    for ring_id in [IRON_RING_ID, COMPOSITE_RING_ID, COGWORK_RING_ID] {
        let ring = base(ring_id);
        let pool = pool_of(ring_id);
        for trial in 0..ITEMS {
            // Given a normal ring, a rare ring made by the orb, a full rare ring and a unique ring
            let normal = Item::new(&ring, &mut rng);
            assert_eq!(normal.rarity, Rarity::Normal);
            let rare = rare_by_orb(&ring, &pool, (trial % 3) as usize, &mut rng);
            let mut full_rare = rare.clone();
            while add_random_mod(&mut full_rare, &pool, &mut rng) {}
            let unique = item_with(&ring, Rarity::Unique, &[], &[], &mut rng);

            for item in [normal, rare, full_rare, unique] {
                for click_pool in [&pool, &empty] {
                    let ctx = format!(
                        "{:?} {} {trial} with {:?}",
                        item.rarity,
                        ring.name,
                        outcome(&item)
                    );
                    let mut clicked = item.clone();
                    // When the page asks whether the orb applies (hover)
                    let hover = can_apply_regal_orb(&item);
                    // And the orb is clicked anyway
                    let click = apply_regal_orb(&mut clicked, click_pool, &mut rng);
                    // Then both fail because the item is not magic, whatever the pool
                    assert_eq!(hover, Err(CraftError::ItemNotMagic), "{ctx}: hover");
                    assert_eq!(click, Err(CraftError::ItemNotMagic), "{ctx}: click");
                    // And the item is unchanged
                    assert_eq!(clicked, item, "{ctx}: the item is unchanged");
                }
            }
        }
    }
    // And hover and click show the orb's message for this reason from the game data
    let not_magic =
        assert_message_is_game_data(Currency::RegalOrb, CraftError::ItemNotMagic, NOT_MAGIC_CODE);
    // And it is not written into the code of crafting-core or web
    assert_only_in_game_data(&not_magic);
    // And the page marks the orb as not applicable and shows the message on hover and click
    assert_web_src_uses(
        &[
            "can_apply_regal_orb",
            "apply_regal_orb",
            "failure_message",
            "aria-disabled",
        ],
        "an inapplicable Regal Orb is marked and shows its failure message",
    );
}

// AC6: „Auf einem seltenen Item gelten für die anderen Orbs ihre bestehenden Regeln:
// - Transmutation: `Failed to apply item: Target is not Normal Rarity.`
// - Alteration und Augmentation: `Failed to apply item: Target is not Magic.`"
// (Tooltips and messages in the page are checked by the PO in the local preview, the wording at
// the acceptance. Here: on rare items made by the Regal Orb and on full rare items, each of the
// other orbs fails on hover and on click with its reason, the item stays unchanged, and each
// orb shows its message for that reason from the game data.)
#[test]
fn ac6_the_other_orbs_on_a_rare_item_keep_their_rules() {
    let mut rng = SeededRng::new(SEED);
    let empty: Vec<ExplicitModDefinition> = Vec::new();

    for ring_id in [IRON_RING_ID, COMPOSITE_RING_ID, COGWORK_RING_ID] {
        let ring = base(ring_id);
        let pool = pool_of(ring_id);
        for trial in 0..ITEMS {
            // Given a rare ring made by the Regal Orb, and the same ring with as many mods as a
            // rare item can hold
            let rare = rare_by_orb(&ring, &pool, (trial % 3) as usize, &mut rng);
            let mut full_rare = rare.clone();
            while add_random_mod(&mut full_rare, &pool, &mut rng) {}

            for item in [rare, full_rare] {
                for click_pool in [&pool, &empty] {
                    let ctx = format!("rare {} {trial} with {:?}", ring.name, outcome(&item));
                    // When the page asks whether each other orb applies (hover), and each is
                    // clicked anyway
                    // Then the Orb of Transmutation fails because the item is not normal
                    let mut clicked = item.clone();
                    assert_eq!(
                        can_apply_orb_of_transmutation(&item),
                        Err(CraftError::ItemNotNormal),
                        "{ctx}: Transmutation, hover"
                    );
                    assert_eq!(
                        apply_orb_of_transmutation(&mut clicked, click_pool, &mut rng),
                        Err(CraftError::ItemNotNormal),
                        "{ctx}: Transmutation, click"
                    );
                    assert_eq!(clicked, item, "{ctx}: Transmutation keeps the item");

                    // And the Orb of Alteration fails because the item is not magic
                    let mut clicked = item.clone();
                    assert_eq!(
                        can_apply_orb_of_alteration(&item),
                        Err(CraftError::ItemNotMagic),
                        "{ctx}: Alteration, hover"
                    );
                    assert_eq!(
                        apply_orb_of_alteration(&mut clicked, click_pool, &mut rng),
                        Err(CraftError::ItemNotMagic),
                        "{ctx}: Alteration, click"
                    );
                    assert_eq!(clicked, item, "{ctx}: Alteration keeps the item");

                    // And the Orb of Augmentation fails because the item is not magic
                    let mut clicked = item.clone();
                    assert_eq!(
                        can_apply_orb_of_augmentation(&item),
                        Err(CraftError::ItemNotMagic),
                        "{ctx}: Augmentation, hover"
                    );
                    assert_eq!(
                        apply_orb_of_augmentation(&mut clicked, click_pool, &mut rng),
                        Err(CraftError::ItemNotMagic),
                        "{ctx}: Augmentation, click"
                    );
                    assert_eq!(clicked, item, "{ctx}: Augmentation keeps the item");
                }
            }
        }
    }

    // And each orb shows its message for its reason from the game data
    assert_message_is_game_data(
        Currency::OrbOfTransmutation,
        CraftError::ItemNotNormal,
        NOT_NORMAL_CODE,
    );
    assert_message_is_game_data(
        Currency::OrbOfAlteration,
        CraftError::ItemNotMagic,
        NOT_MAGIC_CODE,
    );
    assert_message_is_game_data(
        Currency::OrbOfAugmentation,
        CraftError::ItemNotMagic,
        NOT_MAGIC_CODE,
    );
}
