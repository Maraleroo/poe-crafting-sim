//! Acceptance tests for AB#282 "Advanced Mod Descriptions: Tier, Name und Tags der Mods
//! anzeigen".
//!
//! The tests use the public interface from the tech lead's plan on AB#282 and ADR 0024:
//! - `Item::advanced_mods(&self) -> Vec<ModBlock>`: one block per mod, nothing merged; the
//!   implicits in the order of the base, then the prefixes, then the suffixes, each in the order
//!   they were added.
//! - `ModBlock` (`#[non_exhaustive]`, so the tests only read its fields): `kind: ModKind`,
//!   `name: Option<String>`, `tier: Option<u32>`, `tags: String`, `lines: Vec<Vec<TextPart>>`
//!   (unscaled values) and `magnitude_increase: Option<u32>`.
//! - `ModKind { Implicit, Prefix, Suffix }` and `TextPart { Text(String), Range(String) }`: the
//!   parts of a line, put together, are the line of the extended item text; a `Range` holds the
//!   range directly after a value, with its brackets.
//! - `Item::tier(&self, explicit: &ExplicitMod) -> u32`, `Mod::tag_text(&self) -> String` and
//!   `Mod::extended_lines(&self) -> Vec<Vec<TextPart>>`.
//! - unchanged: `Mod::display_text`, `Item::explicit_texts`, `add_random_mod`, `valid_pool`,
//!   `unblocked_mods`, `mod_pool`, `explicit_mods`, `base_item`, `Slot::Ring.base_items()`.
//!
//! The PO's rules are `poe-crafting-rules`: the section "Advanced mod descriptions" (criterion 1),
//! the paragraph "In an extended item text (Ctrl+Alt+C) ..." with its line on values with
//! decimals (criterion 9), the paragraphs on converted values and on "X% increased ... Modifier
//! magnitudes" (AB#185, AB#189). The wording of criteria 1, 2 and 9 is checked byte for byte at
//! the acceptance (step 2 of the `story` skill), not here; the tests check the behaviour of the
//! rules.
//!
//! How the tests read a line: adjacent `Text` parts are joined and empty ones dropped, because
//! only the line and the place of each range are behaviour, not how the text around a value is
//! cut into parts. Every expected value is computed from the rules and the mod's data, not taken
//! from the implementation: a converted number is the text that the protected
//! `Mod::display_text` (AB#185) gives for a mod with only that stat and value.
//!
//! Every random test uses a fixed seed and depends only on invariants, never on the order in
//! which the generator is used.
//!
//! Red state before the implementation: the module does not compile, because `ModBlock`,
//! `ModKind`, `TextPart`, `Item::advanced_mods`, `Item::tier`, `Mod::tag_text` and
//! `Mod::extended_lines` do not exist yet.
//!
//! Criteria without a permanent test here:
//! - AC2 and the parts of AC1 on fractured mods: the simulator has no fractured mods (AC8).
//! - AC3, apart from the translation keys: the start with "Default Mod Descriptions" and that the
//!   Alt key does nothing are behaviour of the rendered page, checked by the PO in the local
//!   preview. That the choice is not stored is protected by `story_39::ac6` (no storage APIs
//!   under `web/`) and checked in the preview.
//! - AC4: the normal view stays as it is; it is protected by the tests of AB#258, AB#189 and
//!   AB#52 and checked in the preview.
//! - AC5, the layout (short label on the left, golden tier box on the right of the first line,
//!   small white semibold name, "·", the range set apart by its look) and AC7 (390 px without
//!   horizontal scrolling): checked by the PO in the local preview (ADR 0004, ADR 0024). Here:
//!   the content of the blocks and that the texts come from the translations.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crafting_core::{
    add_random_mod, base_item, explicit_mods, mod_pool, unblocked_mods, valid_pool, AffixType,
    BaseItem, Conversion, ExplicitMod, ExplicitModDefinition, Item, Mod, ModBlock, ModKind, Rarity,
    Rng, SeededRng, Slot, Stat, TextPart,
};
use serde_json::Value;

// ---------------------------------------------------------------------------------------
// Values from the story, the PO's rules and the RePoE export 3.29.3.3
// ---------------------------------------------------------------------------------------

/// The ring of the PO's examples "Magpie's Iron Ring of Radiance" and "... of Raiding"; its
/// implicit has two fixed values.
const IRON_RING_ID: &str = "Metadata/Items/Rings/Ring1";
/// The ring of the PO's copy "Sapphire Ring of the Flatworm".
const SAPPHIRE_RING_ID: &str = "Metadata/Items/Rings/Ring6";
/// Its implicit has two values with ranges, the PO's evidence "Adds 3(3-4) to 10(10-14) ...".
const STEEL_RING_ID: &str = "Metadata/Items/Rings/RingAtlas1";
/// Its implicit has the tags chaos_damage, damage, chaos (the PO's copy of an Iolite Ring).
const IOLITE_RING_ID: &str = "Metadata/Items/Rings/RingAtlas5";
/// Its implicit "Has 1 Socket" has no tags.
const UNSET_RING_ID: &str = "Metadata/Items/Rings/Ring15";
/// The base "Ring", without implicits.
const MIRROR_RING_ID: &str = "Metadata/Items/Rings/MirrorRing";
/// Its implicit has two lines, one with a negated fixed value.
const DUSK_RING_ID: &str = "Metadata/Items/Rings/RingK1";
/// A Breach ring with the damage type lock.
const CRYONIC_RING_ID: &str = "Metadata/Items/Rings/RingB1";
/// A ring with a larger pool than the Iron Ring.
const BONE_RING_ID: &str = "Metadata/Items/Rings/Ring16";
/// Heist rings (AB#189).
const COGWORK_RING_ID: &str = "Metadata/Items/Rings/RingE1";
const GEODESIC_RING_ID: &str = "Metadata/Items/Rings/RingE2";
const COMPOSITE_RING_ID: &str = "Metadata/Items/Rings/RingE3";
const MANIFOLD_RING_ID: &str = "Metadata/Items/Rings/RingE4";
const HELICAL_RING_ID: &str = "Metadata/Items/Rings/RingE6";

/// The increases of the Heist rings by side, as (ring, prefix %, suffix %), from the RePoE export
/// (AB#189). Every other ring base has none.
const MAGNITUDE_INCREASES: [(&str, u32, u32); 4] = [
    (COGWORK_RING_ID, 0, 25),
    (GEODESIC_RING_ID, 25, 0),
    (MANIFOLD_RING_ID, 50, 0),
    (HELICAL_RING_ID, 0, 50),
];

/// The tags that the rule never shows.
const HIDDEN_TAGS: [&str; 5] = [
    "elemental_damage",
    "physical_damage",
    "chaos_damage",
    "flat_life_regen",
    "resource",
];

/// Ring bases for the order of the blocks: different implicits (none, without tags, two values
/// with ranges, two lines, several lines of a Heist ring, the hidden tag chaos_damage), a locked
/// Breach ring and a larger pool.
const ORDER_BASES: [&str; 9] = [
    IRON_RING_ID,
    STEEL_RING_ID,
    IOLITE_RING_ID,
    UNSET_RING_ID,
    MIRROR_RING_ID,
    DUSK_RING_ID,
    COGWORK_RING_ID,
    CRYONIC_RING_ID,
    BONE_RING_ID,
];

/// Ring bases for the increase: the four rings with an increase, a Heist ring without one and
/// the Iron Ring.
const INCREASE_BASES: [&str; 6] = [
    COGWORK_RING_ID,
    GEODESIC_RING_ID,
    MANIFOLD_RING_ID,
    HELICAL_RING_ID,
    COMPOSITE_RING_ID,
    IRON_RING_ID,
];

