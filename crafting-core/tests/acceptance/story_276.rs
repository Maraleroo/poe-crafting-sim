//! Acceptance tests for AB#276 "Orb of Augmentation auf magische Items anwenden".
//!
//! The tests use the public interface of `crafting-core` from the tech lead's plan on AB#276:
//! - `Currency::OrbOfAugmentation` with the id `orb_of_augmentation`; its name, description and
//!   messages are only in `data/currencies.json`.
//! - `can_apply_orb_of_augmentation(item) -> Result<(), CraftError>`: `ItemNotMagic` (code
//!   `item_not_magic`) for a normal, rare or unique item; `NoSpaceForMods` (code
//!   `no_space_for_mods`) for a magic item on which neither side has room under both affix
//!   limits. The rarity is checked first.
//! - `apply_orb_of_augmentation(item, pool, rng) -> Result<(), CraftError>`: `pool` is the base's
//!   pool; first the same checks, then exactly one `add_random_mod`.
//! - unchanged: `currencies`, `Currency::{id, name, description, failure_message}`,
//!   `CraftError::code`, `add_random_mod`, `valid_pool`, `mod_pool`, `base_item`,
//!   `Slot::Ring.base_items()`, `Item::affix_limits`, `Item::name`, `Mod::roll`, `SeededRng`.
//!   Following ADR 0004, web criteria are checked through the file contents under `web/`
//!   (without the build output in `web/dist` and `web/target`).
//!
//! The PO's rules are `poe-crafting-rules`: `currencies/orb-of-augmentation.md` (committed in
//! 505f273, before these tests), "Affix limits" and the magic item name in "Item basics", and
//! "Adding a random mod".
//!
//! Criterion 1 gives the PO's wording for `poe-crafting-rules`; it is checked byte for byte at
//! the acceptance (step 2 of the `story` skill), so it has no test here. Criterion 7 has no test
//! (PO decision on the story and in the task): no base of today can reach a magic item with room
//! but an empty valid pool; the tech lead covers it with a unit test.
//!
//! Game texts (the orb's name, its description and its messages) are not compared word for word
//! here (`writing-acceptance-tests`, "What permanent tests check"): their wording is checked at
//! the acceptance. The tests check that crafting-core shows the texts of the orb's entry in
//! `data/currencies.json`, under the reason codes of the plan, that the reasons of criteria 5 and
//! 6 have messages of their own, and that the texts are not written into the code of
//! `crafting-core` or `web`.
//!
//! Every random test uses a fixed seed. Statistical tests compare observed shares with the
//! shares that follow from the weights and the rule, with a tolerance of 5 standard deviations.
//! The tests depend only on frequencies and invariants, never on the order in which the
//! generator is used.
//!
//! Red state before the implementation: the module does not compile, because
//! `Currency::OrbOfAugmentation`, `CraftError::NoSpaceForMods`, `can_apply_orb_of_augmentation`
//! and `apply_orb_of_augmentation` do not exist yet.
//!
//! What is checked outside `cargo test`, by the PO in the local preview:
//! - AC2: the third button next to the other two orbs and its tooltip.
//! - AC5 and AC6: how the inapplicable orb looks, its tooltip and the message after a click.
//!
//! Not testable with today's game data: the PO's evidence of a magic Focused Amulet (limits 0
//! prefixes and 0 suffixes), because the simulator has no amulets. Every ring base filled to its
//! magic limits stands in for it in AC6.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crafting_core::{
    add_random_mod, apply_orb_of_augmentation, base_item, can_apply_orb_of_augmentation,
    currencies, mod_pool, valid_pool, AffixLimits, AffixType, BaseItem, CraftError, Currency,
    CurrencyDefinition, ExplicitMod, ExplicitModDefinition, Item, Mod, ModDefinition, Rarity,
    SeededRng, Slot, StatDefinition, DEFAULT_ITEM_LEVEL,
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

const ORB_ID: &str = "orb_of_augmentation";
const TRANSMUTATION_ID: &str = "orb_of_transmutation";
const ALTERATION_ID: &str = "orb_of_alteration";

/// Reason codes from the tech lead's plan and ADR 0012, the keys of `messages`.
const NOT_MAGIC_CODE: &str = "item_not_magic";
const NO_SPACE_CODE: &str = "no_space_for_mods";

