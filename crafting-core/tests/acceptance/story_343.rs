//! Acceptance tests for AB#343 "Orb of Scouring auf magische und seltene Items anwenden".
//!
//! The tests use the public interface of `crafting-core` from the tech lead's plan on AB#343:
//! - `Currency::OrbOfScouring` with the id `orb_of_scouring`; its name, description and message
//!   are only in `data/currencies.json`.
//! - `CraftError::ItemNotMagicOrRare` with the code `item_not_magic_or_rare`.
//! - `can_apply_orb_of_scouring(item) -> Result<(), CraftError>`: `Ok` for a magic or rare item,
//!   `ItemNotMagicOrRare` for a normal or unique item. No pool, no generator.
//! - `apply_orb_of_scouring(item) -> Result<(), CraftError>`: first the same check; on an error
//!   the item is unchanged. Otherwise all prefixes and suffixes are removed and the item becomes
//!   normal; base, implicits with their values and item level stay. `Ok` on every magic or rare
//!   item, also without mods.
//! - unchanged: `currencies`, `Currency::{id, name, description, failure_message}`,
//!   `CraftError::code`, `add_random_mod`, `mod_pool`, `base_item`, `Slot::Ring.base_items()`,
//!   `Item::{new, name, required_level, explicit_texts, advanced_mods}`,
//!   `can_apply_orb_of_transmutation`, `Mod::roll`, `SeededRng`. Following ADR 0004, web criteria
//!   are checked through the file contents under `web/` (without the build output in `web/dist`
//!   and `web/target`).
//!
//! The PO's rules are `poe-crafting-rules`: `currencies/orb-of-scouring.md` (committed in
//! 569b35d, before these tests) and the level requirement in "Item basics".
//!
//! Criterion 1 gives the PO's wording for `poe-crafting-rules`. It is already met by 569b35d and
//! its wording is checked byte for byte at the acceptance (step 2 of the `story` skill), so it
//! has no test here, as for the Chaos Orb (AB#313). The behaviour of the rule is checked by the
//! tests of criteria 3 and 4.
//!
//! Game texts (the orb's name, its description and its message) are not compared word for word
//! here (`writing-acceptance-tests`, "What permanent tests check"): their wording is checked at
//! the acceptance. The tests check that crafting-core shows the texts of the orb's entry in
//! `data/currencies.json`, and that the texts are not written into the code of `crafting-core`
//! or `web` (comments do not count, ADR 0011 amendment AB#326).
//!
//! The orb uses no randomness. The generator with a fixed seed only rolls the items the orb is
//! applied to, so every run is identical.
//!
//! Not modelled yet (the PO's rule and the story): protected and fractured mods that stay on the
//! item. Until such mods exist, scouring always gives a normal item, which is what AC3 checks.
//!
//! Red state before the implementation: the module does not compile, because
//! `Currency::OrbOfScouring`, `CraftError::ItemNotMagicOrRare`, `can_apply_orb_of_scouring` and
//! `apply_orb_of_scouring` do not exist yet.
//!
//! What is checked outside `cargo test`, by the PO in the local preview:
//! - AC2: the seventh button next to the other orbs and its tooltip.
//! - AC3: the normal item on the page, its name and that it shows no requirements line.
//! - AC4: how the inapplicable orb looks, its tooltip and the message after a click.

use std::fs;
use std::path::{Path, PathBuf};