/// The UI texts of criterion 3.
const TOGGLE_TEXTS: [&str; 2] = ["Default Mod Descriptions", "Advanced Mod Descriptions"];
/// The UI texts of criterion 5: the short labels and their names.
const LABEL_TEXTS: [&str; 6] = ["I", "P", "S", "Implicit", "Prefix", "Suffix"];
/// The mark of an increased mod ("— X% Increased"), a label of the page.
const INCREASE_MARK: &str = "% Increased";

const RARITY_STAT: &str = "base_item_found_rarity_+%";
const ACCURACY_STAT: &str = "accuracy_rating_+%";
const LIGHT_RADIUS_STAT: &str = "light_radius_+%";
const LIFE_REGENERATION_STAT: &str = "base_life_regeneration_rate_per_minute";
const LIFE_LEECH_STAT: &str = "life_leech_from_physical_attack_damage_permyriad";
const MINIMUM_PHYSICAL_STAT: &str = "attack_minimum_added_physical_damage";
const MAXIMUM_PHYSICAL_STAT: &str = "attack_maximum_added_physical_damage";

/// A fixed seed, so every run is identical.
const SEED: u64 = 282;
/// Rare items per base of `ORDER_BASES` (AC1), each with up to six mods added in random order.
const ORDER_ITEMS: usize = 6;
/// Rare items per base of `INCREASE_BASES` (AC5), each filled with random mods.
const INCREASE_ITEMS: usize = 3;
/// Random rolls per mod of the game data, besides all values at their minimum and maximum (AC5).
const ROLLS_PER_MOD: usize = 5;

// ---------------------------------------------------------------------------------------
// Helpers: files
// ---------------------------------------------------------------------------------------

fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crafting-core must live inside the repository root")
}

fn read_path(path: &Path) -> String {
    let bytes = fs::read(path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    String::from_utf8_lossy(&bytes).into_owned()
}

/// All files below `dir`, without the entries `skipped` directly below it.
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

/// All files under `web/` outside the translation files, without the build output in
/// `web/dist` and `web/target`.
fn web_files_outside_translations() -> Vec<PathBuf> {
    let web = repo_root().join("web");
    let i18n = web.join("i18n");
    let files: Vec<PathBuf> = files_below(&web, &["dist", "target"])
        .into_iter()
        .filter(|path| !path.starts_with(&i18n))
        .collect();
    assert!(
        files
            .iter()
            .any(|path| path.extension() == Some("rs".as_ref())),
        "web/ must contain the source files of the page"
    );
    files
}

/// The values of the English translation file.
fn english_translations() -> BTreeSet<String> {
    let path = repo_root().join("web").join("i18n").join("en.json");
    let file: Value = serde_json::from_str(&read_path(&path))
        .unwrap_or_else(|e| panic!("web/i18n/en.json is not valid JSON: {e}"));
    file.as_object()
        .expect("web/i18n/en.json must be an object of keys and texts")
        .values()
        .map(|text| {
            text.as_str()
                .expect("every text of web/i18n/en.json is a string")
                .to_owned()
        })
        .collect()
}

/// Asserts that every text is a value of `web/i18n/en.json` and that no file under `web/`
/// outside the translation files contains one of the `forbidden` strings.
fn assert_texts_come_from_translations(texts: &[&str], forbidden: &[String]) {
    let translations = english_translations();
    let missing: Vec<&&str> = texts
        .iter()
        .filter(|text| !translations.contains(**text))
        .collect();
    assert!(
        missing.is_empty(),
        "the texts {missing:?} must be English texts in web/i18n/en.json"
    );
    let mut hard_coded = Vec::new();
    for path in web_files_outside_translations() {
        let content = read_path(&path);
        for needle in forbidden {
            if content.contains(needle.as_str()) {
                hard_coded.push(format!("{} contains {needle}", path.display()));
            }
        }
    }
    assert!(
        hard_coded.is_empty(),
        "UI texts must come from the translation files in web/i18n/: {hard_coded:#?}"
    );
}

fn assert_web_src_uses(needle: &str, why: &str) {
    let used = files_below(&repo_root().join("web").join("src"), &[])
        .iter()
        .any(|path| read_path(path).contains(needle));
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

/// A random index below `len`.
fn index(rng: &mut SeededRng, len: usize) -> usize {
    usize::try_from(rng.next_u64() % len as u64).expect("an index fits")
}

/// A new item of `base` with the given rarity and without prefixes and suffixes.
fn item_of(base: &BaseItem, rarity: Rarity, rng: &mut SeededRng) -> Item {
    let mut item = Item::new(base, rng);
    item.rarity = rarity;
    item
}

/// The mod of `definition`, rolled, as a prefix or suffix.
fn rolled(definition: &ExplicitModDefinition, rng: &mut SeededRng) -> ExplicitMod {
    ExplicitMod {
        name: definition.name.clone(),
        affix_type: definition.affix_type,
        modifier: Mod::roll(&definition.definition, rng),
    }
}

/// Adds the mod of `definition` with `add_random_mod`, as the only mod it can choose, and
/// returns it as it is on the item.
fn add_definition(
    item: &mut Item,
    definition: &ExplicitModDefinition,
    rng: &mut SeededRng,
) -> ExplicitMod {
    let only = std::slice::from_ref(definition);
    assert!(
        add_random_mod(item, only, rng),
        "{:?} can be added",
        definition.name
    );
    side_of(item, definition.affix_type)
        .iter()
        .find(|explicit| explicit.name == definition.name)
        .cloned()
        .unwrap_or_else(|| panic!("{:?} is on the item after adding it", definition.name))
}

/// Adds the mod `name` of `pool` and gives its stat `stat` the raw value `value`.
fn add_with_value(
    item: &mut Item,
    pool: &[ExplicitModDefinition],
    name: &str,
    stat: &str,
    value: i32,
    rng: &mut SeededRng,
) {
    let definition = find(pool, name);
    add_definition(item, definition, rng);
    let side = match definition.affix_type {
        AffixType::Prefix => &mut item.prefixes,
        AffixType::Suffix => &mut item.suffixes,
    };
    let explicit = side
        .iter_mut()
        .find(|explicit| explicit.name == name)
        .expect("added above");
    set_value(&mut explicit.modifier, stat, value, name);
}

fn side_of(item: &Item, affix_type: AffixType) -> &[ExplicitMod] {
    match affix_type {
        AffixType::Prefix => &item.prefixes,
        AffixType::Suffix => &item.suffixes,
    }
}

/// Sets the value of the stat `stat` of `modifier`, which must lie in its range.
fn set_value(modifier: &mut Mod, stat: &str, value: i32, what: &str) {
    let rolled = modifier
        .stats
        .iter_mut()
        .find(|rolled| rolled.id == stat)
        .unwrap_or_else(|| panic!("{what}: the mod must have the stat {stat:?}"));
    assert!(
        (rolled.min..=rolled.max).contains(&value),
        "{what}: {value} lies outside {}..={} of {stat:?}",
        rolled.min,
        rolled.max
    );
    rolled.value = value;
}

/// An invented mod: the text names each stat by its identifier, each stat is (id, min, max,
/// value).
fn invented_mod(
    text: &str,
    tags: &[&str],
    stats: &[(&str, i32, i32, i32)],
    conversions: &[(&str, Conversion)],
) -> Mod {
    Mod {
        family: "TestFamily".to_owned(),
        level: 1,
        tags: tags.iter().map(|tag| (*tag).to_owned()).collect(),
        stats: stats
            .iter()
            .map(|(id, min, max, value)| Stat {
                id: (*id).to_owned(),
                min: *min,
                max: *max,
                value: *value,
            })
            .collect(),
        text: text.to_owned(),
        placeholders: stats
            .iter()
            .map(|(id, ..)| ((*id).to_owned(), (*id).to_owned()))
            .collect(),
        conversions: conversions
            .iter()
            .map(|(id, conversion)| ((*id).to_owned(), conversion.clone()))
            .collect(),
    }
}

/// Every mod of the game data and every implicit of every ring base, rolled.
fn all_mods(rng: &mut SeededRng) -> Vec<(String, Mod)> {
    let mut mods: Vec<(String, Mod)> = explicit_mods()
        .iter()
        .map(|definition| {
            (
                definition.name.clone(),
                Mod::roll(&definition.definition, rng),
            )
        })
        .collect();
    for base in Slot::Ring.base_items() {
        for implicit in &base.implicits {
            mods.push((
                format!("implicit of {}", base.name),
                Mod::roll(implicit, rng),
            ));
        }
    }
    assert!(!mods.is_empty(), "the game data must contain mods");
    mods
}

/// Rare items of `base` at item level 1 that together hold every mod of `pool` that can roll
/// on the base, each item at most three prefixes and three suffixes of different families.
fn packed_items(base: &BaseItem, pool: &[ExplicitModDefinition], rng: &mut SeededRng) -> Vec<Item> {
    let placeable = unblocked_mods(&base.id, pool);
    let mut prefixes: Vec<&ExplicitModDefinition> = placeable
        .iter()
        .filter(|definition| definition.affix_type == AffixType::Prefix)
        .collect();
    let mut suffixes: Vec<&ExplicitModDefinition> = placeable
        .iter()
        .filter(|definition| definition.affix_type == AffixType::Suffix)
        .collect();
    let mut items = Vec::new();
    while !prefixes.is_empty() || !suffixes.is_empty() {
        let mut item = item_of(base, Rarity::Rare, rng);
        item.item_level = 1;
        item.prefixes = take_of_different_families(&mut prefixes, rng);
        item.suffixes = take_of_different_families(&mut suffixes, rng);
        items.push(item);
    }
    items
}

/// Takes up to three mods of different families from `rest` and rolls them.
fn take_of_different_families(
    rest: &mut Vec<&ExplicitModDefinition>,
    rng: &mut SeededRng,
) -> Vec<ExplicitMod> {
    let mut families = BTreeSet::new();
    let mut taken = Vec::new();
    let mut position = 0;
    while taken.len() < 3 && position < rest.len() {
        if families.insert(rest[position].definition.family.clone()) {
            let definition = rest.remove(position);
            taken.push(rolled(definition, rng));
        } else {
            position += 1;
        }
    }
    taken
}

/// One ring base for each different pool of the game data, with its pool.
fn bases_with_different_pools() -> Vec<(BaseItem, Vec<ExplicitModDefinition>)> {
    let mut chosen: Vec<(BaseItem, Vec<ExplicitModDefinition>)> = Vec::new();
    for base in Slot::Ring.base_items() {
        let pool = pool_of(&base.id);
        let known = chosen.iter().any(|(_, other)| {
            other.len() == pool.len() && pool.iter().all(|definition| other.contains(definition))
        });
        if !known {
            chosen.push((base, pool));
        }
    }
    chosen
}

// ---------------------------------------------------------------------------------------
// Helpers: reading the blocks
// ---------------------------------------------------------------------------------------

/// A part of a line as the tests compare it.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Part {
    Text(String),
    Range(String),
}

/// A block as the tests compare it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Block {
    kind: &'static str,
    name: Option<String>,
    tier: Option<u32>,
    tags: String,
    lines: Vec<Vec<Part>>,
    magnitude_increase: Option<u32>,
}