/// Mods of the story's examples and of the PO's evidence "Sapphire Polished Composite Ring".
const STOUT: &str = "Stout";
const PHILOSOPHER: &str = "of the Philosopher";
const SAPPHIRE: &str = "Sapphire";
const POLISHED: &str = "Polished";
/// The PO's evidence for criterion 4: Sapphire before the orb, Polished added by it.
const SAPPHIRE_POLISHED_NAME: &str = "Sapphire Polished Composite Ring";

const CURRENCIES_JSON: &str = "data/currencies.json";

/// Fixed seeds, so every run is identical.
const SEED: u64 = 276;
const STATISTICS_SEED: u64 = 27_601;
const TEST_POOL_SEED: u64 = 27_602;

/// Items for invariant checks.
const ITEMS: u64 = 300;
/// Items per ring base filled to its magic limits (AC6).
const FILLED_ITEMS: u64 = 20;
/// Orbs on a ring with the ring's whole pool (AC3). The valid pools hold about 140 suffixes or
/// about 85 prefixes, weighted 50 to 2000. A uniform choice gives the suffixes with weight 2000
/// about 13 % instead of about 26 % of the results, and the prefixes with weight 1000 about 57 %
/// instead of about 78 %; 5 standard deviations are about 0.02, so it fails clearly. Each orb
/// builds the valid pool of about 230 mods, so more orbs would take longer than a few seconds.
const POOL_TRIALS: u64 = 10_000;
/// Orbs per invented pool (AC3). 5 standard deviations of a share are at most 0.018, so a
/// uniform choice (for example 33 % instead of 60 %) or a coin flip between the sides (50 %
/// instead of 75 %) fails clearly.
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

/// The text of the Orb of Augmentation's entry in `data/currencies.json` at the given path of
/// keys. A game text is never empty.
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

/// Asserts that the orb shows the game data's message for `error`: `code()` is the plan's
/// reason code, and `failure_message` is the text under that code in `currencies()` and in
/// `data/currencies.json`, which is not written into the code. Returns the message.
fn assert_message_is_game_data(error: CraftError, code: &str) -> String {
    assert_eq!(error.code(), code, "the reason code of {error:?}");
    let in_file = text_in_file(&["messages", code]);
    assert_eq!(
        definition(ORB_ID).messages.get(code).map(String::as_str),
        Some(in_file.as_str()),
        "currencies(): message of {ORB_ID:?} for {code:?}"
    );
    assert_eq!(
        Currency::OrbOfAugmentation.failure_message(error),
        in_file,
        "the message for {error:?} is the game data's"
    );
    assert_only_in_game_data(&in_file);
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

/// Whether a side of the item has room under both affix limits ("Affix limits").
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

/// Asserts that `after` is `before` with exactly one more mod at the end of its side, all other
/// mods unchanged in value and order, and returns that mod.
fn assert_one_mod_added(before: &Item, after: &Item, ctx: &str) -> ExplicitMod {
    assert_eq!(after.base, before.base, "{ctx}: the base stays");
    assert_eq!(after.rarity, Rarity::Magic, "{ctx}: the item stays magic");
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
        "{ctx}: the mods on the side of the new mod stay unchanged, in their order"
    );
    assert_eq!(
        other_after, other_before,
        "{ctx}: the other side stays unchanged"
    );
    let added = side.last().expect("one mod was added").clone();
    assert_eq!(
        added.affix_type, affix_type,
        "{ctx}: {:?} sits on its own side",
        added.name
    );
    added
}