use crafting_core::{
    add_random_mod, apply_orb_of_scouring, base_item, can_apply_orb_of_scouring,
    can_apply_orb_of_transmutation, currencies, mod_pool, AffixType, BaseItem, CraftError,
    Currency, CurrencyDefinition, ExplicitMod, ExplicitModDefinition, Item, Mod, ModKind, Rarity,
    SeededRng, Slot, DEFAULT_ITEM_LEVEL,
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

const ORB_ID: &str = "orb_of_scouring";
/// The orbs the page already has (criterion 2: "Neben den bisherigen Orbs").
const EARLIER_ORB_IDS: [&str; 6] = [
    "orb_of_transmutation",
    "orb_of_alteration",
    "orb_of_augmentation",
    "regal_orb",
    "exalted_orb",
    "chaos_orb",
];

/// Reason code from the tech lead's plan and ADR 0012, the key of `messages`.
const NOT_MAGIC_OR_RARE_CODE: &str = "item_not_magic_or_rare";

/// Criterion 3 and the PO's evidence: a rare and a magic Iron Ring of item level 83.
const EXAMPLE_ITEM_LEVEL: u32 = 83;
/// The PO's evidence: the magic "Dancer's Iron Ring of Victory" with level requirement 18.
const DANCERS: &str = "Dancer's";
const OF_VICTORY: &str = "of Victory";
const EVIDENCE_MAGIC_REQUIREMENT: u32 = 18;
/// The PO's evidence: the rare Iron Ring "Blight Finger" with 3 prefixes and 3 suffixes.
const EVIDENCE_RARE_SPLIT: (usize, usize) = (3, 3);

/// Item levels of the items the orb is applied to: the lowest, the example's and the default.
const ITEM_LEVELS: [u32; 3] = [1, EXAMPLE_ITEM_LEVEL, DEFAULT_ITEM_LEVEL];

const CURRENCIES_JSON: &str = "data/currencies.json";

/// Fixed seed, so every run is identical.
const SEED: u64 = 343;

/// Items per ring base, rarity and item level: magic items with 0 to 2 mods and rare items with
/// 0 to 6 mods, every number at least twice. The orb itself is fast; rolling the items takes the
/// time.
const MAGIC_ITEMS: u64 = 6;
const RARE_ITEMS: u64 = 14;
/// Rare Iron Rings with 3 prefixes and 3 suffixes for the example of criterion 3.
const EXAMPLE_ITEMS: u64 = 20;
/// Normal and unique items per ring base for criterion 4.
const NOT_APPLICABLE_ITEMS: u64 = 4;

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

/// Asserts that some line under `web/src/` uses `apply_orb_of_scouring` other than as part of
/// `can_apply_orb_of_scouring`: the page applies the orb on a click, not only asks whether it
/// applies.
fn assert_web_src_applies_the_orb() {
    let applies = web_src_files().iter().any(|path| {
        read_path(path).lines().any(|line| {
            line.replace("can_apply_orb_of_scouring", "")
                .contains("apply_orb_of_scouring")
        })
    });
    assert!(
        applies,
        "a file under web/src/ must call `apply_orb_of_scouring`: a click applies the orb"
    );
}

/// Asserts that a game text lives only in the game data: not in the code of `crafting-core`
/// and not in any file under `web/` (ADR 0011, ADR 0012).
/// Comments are not searched (AB#326, ADR 0011 amendment): see `story_326.rs`.
fn assert_only_in_game_data(text: &str) {
    let in_code = [
        crate::story_326::files_containing_in_code(&core_src_files(), text),
        crate::story_326::files_containing_in_code(&web_files(), text),
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

/// The entry of the Orb of Scouring in `data/currencies.json`.
fn entry_in_file() -> Value {
    let content = read_text(CURRENCIES_JSON);
    let data: Value = serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("{CURRENCIES_JSON} is not valid JSON: {e}"));
    data.get("currencies")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("{CURRENCIES_JSON} must have an array \"currencies\""))
        .iter()
        .find(|entry| entry.get("id").and_then(Value::as_str) == Some(ORB_ID))
        .unwrap_or_else(|| panic!("{CURRENCIES_JSON} must contain the currency {ORB_ID:?}"))
        .clone()
}

/// The text of the Orb of Scouring's entry in `data/currencies.json` at the given path of keys.
/// A game text is never empty.
fn text_in_file(path: &[&str]) -> String {
    let entry = entry_in_file();
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

/// Magic items with 0 to 2 mods and rare items with 0 to 6 mods of `base` at every item level of
/// `ITEM_LEVELS`, also full ones and ones without mods.
fn magic_and_rare_items(base: &BaseItem, rng: &mut SeededRng) -> Vec<Item> {
    let pool = pool_of(&base.id);
    let mut items = Vec::new();
    for item_level in ITEM_LEVELS {
        for trial in 0..MAGIC_ITEMS {
            let mods = (trial % 3) as usize;
            items.push(item_of_rarity(
                base,
                Rarity::Magic,
                item_level,
                &pool,
                mods,
                rng,
            ));
        }
        for trial in 0..RARE_ITEMS {
            let mods = (trial % 7) as usize;
            items.push(item_of_rarity(
                base,
                Rarity::Rare,
                item_level,
                &pool,
                mods,
                rng,
            ));
        }
    }
    items
}

/// Invented mods of `count` different families on one side, which are in no pool, for unique
/// items with mods.
fn invented_mods(affix_type: AffixType, count: usize) -> Vec<ExplicitModDefinition> {
    let template = pool_of(IRON_RING_ID)
        .into_iter()
        .find(|definition| definition.affix_type == affix_type)
        .expect("the Iron Ring's pool has prefixes and suffixes");
    (0..count)
        .map(|n| {
            let mut definition = template.clone();
            definition.name = format!("Invented {affix_type:?} {n}");
            definition.definition.family = format!("Invented {affix_type:?} family {n}");
            definition
        })
        .collect()
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

/// The level requirement of a normal item of `base`: a new item, which is normal, with its
/// implicits and no other mods (the PO's rule: "The level requirement is then that of a normal
/// item of the base.").
fn normal_requirement(base: &BaseItem, rng: &mut SeededRng) -> Option<u32> {
    let normal = Item::new(base, rng);
    assert_eq!(normal.rarity, Rarity::Normal, "a new item is normal");
    assert!(explicits(&normal).is_empty(), "a new item has no mods");
    normal.required_level()
}

/// Applies the orb to the magic or rare `item` and checks the result of the PO's rule: the orb
/// applies on hover and takes effect on click; all prefixes and suffixes are removed; the item is
/// normal and has only its base name; base, implicits with their values and item level are
/// unchanged; the requirement is that of a normal item of the base; the views show only the
/// implicits; and the item can be started again with an Orb of Transmutation. Returns the item
/// before the orb.
fn scour(item: &mut Item, rng: &mut SeededRng, ctx: &str) -> Item {
    let ctx = format!("{ctx}, {:?} with {:?}", item.rarity, outcome(item));
    let before = item.clone();
    assert!(
        [Rarity::Magic, Rarity::Rare].contains(&before.rarity),
        "{ctx}: the test starts magic or rare"
    );

    // When the page asks whether the orb applies (hover)
    // Then it does
    assert_eq!(
        can_apply_orb_of_scouring(item),
        Ok(()),
        "{ctx}: the orb applies to a magic or rare item"
    );
    // When the orb is clicked
    // Then it takes effect, without a message
    assert_eq!(
        apply_orb_of_scouring(item),
        Ok(()),
        "{ctx}: the orb takes effect on a magic or rare item"
    );

    // And all prefixes and suffixes are gone, and the item is normal
    assert!(
        item.prefixes.is_empty() && item.suffixes.is_empty(),
        "{ctx}: all prefixes and suffixes are removed, found {:?}",
        outcome(item)
    );
    assert_eq!(
        item.rarity,
        Rarity::Normal,
        "{ctx}: the item becomes normal"
    );
    // And base, implicits with their values and item level stay; nothing else changes
    assert_eq!(item.base, before.base, "{ctx}: the base stays");
    assert_eq!(
        item.implicits, before.implicits,
        "{ctx}: the implicits stay unchanged with their values"
    );
    assert_eq!(
        item.item_level, before.item_level,
        "{ctx}: the item level stays"
    );
    let mut expected = before.clone();
    expected.rarity = Rarity::Normal;
    expected.prefixes.clear();
    expected.suffixes.clear();
    assert_eq!(
        *item, expected,
        "{ctx}: the orb changes only the rarity and removes the prefixes and suffixes"
    );

    // And the item has no name of its own any more: only the base name
    assert_eq!(item.name(), before.base.name, "{ctx}: the name");
    // And the requirement is that of a normal item of the base
    assert_eq!(
        item.required_level(),
        normal_requirement(&before.base, rng),
        "{ctx}: the level requirement"
    );
    // And the views show no explicit mod, only the implicits as before
    assert!(
        item.explicit_texts().is_empty(),
        "{ctx}: no explicit lines, found {:?}",
        item.explicit_texts()
    );
    let implicit_blocks = |item: &Item| {
        item.advanced_mods()
            .into_iter()
            .filter(|block| block.kind == ModKind::Implicit)
            .collect::<Vec<_>>()
    };
    let blocks = item.advanced_mods();
    assert!(
        blocks.iter().all(|block| block.kind == ModKind::Implicit),
        "{ctx}: the advanced view shows only implicits, found {blocks:?}"
    );
    assert_eq!(
        blocks,
        implicit_blocks(&before),
        "{ctx}: the implicit blocks stay as before"
    );
    // And the item can be started again: it is a normal item for the Orb of Transmutation
    assert_eq!(
        can_apply_orb_of_transmutation(item),
        Ok(()),
        "{ctx}: the scoured item is normal for the Orb of Transmutation"
    );
    before
}

/// Asserts that hover and click both fail because the item is neither magic nor rare, and that
/// the click leaves the item unchanged.
fn assert_not_applicable(item: &Item, ctx: &str) {
    // When the page asks whether the orb applies (hover)
    let hover = can_apply_orb_of_scouring(item);
    // And the orb is clicked anyway
    let mut clicked = item.clone();
    let click = apply_orb_of_scouring(&mut clicked);
    // Then both fail because the item is neither magic nor rare
    assert_eq!(hover, Err(CraftError::ItemNotMagicOrRare), "{ctx}: hover");
    assert_eq!(click, Err(CraftError::ItemNotMagicOrRare), "{ctx}: click");
    // And the item is unchanged
    assert_eq!(&clicked, item, "{ctx}: the item is unchanged");
}

// ---------------------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------------------

// AC2: „Neben den bisherigen Orbs gibt es einen Orb of Scouring. Sein Name ist ein Spieltext. Ist
// der Orb anwendbar, zeigt das Überfahren mit der Maus genau den Text `Removes all modifiers from
// an item` (Spieldaten)."
// (The button's place and the tooltip are checked by the PO in the local preview; the wording of
// the name and the description at the acceptance. Here: the seven orbs exist, the new orb's name
// and description are the texts of its entry in the game data and are not written into the
// code, the orb applies to magic and rare rings with 0 mods up to full ones, and the page uses
// the new orb from crafting-core and shows its description.)
#[test]
fn ac2_orb_of_scouring_exists_with_its_game_name_and_description() {
    // Given the currencies of crafting-core
    let ids: Vec<String> = currencies().into_iter().map(|entry| entry.id).collect();

    // Then the Orb of Scouring exists next to the other orbs
    for id in EARLIER_ORB_IDS.into_iter().chain([ORB_ID]) {
        assert!(
            ids.iter().any(|found| found == id),
            "currencies() must contain {id:?}, found {ids:?}"
        );
    }
    assert_eq!(Currency::OrbOfScouring.id(), ORB_ID);

    // And its name is the game text of its entry in the game data
    let name = text_in_file(&["name"]);
    assert_eq!(Currency::OrbOfScouring.name(), name);
    assert_eq!(definition(ORB_ID).name, name);
    // And it is not written into the code of crafting-core or web
    assert_only_in_game_data(&name);

    // And its description is the game text of its entry in the game data
    let description = text_in_file(&["description"]);
    assert_eq!(Currency::OrbOfScouring.description(), description);
    assert_eq!(definition(ORB_ID).description, description);
    // And it is not written into the code of crafting-core or web
    assert_only_in_game_data(&description);

    // Given magic Iron, Composite and Cogwork Rings with 0 to 2 mods and rare ones with 0 to 6
    // mods, on several item levels
    let mut rng = SeededRng::new(SEED);
    for ring_id in [IRON_RING_ID, COMPOSITE_RING_ID, COGWORK_RING_ID] {
        let ring = base(ring_id);
        for item in magic_and_rare_items(&ring, &mut rng) {
            // When the page asks whether the orb applies
            // Then it does, so the tooltip is the description
            assert_eq!(
                can_apply_orb_of_scouring(&item),
                Ok(()),
                "{:?} {} of item level {} with {:?}",
                item.rarity,
                ring.name,
                item.item_level,
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
            "ChaosOrb",
            "OrbOfScouring",
            "can_apply_orb_of_scouring",
            "description()",
        ],
        "the Orb of Scouring's button comes from crafting-core, next to the other orbs, and its \
         tooltip is its description",
    );
    assert_web_src_applies_the_orb();
}

// AC3: „Auf einem magischen und auf einem seltenen Item entfernt der Orb alle Prefixes und
// Suffixes. Das Item wird normal und hat keinen Namen mehr. Implicits, Basis und Item-Level
// bleiben unverändert. Ein benötigtes Level, das nur von den Mods kam, entfällt. Beispiel: ein
// seltener und ein magischer Iron Ring auf Item-Level 83 werden zu einem normalen Iron Ring mit
// seinem Implicit, ohne Requirements-Zeile."
// (The page is checked by the PO in the local preview. Covered: the example with the PO's
// evidence, the magic "Dancer's Iron Ring of Victory" of item level 83 with level requirement 18,
// and rare Iron Rings of item level 83 with 3 prefixes and 3 suffixes as "Blight Finger"; each
// becomes a normal Iron Ring of item level 83 with its implicit, only the base name and no level
// requirement. And every ring base, including the Breach and Heist rings, magic with 0 to 2 mods
// and rare with 0 to 6 mods on item levels 1, 83 and 100: the result is normal without prefixes
// and suffixes, keeps base, implicits with their values and item level, has only the base name,
// the requirement of a normal item of the base, shows only its implicits, and can be started
// again with an Orb of Transmutation.)
#[test]
fn ac3_the_orb_removes_all_prefixes_and_suffixes_and_makes_the_item_normal() {
    let mut rng = SeededRng::new(SEED);
    let iron = base(IRON_RING_ID);
    let iron_pool = pool_of(IRON_RING_ID);
    let iron_implicits = iron.implicits.len();
    assert!(iron_implicits > 0, "the Iron Ring has its implicit");
    assert_eq!(
        normal_requirement(&iron, &mut rng),
        None,
        "a normal Iron Ring shows no level requirement (PO's evidence)"
    );

    // --- The story's example: the PO's magic "Dancer's Iron Ring of Victory" ---
    // Given the magic Iron Ring of item level 83 with "Dancer's" and "of Victory", which shows
    // the level requirement 18
    let mut magic = item_with(
        &iron,
        Rarity::Magic,
        EXAMPLE_ITEM_LEVEL,
        &[find(&iron_pool, DANCERS)],
        &[find(&iron_pool, OF_VICTORY)],
        &mut rng,
    );
    assert_eq!(
        magic.required_level(),
        Some(EVIDENCE_MAGIC_REQUIREMENT),
        "the PO's evidence: the magic Iron Ring shows level 18"
    );
    // When the orb is applied
    // Then it is a normal Iron Ring of item level 83 with its implicit, its base name and no
    // level requirement
    scour(&mut magic, &mut rng, "the PO's magic Iron Ring");
    assert_eq!(magic.item_level, EXAMPLE_ITEM_LEVEL);
    assert_eq!(magic.implicits.len(), iron_implicits);
    assert_eq!(magic.name(), iron.name);
    assert_eq!(magic.required_level(), None);

    // --- The story's example: rare Iron Rings of item level 83 with 3 prefixes and 3 suffixes ---
    let mut with_requirement = 0;
    for trial in 0..EXAMPLE_ITEMS {
        // Given a rare Iron Ring of item level 83 with 3 prefixes and 3 suffixes
        let mut rare = item_of_rarity(
            &iron,
            Rarity::Rare,
            EXAMPLE_ITEM_LEVEL,
            &iron_pool,
            6,
            &mut rng,
        );
        assert_eq!(
            (rare.prefixes.len(), rare.suffixes.len()),
            EVIDENCE_RARE_SPLIT,
            "the example needs a full rare Iron Ring"
        );
        if rare.required_level().is_some() {
            with_requirement += 1;
        }
        // When the orb is applied
        // Then it is a normal Iron Ring of item level 83 with its implicit, its base name and no
        // level requirement
        let ctx = format!("rare Iron Ring of item level 83, {trial}");
        scour(&mut rare, &mut rng, &ctx);
        assert_eq!(rare.item_level, EXAMPLE_ITEM_LEVEL, "{ctx}");
        assert_eq!(rare.implicits.len(), iron_implicits, "{ctx}");
        assert_eq!(rare.name(), iron.name, "{ctx}");
        assert_eq!(rare.required_level(), None, "{ctx}");
    }
    assert!(
        with_requirement > 0,
        "the example needs rare Iron Rings whose mods give a level requirement"
    );

    // --- Every ring base ---
    let rings = Slot::Ring.base_items();
    assert!(!rings.is_empty(), "the slot \"Ring\" must have ring bases");
    for ring in &rings {
        // Given magic rings with 0 to 2 mods and rare rings with 0 to 6 mods
        for mut item in magic_and_rare_items(ring, &mut rng) {
            // When the orb is applied
            // Then the result is the one the PO's rule gives
            let ctx = format!("{} of item level {}", ring.name, item.item_level);
            scour(&mut item, &mut rng, &ctx);
        }
    }
}

// AC4: „Auf einem normalen Item ist der Orb nicht anwendbar. Überfahren mit der Maus und Klick
// zeigen genau die Meldung `Failed to apply item: Target is not Magic or Rare.` Das Item bleibt
// unverändert. Im Kern gilt dasselbe für ein Unique-Item."
// (Tooltip, look and message area are checked by the PO in the local preview, the wording of the
// message at the acceptance. Here: the reason on hover and on click for normal rings of every
// ring base on several item levels, for rings the orb has just made normal, and in the core for
// unique rings without and with mods; its message from the game data; the unchanged item; and
// that the page marks the orb and shows the message.)
#[test]
fn ac4_the_orb_on_a_normal_or_unique_item_shows_the_game_message_and_keeps_the_item() {
    let mut rng = SeededRng::new(SEED);
    let unique_prefixes = invented_mods(AffixType::Prefix, 2);
    let unique_suffixes = invented_mods(AffixType::Suffix, 2);

    let rings = Slot::Ring.base_items();
    assert!(!rings.is_empty(), "the slot \"Ring\" must have ring bases");
    for ring in &rings {
        let pool = pool_of(&ring.id);
        for item_level in ITEM_LEVELS {
            for trial in 0..NOT_APPLICABLE_ITEMS {
                // Given a normal ring
                let mut normal = Item::new(ring, &mut rng);
                normal.item_level = item_level;
                assert_eq!(normal.rarity, Rarity::Normal);
                // And a ring the orb has just made normal
                let mut scoured = item_of_rarity(
                    ring,
                    if trial % 2 == 0 {
                        Rarity::Magic
                    } else {
                        Rarity::Rare
                    },
                    item_level,
                    &pool,
                    (trial + 1) as usize,
                    &mut rng,
                );
                assert_eq!(apply_orb_of_scouring(&mut scoured), Ok(()));
                // And a unique ring without mods and one with mods
                let unique = item_with(ring, Rarity::Unique, item_level, &[], &[], &mut rng);
                let n = (trial % 3) as usize;
                let unique_with_mods = item_with(
                    ring,
                    Rarity::Unique,
                    item_level,
                    &unique_prefixes.iter().take(n).collect::<Vec<_>>(),
                    &unique_suffixes.iter().take(2 - n).collect::<Vec<_>>(),
                    &mut rng,
                );

                for (label, item) in [
                    ("normal", normal),
                    ("scoured", scoured),
                    ("unique", unique),
                    ("unique with mods", unique_with_mods),
                ] {
                    let ctx = format!(
                        "{label} {} of item level {item_level}, {trial}, with {:?}",
                        ring.name,
                        outcome(&item)
                    );
                    // When the page asks whether the orb applies, and it is clicked anyway
                    // Then both fail because the item is neither magic nor rare, and the item is
                    // unchanged
                    assert_not_applicable(&item, &ctx);
                }
            }
        }
    }

    // And hover and click show the orb's message for this reason from the game data
    assert_eq!(
        CraftError::ItemNotMagicOrRare.code(),
        NOT_MAGIC_OR_RARE_CODE
    );
    let message = text_in_file(&["messages", NOT_MAGIC_OR_RARE_CODE]);
    assert_eq!(
        definition(ORB_ID)
            .messages
            .get(NOT_MAGIC_OR_RARE_CODE)
            .map(String::as_str),
        Some(message.as_str()),
        "currencies(): message of {ORB_ID:?} for {NOT_MAGIC_OR_RARE_CODE:?}"
    );
    assert_eq!(
        Currency::OrbOfScouring.failure_message(CraftError::ItemNotMagicOrRare),
        message,
        "the message of {ORB_ID:?} for an item that is neither magic nor rare is the game data's"
    );
    // And it is not written into the code of crafting-core or web
    assert_only_in_game_data(&message);
    // And the page marks the orb as not applicable and shows the message on hover and click
    assert_web_src_uses(
        &[
            "can_apply_orb_of_scouring",
            "failure_message",
            "aria-disabled",
        ],
        "an inapplicable Orb of Scouring is marked and shows its failure message",
    );
    assert_web_src_applies_the_orb();
}