fn kind_of(kind: &ModKind) -> &'static str {
    match kind {
        ModKind::Implicit => "implicit",
        ModKind::Prefix => "prefix",
        ModKind::Suffix => "suffix",
    }
}

fn kind_of_side(affix_type: AffixType) -> &'static str {
    match affix_type {
        AffixType::Prefix => "prefix",
        AffixType::Suffix => "suffix",
    }
}

/// The parts of a line with adjacent text parts joined and empty text parts dropped.
fn parts_of(line: &[TextPart]) -> Vec<Part> {
    let mut parts: Vec<Part> = Vec::new();
    for part in line {
        match part {
            TextPart::Text(text) => {
                if text.is_empty() {
                    continue;
                }
                match parts.last_mut() {
                    Some(Part::Text(last)) => last.push_str(text),
                    _ => parts.push(Part::Text(text.clone())),
                }
            }
            TextPart::Range(range) => parts.push(Part::Range(range.clone())),
        }
    }
    parts
}

fn lines_of(lines: &[Vec<TextPart>]) -> Vec<Vec<Part>> {
    lines.iter().map(|line| parts_of(line)).collect()
}

fn block_of(block: &ModBlock) -> Block {
    Block {
        kind: kind_of(&block.kind),
        name: block.name.clone(),
        tier: block.tier,
        tags: block.tags.clone(),
        lines: lines_of(&block.lines),
        magnitude_increase: block.magnitude_increase,
    }
}

fn blocks_of(item: &Item) -> Vec<Block> {
    item.advanced_mods().iter().map(block_of).collect()
}

/// The line as the extended item text shows it: all parts put together.
fn line_text(parts: &[Part]) -> String {
    parts
        .iter()
        .map(|part| match part {
            Part::Text(text) | Part::Range(text) => text.as_str(),
        })
        .collect()
}

/// A block of one line, as the examples of AC6 compare it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct OneLine {
    kind: &'static str,
    name: Option<String>,
    tier: Option<u32>,
    line: String,
    ranges: Vec<String>,
    magnitude_increase: Option<u32>,
}

fn one_line(block: &Block) -> OneLine {
    assert_eq!(block.lines.len(), 1, "{:?} has one line", block.name);
    OneLine {
        kind: block.kind,
        name: block.name.clone(),
        tier: block.tier,
        line: line_text(&block.lines[0]),
        ranges: ranges_of(&block.lines[0])
            .into_iter()
            .map(str::to_owned)
            .collect(),
        magnitude_increase: block.magnitude_increase,
    }
}

/// The expected block of the explicit mod `name` with one line and one range.
fn expected_one_line(
    kind: &'static str,
    name: &str,
    tier: u32,
    line: String,
    range: &str,
    magnitude_increase: Option<u32>,
) -> OneLine {
    OneLine {
        kind,
        name: Some(name.to_owned()),
        tier: Some(tier),
        line,
        ranges: vec![range.to_owned()],
        magnitude_increase,
    }
}

/// The range parts of a line, in their order.
fn ranges_of(parts: &[Part]) -> Vec<&str> {
    parts
        .iter()
        .filter_map(|part| match part {
            Part::Range(range) => Some(range.as_str()),
            Part::Text(_) => None,
        })
        .collect()
}

// ---------------------------------------------------------------------------------------
// Helpers: the PO's rules
// ---------------------------------------------------------------------------------------