/// Applies the orb to a copy of `item`, checks the rule for one result and returns the new
/// mod: the orb applies, exactly one mod of the valid pool is added at the end of its side,
/// rolled from its definition, and nothing else changes.
fn augment_once(
    item: &Item,
    pool: &[ExplicitModDefinition],
    expected: &[&ExplicitModDefinition],
    rng: &mut SeededRng,
    ctx: &str,
) -> (Item, ExplicitMod) {
    let mut after = item.clone();
    assert_eq!(
        can_apply_orb_of_augmentation(&after),
        Ok(()),
        "{ctx}: the orb applies to a magic item with room, found {:?}",
        outcome(item)
    );
    let result = apply_orb_of_augmentation(&mut after, pool, rng);
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

/// Applies the orb `trials` times to copies of `item` and asserts that the new mod is chosen
/// from the valid pool of the rule, each mod with the chance of its weight divided by the sum of
/// the weights in the pool. Also checks the shares of the mods of each weight and of each side,
/// so that a choice that ignores the weights or flips a coin between the sides fails clearly.
fn assert_augments_by_weight(
    item: &Item,
    pool: &[ExplicitModDefinition],
    trials: u64,
    rng: &mut SeededRng,
    label: &str,
) {
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
        let (_, added) = augment_once(item, pool, &expected, rng, &format!("{label}, {trial}"));
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
    for (side, (hits, weights)) in by_side {
        assert_share(
            &format!("{label}: {side} mods"),
            hits,
            trials,
            weights / total,
        );
    }
}

// ---------------------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------------------

// AC2: „Neben dem Orb of Transmutation und dem Orb of Alteration gibt es einen Orb of
// Augmentation. Sein Name ist ein Spieltext. Ist der Orb anwendbar, zeigt das Überfahren mit der
// Maus genau den Text `Augments a magic item with a new random modifier` (Spieldaten)."
// (The button's place and the tooltip are checked by the PO in the local preview; the wording of
// the name and the description at the acceptance. Here: the three orbs exist, the new orb's name
// and description are the texts of its entry in the game data and are not written into the
// code, the orb applies to magic items with room, and the page uses the new orb from
// crafting-core and shows its description.)
#[test]
fn ac2_orb_of_augmentation_exists_with_its_game_name_and_description() {
    // Given the currencies of crafting-core
    let ids: Vec<String> = currencies().into_iter().map(|entry| entry.id).collect();

    // Then the Orb of Augmentation exists next to the other two orbs
    for id in [TRANSMUTATION_ID, ALTERATION_ID, ORB_ID] {
        assert!(
            ids.iter().any(|found| found == id),
            "currencies() must contain {id:?}, found {ids:?}"
        );
    }
    assert_eq!(Currency::OrbOfAugmentation.id(), ORB_ID);

    // And its name is the game text of its entry in the game data
    let name = text_in_file(&["name"]);
    assert_eq!(Currency::OrbOfAugmentation.name(), name);
    assert_eq!(definition(ORB_ID).name, name);
    // And it is not written into the code of crafting-core or web
    assert_only_in_game_data(&name);

    // And its description is the game text of its entry in the game data
    let description = text_in_file(&["description"]);
    assert_eq!(Currency::OrbOfAugmentation.description(), description);
    assert_eq!(definition(ORB_ID).description, description);
    // And it is not written into the code of crafting-core or web
    assert_only_in_game_data(&description);

    // Given magic Iron Rings and Composite Rings with room: without mods and with one mod
    let mut rng = SeededRng::new(SEED);
    for ring_id in [IRON_RING_ID, COMPOSITE_RING_ID] {
        let ring = base(ring_id);
        let pool = pool_of(ring_id);
        for trial in 0..ITEMS {
            let mut item = item_with(&ring, Rarity::Magic, &[], &[], &mut rng);
            if trial % 2 == 1 {
                assert!(add_random_mod(&mut item, &pool, &mut rng), "one mod");
            }
            // When the page asks whether the orb applies
            // Then it does, so the tooltip is the description
            assert_eq!(
                can_apply_orb_of_augmentation(&item),
                Ok(()),
                "{} with {:?}",
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
            "can_apply_orb_of_augmentation",
            "apply_orb_of_augmentation",
            "description()",
        ],
        "the Orb of Augmentation's button comes from crafting-core, next to the other two orbs, \
         and its tooltip is its description",
    );
}

// AC3: „Auf einem magischen Item mit Platz fügt der Orb genau einen Mod nach Kriterium 1 hinzu;
// die vorhandenen Mods bleiben unverändert. Beispiele: Ein magischer Iron Ring mit nur einem
// Prefix bekommt einen Suffix, gewählt mit den Häufigkeiten der Gewichte. Ein magischer Composite
// Ring mit einem Prefix bekommt einen zweiten Prefix aus einer anderen Familie."
// (Covered: both examples with the ring's whole pool from the game data, by weight and by the
// rule "Adding a random mod"; invented pools in which every filter of the rule leaves out a mod
// with a high weight; a magic item without mods, on which the mod is chosen from both sides by
// weight; and the orb applied to the same ring until it is full.)
#[test]
fn ac3_the_orb_adds_exactly_one_mod_by_the_rule_and_keeps_the_others() {
    let mut rng = SeededRng::new(STATISTICS_SEED);
    let iron = base(IRON_RING_ID);
    let iron_pool = pool_of(IRON_RING_ID);
    let composite = base(COMPOSITE_RING_ID);
    let composite_pool = pool_of(COMPOSITE_RING_ID);

    // --- The story's examples with the game data ---
    // Given a magic Iron Ring with only a prefix
    let stout = find(&iron_pool, STOUT);
    let iron_with_prefix = item_with(&iron, Rarity::Magic, &[stout], &[], &mut rng);
    // When the orb is applied many times
    // Then it adds exactly one suffix, chosen with the frequencies of the weights, and the prefix
    // stays unchanged
    assert!(
        rule_pool(&iron_with_prefix, &iron_pool)
            .iter()
            .all(|definition| definition.affix_type == AffixType::Suffix),
        "the Iron Ring with a prefix has room for a suffix only"
    );
    assert_augments_by_weight(
        &iron_with_prefix,
        &iron_pool,
        POOL_TRIALS,
        &mut rng,
        "Iron Ring with a prefix",
    );

    // Given a magic Composite Ring with one prefix
    let sapphire = find(&composite_pool, SAPPHIRE);
    let composite_with_prefix = item_with(&composite, Rarity::Magic, &[sapphire], &[], &mut rng);
    // When the orb is applied many times
    // Then it adds exactly one second prefix of another family, chosen by weight, and the first
    // prefix stays unchanged
    let expected = rule_pool(&composite_with_prefix, &composite_pool);
    assert!(
        expected.iter().all(|definition| {
            definition.affix_type == AffixType::Prefix
                && definition.definition.family != sapphire.definition.family
        }),
        "the Composite Ring with a prefix has room for a prefix of another family only"
    );
    assert!(
        composite_pool.iter().any(|definition| {
            definition.affix_type == AffixType::Prefix
                && definition.name != sapphire.name
                && definition.definition.family == sapphire.definition.family
        }),
        "the pool must hold other prefixes of the family of {SAPPHIRE:?}, so the family rule is \
         checked"
    );
    assert_augments_by_weight(
        &composite_with_prefix,
        &composite_pool,
        POOL_TRIALS,
        &mut rng,
        "Composite Ring with a prefix",
    );

    // --- Invented pools: each filter of the rule leaves out a mod with a high weight ---
    let mut test_rng = SeededRng::new(TEST_POOL_SEED);
    let held_prefix = test_mod("Held Prefix", AffixType::Prefix, "Held Family", 1000, 1);
    let held_suffix = test_mod(
        "of the Held Suffix",
        AffixType::Suffix,
        "Held Family",
        1000,
        1,
    );
    // The suffix side has room: 60 %, 30 % and 10 %; no prefix, no mod of the held family, no
    // mod above the item level
    let suffix_pool = vec![
        test_mod("Other Prefix", AffixType::Prefix, "Other Family", 9000, 1),
        test_mod(
            "of the Held Family",
            AffixType::Suffix,
            "Held Family",
            9000,
            1,
        ),
        test_mod("of the Heights", AffixType::Suffix, "Heights", 9000, 51),
        test_mod("of A", AffixType::Suffix, "A", 6000, 1),
        test_mod("of B", AffixType::Suffix, "B", 3000, 50),
        test_mod("of C", AffixType::Suffix, "C", 1000, 1),
    ];
    // The prefix side has room: 20 % and 80 %
    let prefix_pool = vec![
        test_mod(
            "of the Other Suffix",
            AffixType::Suffix,
            "Other Family",
            9000,
            1,
        ),
        test_mod(
            "Held Family Prefix",
            AffixType::Prefix,
            "Held Family",
            9000,
            1,
        ),
        test_mod("High Prefix", AffixType::Prefix, "Heights", 9000, 51),
        test_mod("Prefix A", AffixType::Prefix, "A", 1000, 1),
        test_mod("Prefix B", AffixType::Prefix, "B", 4000, 50),
    ];
    // Both sides have room: 75 % for the prefix, 25 % for the suffix, not a coin flip
    let both_sides_pool = vec![
        test_mod("Prefix A", AffixType::Prefix, "A", 3000, 1),
        test_mod("of B", AffixType::Suffix, "B", 1000, 1),
        test_mod("of the Heights", AffixType::Suffix, "Heights", 9000, 51),
    ];
    // A Composite Ring has room for prefixes only: 60 % and 40 %; no suffix, no mod of the held
    // family
    let composite_test_pool = vec![
        test_mod("of the Suffix", AffixType::Suffix, "Suffix Family", 9000, 1),
        test_mod(
            "Held Family Prefix",
            AffixType::Prefix,
            "Held Family",
            9000,
            1,
        ),
        test_mod("Prefix A", AffixType::Prefix, "A", 3000, 1),
        test_mod("Prefix B", AffixType::Prefix, "B", 2000, 1),
    ];
    let mut cases = vec![
        (
            "invented pool, Iron Ring with a prefix",
            item_with(&iron, Rarity::Magic, &[&held_prefix], &[], &mut test_rng),
            suffix_pool,
        ),
        (
            "invented pool, Iron Ring with a suffix",
            item_with(&iron, Rarity::Magic, &[], &[&held_suffix], &mut test_rng),
            prefix_pool,
        ),
        (
            "invented pool, Iron Ring without mods",
            item_with(&iron, Rarity::Magic, &[], &[], &mut test_rng),
            both_sides_pool,
        ),
        (
            "invented pool, Composite Ring with a prefix",
            item_with(
                &composite,
                Rarity::Magic,
                &[&held_prefix],
                &[],
                &mut test_rng,
            ),
            composite_test_pool,
        ),
    ];
    for (label, item, pool) in &mut cases {
        // Given a magic ring of item level 50 with room, and the invented pool
        item.item_level = 50;
        // When the orb is applied many times
        // Then it adds exactly one mod of the valid pool, chosen by weight, and the other mods
        // stay unchanged
        assert_augments_by_weight(item, pool, TEST_POOL_TRIALS, &mut test_rng, label);
    }

    // --- The orb on the same ring until it is full ---
    for ring in [&iron, &composite] {
        let pool = pool_of(&ring.id);
        for trial in 0..ITEMS {
            let ctx = format!("{} {trial}", ring.name);
            // Given a magic ring without mods
            let mut item = item_with(ring, Rarity::Magic, &[], &[], &mut rng);
            // When the orb is applied twice
            for orb in 1..=2 {
                let expected = rule_pool(&item, &pool);
                let (after, _) = augment_once(
                    &item,
                    &pool,
                    &expected,
                    &mut rng,
                    &format!("{ctx}, orb {orb}"),
                );
                item = after;
            }
            // Then the ring holds two mods of different families
            let families: BTreeSet<&str> = explicits(&item)
                .iter()
                .map(|explicit| explicit.modifier.family.as_str())
                .collect();
            assert_eq!(families.len(), 2, "{ctx}: {:?}", outcome(&item));
        }
    }
}

// AC4: „Der neue Mod erscheint im Namen nach der bestehenden Namensregel, also hinter den
// vorhandenen Mods seiner Seite. Beleg: „Sapphire Polished Composite Ring“ (Polished per
// Augmentation hinzugefügt)."
// (Covered: the PO's evidence, and the names after the orb on Iron Rings, Composite Rings and
// Cogwork Rings with a mod on the same side and on the other side.)
#[test]
fn ac4_the_new_mod_appears_in_the_name_after_the_mods_of_its_side() {
    let mut rng = SeededRng::new(SEED);

    // --- The PO's evidence ---
    // Given a magic Composite Ring with the prefix "Sapphire"
    let composite = base(COMPOSITE_RING_ID);
    let composite_pool = pool_of(COMPOSITE_RING_ID);
    let sapphire = find(&composite_pool, SAPPHIRE);
    let polished = find(&composite_pool, POLISHED);
    let mut item = item_with(&composite, Rarity::Magic, &[sapphire], &[], &mut rng);
    // When the orb adds "Polished"
    let result = apply_orb_of_augmentation(&mut item, std::slice::from_ref(polished), &mut rng);
    // Then the ring is the "Sapphire Polished Composite Ring"
    assert_eq!(result, Ok(()), "the orb adds {POLISHED:?}");
    assert_eq!(item.name(), SAPPHIRE_POLISHED_NAME, "{:?}", outcome(&item));

    // --- The name after the orb, with the rings' whole pools ---
    let iron_pool = pool_of(IRON_RING_ID);
    let cogwork_pool = pool_of(COGWORK_RING_ID);
    let cases = [
        (IRON_RING_ID, vec![STOUT], vec![]),
        (IRON_RING_ID, vec![], vec![PHILOSOPHER]),
        (COMPOSITE_RING_ID, vec![SAPPHIRE], vec![]),
        (COGWORK_RING_ID, vec![], vec![PHILOSOPHER]),
    ];
    for (ring_id, prefixes, suffixes) in cases {
        let ring = base(ring_id);
        let pool = match ring_id {
            IRON_RING_ID => &iron_pool,
            COMPOSITE_RING_ID => &composite_pool,
            _ => &cogwork_pool,
        };
        let prefixes: Vec<&ExplicitModDefinition> =
            prefixes.iter().map(|name| find(pool, name)).collect();
        let suffixes: Vec<&ExplicitModDefinition> =
            suffixes.iter().map(|name| find(pool, name)).collect();
        // Given a magic ring with one mod
        let item = item_with(&ring, Rarity::Magic, &prefixes, &suffixes, &mut rng);
        let expected = rule_pool(&item, pool);
        for trial in 0..ITEMS {
            let ctx = format!("{} with {:?}, {trial}", ring.name, outcome(&item));
            // When the orb is applied
            let (after, added) = augment_once(&item, pool, &expected, &mut rng, &ctx);
            // Then the name is the prefixes, the base name and the suffixes, the new mod behind
            // the mods of its side
            let (new_prefixes, new_suffixes) = match added.affix_type {
                AffixType::Prefix => (
                    [names(prefixes.iter().copied()), vec![added.name.clone()]].concat(),
                    names(suffixes.iter().copied()),
                ),
                AffixType::Suffix => (
                    names(prefixes.iter().copied()),
                    [names(suffixes.iter().copied()), vec![added.name.clone()]].concat(),
                ),
            };
            let expected_name = [new_prefixes, vec![ring.name.clone()], new_suffixes]
                .concat()
                .join(" ");
            assert_eq!(after.name(), expected_name, "{ctx}");
        }
    }
}

// AC5: „Auf einem normalen Item ist der Orb nicht anwendbar. Überfahren mit der Maus und Klick
// zeigen genau die Meldung `Failed to apply item: Target is not Magic.` Das Item bleibt
// unverändert."
// (Tooltip, look and message area are checked by the PO in the local preview, the wording of the
// message at the acceptance. Here: the reason on hover and on click for normal items and, by the
// PO's rule "the item is not magic", for rare and unique items, also with a full rare item and
// with an empty pool; its message from the game data; the unchanged item; and that the page
// marks the orb and shows the message.)
#[test]
fn ac5_the_orb_on_a_normal_item_shows_the_game_message_and_keeps_the_item() {
    let mut rng = SeededRng::new(SEED);
    let empty: Vec<ExplicitModDefinition> = Vec::new();

    for ring_id in [IRON_RING_ID, COMPOSITE_RING_ID] {
        let ring = base(ring_id);
        let pool = pool_of(ring_id);
        for trial in 0..ITEMS {
            // Given a normal ring, a rare ring with some mods, a full rare ring and a unique ring
            let normal = Item::new(&ring, &mut rng);
            assert_eq!(normal.rarity, Rarity::Normal);
            let mut rare = item_with(&ring, Rarity::Rare, &[], &[], &mut rng);
            assert!(add_random_mod(&mut rare, &pool, &mut rng), "a rare mod");
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
                    let hover = can_apply_orb_of_augmentation(&item);
                    // And the orb is clicked anyway
                    let click = apply_orb_of_augmentation(&mut clicked, click_pool, &mut rng);
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
    let not_magic = assert_message_is_game_data(CraftError::ItemNotMagic, NOT_MAGIC_CODE);
    // And it is another message than the one for a magic item without room
    assert_ne!(
        not_magic,
        text_in_file(&["messages", NO_SPACE_CODE]),
        "the two reasons of the PO's rule have messages of their own"
    );
    // And the page marks the orb as not applicable and shows the message on hover and click
    assert_web_src_uses(
        &[
            "can_apply_orb_of_augmentation",
            "apply_orb_of_augmentation",
            "failure_message",
            "aria-disabled",
        ],
        "an inapplicable Orb of Augmentation is marked and shows its failure message",
    );
}

// AC6: „Auf einem magischen Item ohne Platz ist der Orb nicht anwendbar. Überfahren mit der Maus
// und Klick zeigen genau die Meldung `Failed to apply item: Item has no space for more Mods.` Das
// Item bleibt unverändert. Beispiele: Iron Ring mit Prefix und Suffix, Composite Ring mit 2
// Prefixen."
// (Tooltip, look and message area are checked by the PO in the local preview, the wording of the
// message at the acceptance. Here: both examples; every ring base, including the Heist rings
// with their changed limits, filled to its magic limits, with the ring's pool and with an empty
// pool; with one mod less, the orb applies again. And the message from the game data, the
// unchanged item, and that the page shows the message.)
#[test]
fn ac6_the_orb_on_a_magic_item_without_room_shows_the_game_message_and_keeps_the_item() {
    let mut rng = SeededRng::new(SEED);
    let empty: Vec<ExplicitModDefinition> = Vec::new();

    // --- The story's examples ---
    let iron = base(IRON_RING_ID);
    let iron_pool = pool_of(IRON_RING_ID);
    let composite = base(COMPOSITE_RING_ID);
    let composite_pool = pool_of(COMPOSITE_RING_ID);
    let mut examples = vec![
        (
            item_with(
                &iron,
                Rarity::Magic,
                &[find(&iron_pool, STOUT)],
                &[find(&iron_pool, PHILOSOPHER)],
                &mut rng,
            ),
            iron_pool.clone(),
        ),
        (
            item_with(
                &composite,
                Rarity::Magic,
                &[
                    find(&composite_pool, SAPPHIRE),
                    find(&composite_pool, POLISHED),
                ],
                &[],
                &mut rng,
            ),
            composite_pool.clone(),
        ),
    ];

    // --- Every ring base, filled to its magic limits ---
    let rings = Slot::Ring.base_items();
    assert!(!rings.is_empty(), "the slot \"Ring\" must have ring bases");
    for ring in &rings {
        let pool = pool_of(&ring.id);
        for _ in 0..FILLED_ITEMS {
            let mut item = item_with(ring, Rarity::Magic, &[], &[], &mut rng);
            item.item_level = DEFAULT_ITEM_LEVEL;
            while add_random_mod(&mut item, &pool, &mut rng) {}
            examples.push((item, pool.clone()));
        }
    }

    for (item, pool) in &examples {
        let ctx = format!("magic {} with {:?}", item.base.name, outcome(item));
        // Given a magic item on which neither side has room under both affix limits
        assert!(
            !side_has_room(item, AffixType::Prefix) && !side_has_room(item, AffixType::Suffix),
            "{ctx}: the test needs an item without room, limits {:?}",
            limits(item)
        );
        for click_pool in [pool, &empty] {
            let mut clicked = item.clone();
            // When the page asks whether the orb applies (hover)
            let hover = can_apply_orb_of_augmentation(item);
            // And the orb is clicked anyway
            let click = apply_orb_of_augmentation(&mut clicked, click_pool, &mut rng);
            // Then both fail because the item has no space for more mods, whatever the pool
            assert_eq!(hover, Err(CraftError::NoSpaceForMods), "{ctx}: hover");
            assert_eq!(click, Err(CraftError::NoSpaceForMods), "{ctx}: click");
            // And the item is unchanged
            assert_eq!(&clicked, item, "{ctx}: the item is unchanged");
        }
        // And with one mod less, the side of that mod has room again and the orb applies
        let mut one_less = item.clone();
        if one_less.suffixes.pop().is_none() {
            one_less
                .prefixes
                .pop()
                .expect("a full magic ring has a mod");
        }
        assert_eq!(
            can_apply_orb_of_augmentation(&one_less),
            Ok(()),
            "{ctx}: with {:?} the orb applies",
            outcome(&one_less)
        );
    }

    // And hover and click show the orb's message for this reason from the game data
    let no_space = assert_message_is_game_data(CraftError::NoSpaceForMods, NO_SPACE_CODE);
    // And it is another message than the one for an item that is not magic
    assert_ne!(
        no_space,
        Currency::OrbOfAugmentation.failure_message(CraftError::ItemNotMagic),
        "the two reasons of the PO's rule have messages of their own"
    );
    // And the page shows the message on hover and click
    assert_web_src_uses(
        &[
            "can_apply_orb_of_augmentation",
            "apply_orb_of_augmentation",
            "failure_message",
            "aria-disabled",
        ],
        "an Orb of Augmentation without room is marked and shows its failure message",
    );
}