/// The raw value `raw` of the stat `stat_id` of `modifier` as the game shows it: the text that
/// `Mod::display_text` gives for a mod with only this stat and value.
fn shown(modifier: &Mod, stat_id: &str, raw: i32) -> String {
    let single = Mod {
        family: modifier.family.clone(),
        level: modifier.level,
        tags: Vec::new(),
        stats: vec![Stat {
            id: stat_id.to_owned(),
            min: raw,
            max: raw,
            value: raw,
        }],
        text: "{value}".to_owned(),
        placeholders: BTreeMap::from([("value".to_owned(), stat_id.to_owned())]),
        conversions: modifier
            .conversions
            .iter()
            .filter(|(stat, _)| stat.as_str() == stat_id)
            .map(|(stat, conversion)| (stat.clone(), conversion.clone()))
            .collect(),
    };
    single.display_text()
}

/// The lines of `modifier` in the extended item text, with its rolled (unscaled) values: each
/// value as the game shows it, and after a value whose stat has a range (minimum less than
/// maximum) the range `(minimum-maximum)` with both bounds shown the same way.
///
/// Panics on a range of a negative or a negated value, which the rules do not define.
fn expected_lines(modifier: &Mod) -> Vec<Vec<Part>> {
    modifier
        .text
        .split('\n')
        .map(|line| {
            let mut parts = Vec::new();
            let mut text = String::new();
            let mut rest = line;
            while let Some(start) = rest.find('{') {
                let end = start
                    + rest[start..]
                        .find('}')
                        .unwrap_or_else(|| panic!("unclosed placeholder in {line:?}"));
                text.push_str(&rest[..start]);
                let name = &rest[start + 1..end];
                let stat_id = modifier.placeholders.get(name).unwrap_or_else(|| {
                    panic!("the placeholder {name:?} of {line:?} names no stat")
                });
                let stat = modifier
                    .stats
                    .iter()
                    .find(|stat| &stat.id == stat_id)
                    .unwrap_or_else(|| panic!("the mod has no stat {stat_id:?} for {line:?}"));
                text.push_str(&shown(modifier, stat_id, stat.value));
                if stat.min < stat.max {
                    let negated = modifier
                        .conversions
                        .get(stat_id)
                        .is_some_and(|conversion| conversion.divisor < 0);
                    assert!(
                        stat.min >= 0 && !negated,
                        "{line:?}: the range of a negative or negated value is not defined by \
                         poe-crafting-rules"
                    );
                    parts.push(Part::Text(std::mem::take(&mut text)));
                    parts.push(Part::Range(format!(
                        "({}-{})",
                        shown(modifier, stat_id, stat.min),
                        shown(modifier, stat_id, stat.max)
                    )));
                }
                rest = &rest[end + 1..];
            }
            text.push_str(rest);
            if !text.is_empty() {
                parts.push(Part::Text(text));
            }
            parts
        })
        .collect()
}

/// The tags of the header by the rule: in the order of the data, without the hidden tags, each
/// word (split at `_`) with an uppercase first letter, separated by ", ".
fn expected_tags(tags: &[String]) -> String {
    tags.iter()
        .filter(|tag| !HIDDEN_TAGS.contains(&tag.as_str()))
        .map(|tag| {
            tag.split('_')
                .map(|word| {
                    let mut chars = word.chars();
                    match chars.next() {
                        Some(first) => first.to_uppercase().chain(chars).collect(),
                        None => String::new(),
                    }
                })
                .collect::<Vec<String>>()
                .join(" ")
        })
        .collect::<Vec<String>>()
        .join(", ")
}

/// The tier by the rule: 1 plus the number of mods in the base's pool on the same side and of
/// the same family with a higher level.
fn expected_tier(pool: &[ExplicitModDefinition], explicit: &ExplicitMod) -> u32 {
    let higher = pool
        .iter()
        .filter(|definition| {
            definition.affix_type == explicit.affix_type
                && definition.definition.family == explicit.modifier.family
                && definition.definition.level > explicit.modifier.level
        })
        .count();
    1 + u32::try_from(higher).expect("a count fits")
}

/// The increase of the ring for a side, 0 for every other base.
fn increase_of(base_id: &str, affix_type: AffixType) -> u32 {
    MAGNITUDE_INCREASES
        .iter()
        .find(|(id, _, _)| *id == base_id)
        .map_or(0, |(_, prefixes, suffixes)| match affix_type {
            AffixType::Prefix => *prefixes,
            AffixType::Suffix => *suffixes,
        })
}

/// The blocks of the advanced view by the rule: the implicits of the item in their order, then
/// `prefixes`, then `suffixes`, each in the given order (the order they were added).
fn expected_blocks(
    item: &Item,
    pool: &[ExplicitModDefinition],
    prefixes: &[ExplicitMod],
    suffixes: &[ExplicitMod],
) -> Vec<Block> {
    let mut blocks: Vec<Block> = item
        .implicits
        .iter()
        .map(|implicit| Block {
            kind: "implicit",
            name: None,
            tier: None,
            tags: expected_tags(&implicit.tags),
            lines: expected_lines(implicit),
            magnitude_increase: None,
        })
        .collect();
    for (side, affix_type) in [(prefixes, AffixType::Prefix), (suffixes, AffixType::Suffix)] {
        for explicit in side {
            assert_eq!(
                explicit.affix_type, affix_type,
                "{} is on its side",
                explicit.name
            );
            let increase = increase_of(&item.base.id, affix_type);
            blocks.push(Block {
                kind: kind_of_side(affix_type),
                name: Some(explicit.name.clone()),
                tier: Some(expected_tier(pool, explicit)),
                tags: expected_tags(&explicit.modifier.tags),
                lines: expected_lines(&explicit.modifier),
                magnitude_increase: (increase > 0).then_some(increase),
            });
        }
    }
    blocks
}

/// The text of `modifier` with the placeholder of each stat in `values` replaced by its text,
/// one entry per line. Every placeholder of the text must be replaced.
fn text_with(modifier: &Mod, values: &[(&str, &str)]) -> Vec<String> {
    let mut text = modifier.text.clone();
    for (name, stat) in &modifier.placeholders {
        let placeholder = format!("{{{name}}}");
        if !text.contains(&placeholder) {
            continue;
        }
        let value = values
            .iter()
            .find(|(id, _)| id == stat)
            .unwrap_or_else(|| panic!("no value given for the stat {stat:?} of {text:?}"))
            .1;
        text = text.replace(&placeholder, value);
    }
    text.split('\n').map(str::to_owned).collect()
}

/// The text of `modifier` with the raw value `value` for its stat `stat`, as `Mod::display_text`
/// shows it.
fn display_with(modifier: &Mod, stat: &str, value: i32) -> String {
    let mut changed = modifier.clone();
    let rolled = changed
        .stats
        .iter_mut()
        .find(|rolled| rolled.id == stat)
        .unwrap_or_else(|| panic!("the mod must have the stat {stat:?}"));
    rolled.value = value;
    changed.display_text()
}

/// A value multiplied by (1 + X/100) and rounded down to a whole number.
fn scaled(value: i32, increase: u32) -> i32 {
    assert!(
        value >= 0,
        "rounding down a scaled negative value is not evidenced"
    );
    let value = i64::from(value) * i64::from(100 + increase) / 100;
    i32::try_from(value).expect("a scaled value fits")
}

// ---------------------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------------------

// AC1: „Regel des PO, Wortlaut für poe-crafting-rules (verbindlich), als neuer Abschnitt
// ## Advanced mod descriptions direkt nach dem Absatz zur normalen Ansicht:
// The advanced view (Alt in the game) shows each mod as its own block; nothing is merged.
// Implicit mods come first, then prefixes, then suffixes. Within each side, a fractured mod
// comes first; the other mods follow in the order they were added. A block shows the mod's lines
// with each value and its range as in the extended item text, its tags, and for a prefix or
// suffix its name and tier.
// A mod's tier is 1 plus the number of mods in the base's pool on the same side and of the same
// family with a higher level. The item level does not matter.
// The header of a mod shows its tags from the game data in their order, each word capitalised and
// with spaces instead of underscores, separated by commas. The tags elemental_damage,
// physical_damage, chaos_damage, flat_life_regen and resource are never shown. An implicit mod
// without tags shows no tags.
// - Evidence: the PO's Ctrl+Alt+C recordings (poe_orb_test, Bone Ring and Composite Ring): tiers
// of all 297 different mods and tags of all 11,425 mod headers fit. Copies by the PO on
// 2026-11-01: Sapphire Ring of the Flatworm ("of the Flatworm" Tier 5, tags "Life"; data:
// flat_life_regen, resource, life), rare Unset Ring (Tiers 3, 8, 2, 4; "Gem", "Attribute",
// "Damage, Elemental, Lightning", "Drop"), Iolite Ring (implicit tags "Damage, Chaos"; data:
// chaos_damage, damage, chaos). Order and fractured mods: rare Wyvernscale Boots "Dusk Spark",
// screenshots of the PO on 2026-11-01 before and after a Fracturing Orb (a fractured suffix and a
// fractured prefix)."
// (The wording is checked at the acceptance. Fractured mods are not tested (AC8). The tier is
// checked for every mod of every different ring pool at item level 1, so an implementation that
// counted only mods the item level allows would give tier 1 everywhere; the PO's examples are
// also checked at item level 100. The data has no tag with an underscore besides the hidden
// ones, so an invented mod checks words and underscores. The Unset Ring examples of the evidence
// name no mods and are not used.)
#[test]
fn ac1_blocks_follow_the_rule_for_order_tier_and_tags() {
    let mut rng = SeededRng::new(SEED);

    // --- Order: implicits, then prefixes, then suffixes, each in the order added; one block
    // per mod.
    // Given rare rings of different bases, with up to six mods added in random order
    let mut prefix_before_suffix = false;
    let mut suffix_before_prefix = false;
    for base_id in ORDER_BASES {
        let base = base(base_id);
        let pool = pool_of(base_id);
        for _ in 0..ORDER_ITEMS {
            let mut item = item_of(&base, Rarity::Rare, &mut rng);
            let mut prefixes = Vec::new();
            let mut suffixes = Vec::new();
            let mut added = Vec::new();
            let count = 1 + index(&mut rng, 6);
            for _ in 0..count {
                let valid = valid_pool(&item, &pool);
                if valid.is_empty() {
                    break;
                }
                let choice = &valid[index(&mut rng, valid.len())];
                let explicit = add_definition(&mut item, choice, &mut rng);
                if choice.affix_type == AffixType::Prefix {
                    suffix_before_prefix |= !suffixes.is_empty();
                    prefixes.push(explicit);
                } else {
                    prefix_before_suffix |= !prefixes.is_empty();
                    suffixes.push(explicit);
                }
                added.push(choice.name.clone());
            }

            // When showing the advanced view
            let blocks = blocks_of(&item);

            // Then there is one block per mod, in the order of the rule, each with its content
            assert_eq!(
                blocks,
                expected_blocks(&item, &pool, &prefixes, &suffixes),
                "{} with the mods {added:?} added in this order",
                base.name
            );
        }
    }
    assert!(
        prefix_before_suffix && suffix_before_prefix,
        "the items must have prefixes added before suffixes and suffixes added before prefixes"
    );

    // Given Iron Rings with the same four mods added in two different orders; Magpie's and of
    // Raiding have the same stat, which the normal view merges
    let iron_ring = base(IRON_RING_ID);
    let iron_pool = pool_of(IRON_RING_ID);
    for (order, expected) in [
        (
            ["Hale", "of the Brute", "Magpie's", "of Raiding"],
            ["Hale", "Magpie's", "of the Brute", "of Raiding"],
        ),
        (
            ["of Raiding", "Magpie's", "of the Brute", "Hale"],
            ["Magpie's", "Hale", "of Raiding", "of the Brute"],
        ),
    ] {
        let mut item = item_of(&iron_ring, Rarity::Rare, &mut rng);
        for name in order {
            add_definition(&mut item, find(&iron_pool, name), &mut rng);
        }

        // When showing the advanced view
        let blocks = blocks_of(&item);

        // Then each mod has its own block: the implicit first, then the prefixes, then the
        // suffixes, each side in the order the mods were added
        let shown: Vec<(&str, Option<&str>)> = blocks
            .iter()
            .map(|block| (block.kind, block.name.as_deref()))
            .collect();
        assert_eq!(
            shown,
            [
                ("implicit", None),
                ("prefix", Some(expected[0])),
                ("prefix", Some(expected[1])),
                ("suffix", Some(expected[2])),
                ("suffix", Some(expected[3])),
            ],
            "mods added in the order {order:?}"
        );
    }

    // --- Tier: 1 plus the number of mods of the base's pool on the same side and of the same
    // family with a higher level; the item level does not matter.
    // Given one ring base for each different pool of the game data, and rare items of item level
    // 1 that together hold every mod of that pool that can roll on the base
    let pools = bases_with_different_pools();
    assert!(
        pools.len() > 1,
        "the ring bases of the game data have different pools"
    );
    for (base, pool) in &pools {
        let mut tiers = BTreeSet::new();
        for item in packed_items(base, pool, &mut rng) {
            // When showing the advanced view
            let blocks = blocks_of(&item);

            // Then every prefix and suffix shows the tier of the rule, counted in the base's pool
            assert_eq!(
                blocks,
                expected_blocks(&item, pool, &item.prefixes, &item.suffixes),
                "{} at item level 1",
                base.name
            );
            tiers.extend(blocks.iter().filter_map(|block| block.tier));
        }
        assert!(
            tiers.len() > 1,
            "{}: the pool has mods of several tiers",
            base.name
        );
    }

    // Given the PO's examples at item level 1 and 100
    for (base_id, name, tier) in [
        (IRON_RING_ID, "Magpie's", 4),
        (IRON_RING_ID, "of Radiance", 1),
        (IRON_RING_ID, "of Raiding", 3),
        (SAPPHIRE_RING_ID, "of the Flatworm", 5),
    ] {
        let pool = pool_of(base_id);
        for item_level in [1, 100] {
            let mut item = item_of(&base(base_id), Rarity::Magic, &mut rng);
            item.item_level = item_level;
            let explicit = rolled(find(&pool, name), &mut rng);
            match explicit.affix_type {
                AffixType::Prefix => item.prefixes.push(explicit.clone()),
                AffixType::Suffix => item.suffixes.push(explicit.clone()),
            }

            // When asking for its tier and showing the advanced view
            let direct = item.tier(&explicit);
            let blocks = blocks_of(&item);

            // Then both give the tier of the PO's copy
            assert_eq!(direct, tier, "tier of {name} at item level {item_level}");
            let block = blocks.last().expect("a block for the mod");
            assert_eq!(block.name.as_deref(), Some(name));
            assert_eq!(
                block.tier,
                Some(tier),
                "block of {name} at item level {item_level}"
            );
        }
    }

    // --- Tags.
    // Given every mod of the game data and every implicit of every ring base
    for (what, modifier) in all_mods(&mut rng) {
        // When asking for its tags
        let tags = modifier.tag_text();

        // Then they follow the rule
        assert_eq!(
            tags,
            expected_tags(&modifier.tags),
            "{what} with the tags {:?}",
            modifier.tags
        );
    }

    // Given the PO's examples
    let implicit_of = |base_id: &str, rng: &mut SeededRng| {
        let base = base(base_id);
        assert_eq!(base.implicits.len(), 1, "{} has one implicit", base.name);
        Mod::roll(&base.implicits[0], rng)
    };
    let explicit_of =
        |name: &str, rng: &mut SeededRng| rolled(find(&iron_pool, name), rng).modifier;
    for (what, modifier, tags) in [
        (
            "implicit of the Iron Ring",
            implicit_of(IRON_RING_ID, &mut rng),
            "Damage, Physical, Attack",
        ),
        (
            "implicit of the Iolite Ring",
            implicit_of(IOLITE_RING_ID, &mut rng),
            "Damage, Chaos",
        ),
        (
            "implicit of the Unset Ring, without tags",
            implicit_of(UNSET_RING_ID, &mut rng),
            "",
        ),
        ("Magpie's", explicit_of("Magpie's", &mut rng), "Drop"),
        (
            "of Radiance",
            explicit_of("of Radiance", &mut rng),
            "Attack",
        ),
        (
            "of the Flatworm",
            explicit_of("of the Flatworm", &mut rng),
            "Life",
        ),
        // Invented: words split at underscores, hidden tags anywhere in the list
        (
            "an invented mod",
            invented_mod(
                "",
                &[
                    "resource",
                    "elemental_damage",
                    "test_new_tag",
                    "physical_damage",
                    "life",
                    "chaos_damage",
                    "flat_life_regen",
                    "x",
                ],
                &[],
                &[],
            ),
            "Test New Tag, Life, X",
        ),
        (
            "an invented mod with only hidden tags",
            invented_mod("", &HIDDEN_TAGS, &[], &[]),
            "",
        ),
    ] {
        // When asking for its tags
        let shown = modifier.tag_text();

        // Then they are those of the PO's copy
        assert_eq!(shown, tags, "{what}");
    }
}

// AC3: „Neben dem Item gibt es einen Umschalter mit den beiden Zuständen „Default Mod
// Descriptions“ und „Advanced Mod Descriptions“. Beide Texte sind Übersetzungsschlüssel, Englisch
// ist der Standard. Beim Laden der Seite ist „Default Mod Descriptions“ aktiv. Die Wahl wird
// nicht gespeichert. Die Alt-Taste hat keine Funktion."
// (Tech lead's plan, ADR 0014: only the structure is checked here: both texts are English texts
// of the translation file and are not written into other files under web/. The start state and
// the Alt key are checked in the local preview; that nothing is stored is protected by
// story_39::ac6.)
#[test]
fn ac3_the_toggle_texts_come_from_the_translations() {
    // Given the English translation file and the other files under web/
    let forbidden: Vec<String> = TOGGLE_TEXTS.iter().map(|text| (*text).to_owned()).collect();

    // When / Then the toggle texts are English texts of web/i18n/en.json and in no other file
    assert_texts_come_from_translations(&TOGGLE_TEXTS, &forbidden);
}

// AC5: „„Advanced Mod Descriptions“ zeigt die Mods nach Kriterium 1, jeder Mod als Block
// (Variante B):
// - links ein Kürzel: sichtbar „I“, „P“ bzw. „S“; als Name für Screenreader und als Tooltip beim
//   Überfahren mit der Maus „Implicit“, „Prefix“ bzw. „Suffix“. Alle sechs Texte sind
//   Übersetzungsschlüssel, Englisch ist der Standard;
// - daneben die Zeilen des Mods mit Wert und Bereich im Format des erweiterten Item-Texts, der
//   Bereich abgesetzt;
// - bei Prefix und Suffix rechts in der ersten Zeile das Tier in der goldenen Tier-Box (T1, T2 …);
// - darunter klein der Name des Mods (weiß, halbfett), dann „·“, dann die Tags mit Komma getrennt;
// - bei verstärkten Mods (Heist-Ringe) zeigt der Block den unverstärkten Wert und Bereich und in
//   der kleinen Zeile „— X% Increased“, wie in der bestehenden Regel."
// (Checked here: the lines of every mod and implicit in the extended format with each range as a
// part of its own, the PO's evidence of that format, the unscaled values and the increase of the
// Heist rings, that the six label texts are English texts of the translation file and not
// written into other files under web/, that no file outside the translations writes the mark
// "% Increased", and that the page shows the blocks of crafting-core. The layout is checked in
// the local preview.)
#[test]
fn ac5_blocks_show_the_extended_lines_the_increase_and_translated_labels() {
    let mut rng = SeededRng::new(SEED);

    // --- Lines in the format of the extended item text, the range as a part of its own.
    // Given every mod of the game data and every implicit of every ring base, with all values at
    // their minimum, at their maximum and rolled
    for (what, modifier) in all_mods(&mut rng) {
        let mut variants = Vec::new();
        for at_maximum in [false, true] {
            let mut variant = modifier.clone();
            for stat in &mut variant.stats {
                stat.value = if at_maximum { stat.max } else { stat.min };
            }
            variants.push(variant);
        }
        for _ in 0..ROLLS_PER_MOD {
            let mut variant = modifier.clone();
            for stat in &mut variant.stats {
                stat.value = stat.min + index(&mut rng, (stat.max - stat.min + 1) as usize) as i32;
            }
            variants.push(variant);
        }
        for variant in variants {
            // When asking for its lines
            let lines = lines_of(&variant.extended_lines());

            // Then each value is followed by its range, if it has one, as a part of its own
            assert_eq!(
                lines,
                expected_lines(&variant),
                "{what} with the values {:?}",
                variant.stats
            );
        }
    }

    // Given the PO's evidence: a Steel Ring whose implicit rolled 3 and 10, and an Iron Ring
    let mut steel_implicit = Mod::roll(&base(STEEL_RING_ID).implicits[0], &mut rng);
    set_value(&mut steel_implicit, MINIMUM_PHYSICAL_STAT, 3, "Steel Ring");
    set_value(&mut steel_implicit, MAXIMUM_PHYSICAL_STAT, 10, "Steel Ring");
    let iron_implicit = Mod::roll(&base(IRON_RING_ID).implicits[0], &mut rng);

    // When asking for their lines
    let steel_lines = lines_of(&steel_implicit.extended_lines());
    let iron_lines = lines_of(&iron_implicit.extended_lines());

    // Then each value with a range is followed by it, and fixed values have no brackets
    assert_eq!(steel_lines.len(), 1, "the Steel Ring implicit has one line");
    assert_eq!(
        line_text(&steel_lines[0]),
        text_with(
            &steel_implicit,
            &[
                (MINIMUM_PHYSICAL_STAT, "3(3-4)"),
                (MAXIMUM_PHYSICAL_STAT, "10(10-14)")
            ]
        )[0]
    );
    assert_eq!(ranges_of(&steel_lines[0]), ["(3-4)", "(10-14)"]);
    assert_eq!(iron_lines.len(), 1, "the Iron Ring implicit has one line");
    assert_eq!(line_text(&iron_lines[0]), iron_implicit.display_text());
    assert!(
        ranges_of(&iron_lines[0]).is_empty(),
        "fixed values have no range"
    );

    // --- Increased mods: unscaled values and the increase of their side.
    // Given rare Heist rings and an Iron Ring, filled with random mods
    let mut increases = BTreeSet::new();
    for base_id in INCREASE_BASES {
        let base = base(base_id);
        let pool = pool_of(base_id);
        for _ in 0..INCREASE_ITEMS {
            let mut item = item_of(&base, Rarity::Rare, &mut rng);
            let mut prefixes = Vec::new();
            let mut suffixes = Vec::new();
            loop {
                let valid = valid_pool(&item, &pool);
                if valid.is_empty() {
                    break;
                }
                let choice = &valid[index(&mut rng, valid.len())];
                let explicit = add_definition(&mut item, choice, &mut rng);
                match choice.affix_type {
                    AffixType::Prefix => prefixes.push(explicit),
                    AffixType::Suffix => suffixes.push(explicit),
                }
            }

            // When showing the advanced view
            let blocks = blocks_of(&item);

            // Then the lines show the unscaled values and each block the increase of its side
            assert_eq!(
                blocks,
                expected_blocks(&item, &pool, &prefixes, &suffixes),
                "{}",
                base.name
            );
            for block in &blocks {
                increases.insert((base_id, block.kind, block.magnitude_increase));
            }
        }
    }
    for expected in [
        (COGWORK_RING_ID, "suffix", Some(25)),
        (COGWORK_RING_ID, "prefix", None),
        (COGWORK_RING_ID, "implicit", None),
        (GEODESIC_RING_ID, "prefix", Some(25)),
        (GEODESIC_RING_ID, "suffix", None),
        (MANIFOLD_RING_ID, "prefix", Some(50)),
        (HELICAL_RING_ID, "suffix", Some(50)),
        (COMPOSITE_RING_ID, "prefix", None),
        (IRON_RING_ID, "prefix", None),
        (IRON_RING_ID, "suffix", None),
    ] {
        assert!(
            increases.contains(&expected),
            "the items must show {expected:?}, found {increases:?}"
        );
    }

    // --- Labels from the translations.
    // Given the English translation file and the other files under web/
    let mut forbidden: Vec<String> = LABEL_TEXTS
        .iter()
        .flat_map(|text| [format!("\"{text}\""), format!("'{text}'")])
        .collect();
    forbidden.push(INCREASE_MARK.to_owned());

    // When / Then the six labels are English texts of web/i18n/en.json and in no other file, and
    // no other file writes the mark of an increased mod
    assert_texts_come_from_translations(&LABEL_TEXTS, &forbidden);
    // And the page shows the blocks of crafting-core
    assert_web_src_uses(
        "advanced_mods",
        "the blocks and their content come from crafting-core",
    );
}

// AC6: „Beispiele, die in der Abnahme geprüft werden:
// - „Magpie's Iron Ring of Radiance“: Implicit „Adds 1 to 4 Physical Damage to Attacks“ mit den
//   Tags „Damage, Physical, Attack“, ohne Tier. Prefix Magpie's T4 „Drop“. Suffix of Radiance T1
//   „Attack“ mit beiden Zeilen (Global Accuracy, Light Radius) in einem Block.
// - „Magpie's Iron Ring of Raiding“: Default zeigt eine Rarity-Zeile mit der Summe, Advanced
//   zeigt zwei Blöcke (T4 und T3).
// - Cogwork Ring mit zwei Suffixen: Advanced unverstärkt mit „— 25% Increased“, Default
//   verstärkt."
// (The values are chosen by the test: Magpie's 10, of Radiance 18 accuracy, of Raiding 12, of the
// Flatworm 606 (shown 10.1). The Cogwork Ring holds of the Flatworm and of Raiding. The expected
// lines are built from the mods' texts in the data, so the game wording is not checked here. The
// page is checked by the PO in the local preview.)
#[test]
fn ac6_the_pos_examples_show_their_blocks() {
    let mut rng = SeededRng::new(SEED);
    let iron_ring = base(IRON_RING_ID);
    let iron_pool = pool_of(IRON_RING_ID);

    // --- Magpie's Iron Ring of Radiance.
    // Given a magic Iron Ring with Magpie's and of Radiance
    let mut item = item_of(&iron_ring, Rarity::Magic, &mut rng);
    add_with_value(&mut item, &iron_pool, "Magpie's", RARITY_STAT, 10, &mut rng);
    add_with_value(
        &mut item,
        &iron_pool,
        "of Radiance",
        ACCURACY_STAT,
        18,
        &mut rng,
    );

    // When showing the advanced view
    let blocks = blocks_of(&item);

    // Then the implicit shows its fixed values, its tags and no tier
    assert_eq!(blocks.len(), 3, "an implicit, a prefix and a suffix");
    let implicit = &blocks[0];
    assert_eq!(implicit.kind, "implicit");
    assert_eq!(implicit.name, None);
    assert_eq!(implicit.tier, None);
    assert_eq!(implicit.tags, "Damage, Physical, Attack");
    assert_eq!(implicit.lines.len(), 1);
    assert_eq!(
        line_text(&implicit.lines[0]),
        item.implicits[0].display_text()
    );
    assert!(ranges_of(&implicit.lines[0]).is_empty());
    // And Magpie's is a prefix of tier 4 with the tag "Drop"
    let magpies = &blocks[1];
    let magpies_mod = &item.prefixes[0].modifier;
    assert_eq!(magpies.kind, "prefix");
    assert_eq!(magpies.name.as_deref(), Some("Magpie's"));
    assert_eq!(magpies.tier, Some(4));
    assert_eq!(magpies.tags, "Drop");
    assert_eq!(magpies.lines.len(), 1);
    assert_eq!(
        line_text(&magpies.lines[0]),
        text_with(magpies_mod, &[(RARITY_STAT, "10(8-12)")])[0]
    );
    assert_eq!(ranges_of(&magpies.lines[0]), ["(8-12)"]);
    // And of Radiance is a suffix of tier 1 with the tag "Attack" and both lines in one block
    let radiance = &blocks[2];
    let radiance_mod = &item.suffixes[0].modifier;
    assert_eq!(radiance.kind, "suffix");
    assert_eq!(radiance.name.as_deref(), Some("of Radiance"));
    assert_eq!(radiance.tier, Some(1));
    assert_eq!(radiance.tags, "Attack");
    let radiance_lines = text_with(
        radiance_mod,
        &[(ACCURACY_STAT, "18(16-20)"), (LIGHT_RADIUS_STAT, "15")],
    );
    assert_eq!(radiance_lines.len(), 2, "of Radiance has two lines");
    let shown: Vec<String> = radiance.lines.iter().map(|line| line_text(line)).collect();
    assert_eq!(shown, radiance_lines);
    assert_eq!(ranges_of(&radiance.lines[0]), ["(16-20)"]);
    assert!(
        ranges_of(&radiance.lines[1]).is_empty(),
        "the light radius is fixed"
    );

    // --- Magpie's Iron Ring of Raiding.
    // Given a magic Iron Ring with Magpie's and of Raiding
    let mut item = item_of(&iron_ring, Rarity::Magic, &mut rng);
    add_with_value(&mut item, &iron_pool, "Magpie's", RARITY_STAT, 10, &mut rng);
    add_with_value(
        &mut item,
        &iron_pool,
        "of Raiding",
        RARITY_STAT,
        12,
        &mut rng,
    );

    // When showing the normal and the advanced view
    let default_lines = item.explicit_texts();
    let blocks = blocks_of(&item);

    // Then the normal view shows one rarity line with the sum
    assert_eq!(
        default_lines,
        [display_with(&item.prefixes[0].modifier, RARITY_STAT, 22)]
    );
    // And the advanced view shows two blocks of tier 4 and 3 after the implicit
    assert_eq!(blocks.len(), 3, "an implicit and two blocks");
    let explicit_blocks: Vec<OneLine> = blocks[1..].iter().map(one_line).collect();
    assert_eq!(
        explicit_blocks,
        [
            expected_one_line(
                "prefix",
                "Magpie's",
                4,
                text_with(&item.prefixes[0].modifier, &[(RARITY_STAT, "10(8-12)")])[0].clone(),
                "(8-12)",
                None,
            ),
            expected_one_line(
                "suffix",
                "of Raiding",
                3,
                text_with(&item.suffixes[0].modifier, &[(RARITY_STAT, "12(11-14)")])[0].clone(),
                "(11-14)",
                None,
            ),
        ]
    );

    // --- Cogwork Ring with two suffixes.
    // Given a magic Cogwork Ring with of the Flatworm and of Raiding
    let cogwork_pool = pool_of(COGWORK_RING_ID);
    let mut item = item_of(&base(COGWORK_RING_ID), Rarity::Magic, &mut rng);
    add_with_value(
        &mut item,
        &cogwork_pool,
        "of the Flatworm",
        LIFE_REGENERATION_STAT,
        606,
        &mut rng,
    );
    add_with_value(
        &mut item,
        &cogwork_pool,
        "of Raiding",
        RARITY_STAT,
        12,
        &mut rng,
    );
    let flatworm = item.suffixes[0].modifier.clone();
    let raiding = item.suffixes[1].modifier.clone();

    // When showing the normal and the advanced view
    let default_lines: BTreeSet<String> = item.explicit_texts().into_iter().collect();
    let blocks = blocks_of(&item);

    // Then the normal view shows the values increased by 25 % and rounded down
    let increased = BTreeSet::from([
        display_with(&flatworm, LIFE_REGENERATION_STAT, scaled(606, 25)),
        display_with(&raiding, RARITY_STAT, scaled(12, 25)),
    ]);
    let unscaled = BTreeSet::from([flatworm.display_text(), raiding.display_text()]);
    assert!(
        increased.is_disjoint(&unscaled),
        "the example changes both values"
    );
    assert_eq!(default_lines, increased);
    // And the advanced view shows both suffixes unscaled, with the increase of 25 %
    let suffix_blocks: Vec<OneLine> = blocks
        .iter()
        .filter(|block| block.kind == "suffix")
        .map(one_line)
        .collect();
    assert_eq!(
        suffix_blocks,
        [
            expected_one_line(
                "suffix",
                "of the Flatworm",
                5,
                text_with(&flatworm, &[(LIFE_REGENERATION_STAT, "10.1(8.1-16)")])[0].clone(),
                "(8.1-16)",
                Some(25),
            ),
            expected_one_line(
                "suffix",
                "of Raiding",
                3,
                text_with(&raiding, &[(RARITY_STAT, "12(11-14)")])[0].clone(),
                "(11-14)",
                Some(25),
            ),
        ]
    );
    assert!(
        blocks
            .iter()
            .all(|block| block.kind == "suffix" || block.magnitude_increase.is_none()),
        "the implicits show no increase"
    );
}

// AC9: „Regel des PO, Wortlaut für poe-crafting-rules (verbindlich). Im Abschnitt „Item basics“,
// im Absatz, der mit In an extended item text (Ctrl+Alt+C) beginnt, wird die Zeile - Not
// evidenced and not to be guessed: ranges of negated values (reduced) and of values with
// decimals. They do not occur among the ring implicits; all negated values there are fixed.
// ersetzt durch genau: - Not evidenced and not to be guessed: ranges of negated values (reduced).
// They do not occur among the ring implicits; all negated values there are fixed.
// Direkt danach, als neue Zeile desselben Absatzes:
// - Values with decimals follow the same format: the value and both bounds of the range are
// converted and rounded as described above, without trailing zeros. Evidence: the PO's
// recordings (poe_orb_test): "Regenerate 3.5(2-4) Life per second", "Regenerate 10.1(8.1-16)
// Life per second", "0.38(0.2-0.4)% of Physical Attack Damage Leeched as Life"."
// (The wording is checked at the acceptance. "Regenerate 3.5(2-4) ..." comes from an amulet,
// which the data does not have; an invented mod with the raw range 120-240 per minute and the
// conversion of life regeneration stands in for it. Invented mods also check the rounding of the
// bounds (exact halves) and a fixed converted value. Every value of every converted range in the
// data is checked.)
#[test]
fn ac9_values_with_decimals_show_the_converted_value_and_bounds() {
    let mut rng = SeededRng::new(SEED);
    let per_second = Conversion {
        divisor: 60,
        precision: Some(1),
    };
    let per_hundred = Conversion {
        divisor: 100,
        precision: None,
    };
    let mods = explicit_mods();
    let mut flatworm = Mod::roll(&find(&mods, "of the Flatworm").definition, &mut rng);
    set_value(
        &mut flatworm,
        LIFE_REGENERATION_STAT,
        606,
        "of the Flatworm",
    );
    let mut remora = Mod::roll(&find(&mods, "of the Remora").definition, &mut rng);
    set_value(&mut remora, LIFE_LEECH_STAT, 38, "of the Remora");

    // Given the PO's evidence and invented mods
    for (what, modifier, expected_line, expected_ranges) in [
        (
            "of the Flatworm with 606 per minute",
            flatworm.clone(),
            text_with(&flatworm, &[(LIFE_REGENERATION_STAT, "10.1(8.1-16)")])[0].clone(),
            vec!["(8.1-16)"],
        ),
        (
            "of the Remora with 38 per ten thousand",
            remora.clone(),
            text_with(&remora, &[(LIFE_LEECH_STAT, "0.38(0.2-0.4)")])[0].clone(),
            vec!["(0.2-0.4)"],
        ),
        (
            "invented life regeneration 210 (120-240) per minute",
            invented_mod(
                "Regenerate {test_regeneration} Life per second",
                &[],
                &[("test_regeneration", 120, 240, 210)],
                &[("test_regeneration", per_second.clone())],
            ),
            "Regenerate 3.5(2-4) Life per second".to_owned(),
            vec!["(2-4)"],
        ),
        (
            "invented exact halves 129 (123-135) per minute",
            invented_mod(
                "Regenerate {test_regeneration} Life per second",
                &[],
                &[("test_regeneration", 123, 135, 129)],
                &[("test_regeneration", per_second.clone())],
            ),
            "Regenerate 2.2(2-2.2) Life per second".to_owned(),
            vec!["(2-2.2)"],
        ),
        (
            "invented fixed leech 21",
            invented_mod(
                "{test_leech}% of Damage Leeched",
                &[],
                &[("test_leech", 21, 21, 21)],
                &[("test_leech", per_hundred.clone())],
            ),
            "0.21% of Damage Leeched".to_owned(),
            vec![],
        ),
    ] {
        // When asking for its lines
        let lines = lines_of(&modifier.extended_lines());

        // Then the value and both bounds are converted and rounded, without trailing zeros
        assert_eq!(lines.len(), 1, "{what}: one line");
        assert_eq!(line_text(&lines[0]), expected_line, "{what}");
        assert_eq!(ranges_of(&lines[0]), expected_ranges, "{what}");
    }

    // Given every mod and implicit of the data with a converted stat that has a range, at every
    // value of that range
    let mut checked = 0;
    for (what, modifier) in all_mods(&mut rng) {
        for stat in &modifier.stats {
            if stat.min == stat.max || !modifier.conversions.contains_key(&stat.id) {
                continue;
            }
            for value in stat.min..=stat.max {
                let mut variant = modifier.clone();
                set_value(&mut variant, &stat.id, value, &what);

                // When asking for its lines
                let lines = lines_of(&variant.extended_lines());

                // Then they show the converted value and range
                assert_eq!(lines, expected_lines(&variant), "{what} with {value}");
                checked += 1;
            }
        }
    }
    assert!(checked > 0, "the data has converted stats with ranges");
}
