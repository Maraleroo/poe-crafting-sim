//! Acceptance tests for AB#186 "Ring auswählen mit Suche, Slots und klickbarem Pfad".
//!
//! The tests use the public interface of `crafting-core` from the tech lead's plan on AB#186
//! and ADR 0018:
//! - `Slot { Weapon, Helmet, OffHand, BodyArmour, Amulet, Ring, Belt, Gloves, Boots }`, a
//!   copyable enum like `Currency` (ADR 0011), with `Slot::ALL: [Slot; 9]`,
//!   `Slot::id(self) -> &'static str`, `Slot::item_classes(self) -> Vec<String>` and
//!   `Slot::base_items(self) -> Vec<BaseItem>` (in the order of `data/base_items.json`).
//! - `ModDefinition::range_text(&self) -> String`.
//! - unchanged: `base_items()`, `base_item(id)`, `mod_pool(id)`, `valid_mods`, `Item::new`,
//!   `apply_orb_of_transmutation`, `apply_orb_of_alteration`.
//!
//! The assignment of item classes to slots is the data file `data/item_slots.json`
//! (ADR 0018). Range texts follow the PO's rule in `poe-crafting-rules`, "Item basics" (the
//! paragraph "In an extended item text (Ctrl+Alt+C) ..." and the PO's decision for AB#186):
//! without a rolled value, a range is shown as `(minimum-maximum)` with a normal hyphen, a fixed
//! value as the value alone, converted as the game shows it. Every random test uses a fixed
//! seed and depends only on invariants and on events whose absence is practically impossible,
//! never on the order in which the generator is used.
//!
//! Red state before the implementation: the module does not compile, because `Slot` and
//! `ModDefinition::range_text` do not exist yet.
//!
//! What is checked outside `cargo test`, by the PO in the local preview (tech lead's plan):
//! - AC1, AC2, AC4, AC8, AC9: the picker itself (opening, suggestions, path, keyboard, width
//!   of 375 pixels). The search, the suggestions and the path are covered by the developer's
//!   unit tests in `web` (ADR 0018).
//! - AC3: that the unavailable slots show "Not available yet" and cannot be chosen.
//! - AC5: that choosing a base closes the picker and shows the item on the page.
//! - AC7: the wording of the UI texts (also in the review against the criterion, ADR 0014).

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crafting_core::{
    apply_orb_of_alteration, apply_orb_of_transmutation, base_item, base_items, mod_pool,
    valid_mods, BaseItem, Conversion, ExplicitMod, ExplicitModDefinition, Item, ModDefinition,
    Rarity, SeededRng, Slot, StatDefinition,
};
use serde_json::Value;

// ---------------------------------------------------------------------------------------
// Values from the story, the PO's decisions, ADR 0018 and the game data
// ---------------------------------------------------------------------------------------

/// The slot identifiers of ADR 0018, in the order of the story (Weapon, Helmet, Off-hand,
/// Body Armour, Amulet, Ring, Belt, Gloves, Boots).
const SLOT_IDS: [&str; 9] = [
    "weapon",
    "helmet",
    "off_hand",
    "body_armour",
    "amulet",
    "ring",
    "belt",
    "gloves",
    "boots",
];
const RING_SLOT_ID: &str = "ring";

/// The item class of every ring base in the game data.
const RINGS_CLASS: &str = "Rings";
/// Number of ring bases imported in AB#185 (31) and AB#188 (the six Breach rings, which
/// replace criterion 2 of AB#185 for them, PO confirmation on AB#188).
const RING_BASES: usize = 37;

const IRON_RING_ID: &str = "Metadata/Items/Rings/Ring1";
const BONE_RING_ID: &str = "Metadata/Items/Rings/Ring16";

/// Bases with the same name and different implicits (AC6).
const TWO_STONE_RINGS: [&str; 3] = [
    "Metadata/Items/Rings/Ring12",
    "Metadata/Items/Rings/Ring13",
    "Metadata/Items/Rings/Ring14",
];
const SHADOWED_RINGS: [&str; 3] = [
    "Metadata/Items/Rings/RingK5a",
    "Metadata/Items/Rings/RingK5b",
    "Metadata/Items/Rings/RingK5c",
];

/// The PO's examples for a text without a rolled value (`poe-crafting-rules`, "Item basics",
/// PO decision for AB#186): base id and the expected range text of its implicit.
const RULE_EXAMPLES: [(&str, &str); 3] = [
    (
        "Metadata/Items/Rings/Ring12",
        "+(12-16)% to Fire and Lightning Resistances",
    ),
    (
        "Metadata/Items/Rings/RingAtlas1",
        "Adds (3-4) to (10-14) Physical Damage to Attacks",
    ),
    (IRON_RING_ID, "Adds 1 to 4 Physical Damage to Attacks"),
];

/// The en dash, which the PO's rule replaces by a normal hyphen.
const EN_DASH: char = '\u{2013}';

/// The item level the UI uses by default ("Item level" in `poe-crafting-rules`).
const UI_ITEM_LEVEL: u32 = 100;

/// Fixed seed, so every run is identical.
const SEED: u64 = 186;
/// Orbs of each kind per base for AC5. The smallest share of base-specific mods is the Unset
/// Ring's: 3.6 % of the prefix weight and 1.4 % of the combined weight, so an orb adds one with
/// a chance of about 2.5 %. With 1000 orbs about 25 are expected; seeing none has a chance of
/// about 10^-11. An implementation that rolls from the Iron Ring's pool sees none.
const ORBS: u64 = 1_000;

/// UI texts from the description that are sentences or phrases; none may be written into a
/// file under `web/` outside `web/i18n/` (AC7). The path separator is from the example
/// "All items › Ring".
const UI_PHRASES: [&str; 8] = [
    "Search, e.g. ruby or bone",
    "All items",
    "Not available yet",
    "Nothing found.",
    "Type two letters for suggestions, or click through the slots.",
    "Body Armour",
    "Off-hand",
    "\u{203a}",
];

/// UI texts from the description that are single words and therefore also Rust identifiers
/// (`Item`, `Slot::Ring`); they may not appear as a string literal of their own in `web/`
/// outside `web/i18n/` (AC7).
const UI_WORDS: [&str; 10] = [
    "Item", "Class", "Base", "Weapon", "Helmet", "Amulet", "Ring", "Belt", "Gloves", "Boots",
];

/// The base name of the MirrorRing is "Ring", which is also the slot name (a UI text, PO
/// decision on AB#186). It is left out of the check for base names in `web/` (AC7).
const NAME_SHARED_WITH_SLOT: &str = "Ring";

const BASE_ITEMS_JSON: &str = "data/base_items.json";
const ITEM_SLOTS_JSON: &str = "data/item_slots.json";

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

fn load_json(relative: &str) -> Value {
    let path = repo_path(relative);
    assert!(path.is_file(), "{relative} must exist");
    serde_json::from_str(&read_path(&path))
        .unwrap_or_else(|e| panic!("{relative} is not valid JSON: {e}"))
}

/// All files below `dir`, skipping the given direct subdirectories.
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

/// All files under `web/`, except the build output in `web/dist` and `web/target`.
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

fn assert_web_src_uses(needle: &str, why: &str) {
    let used = web_src_files()
        .iter()
        .any(|path| read_path(path).contains(needle));
    assert!(used, "a file under web/src/ must use `{needle}`: {why}");
}

// ---------------------------------------------------------------------------------------
// Helpers: slots
// ---------------------------------------------------------------------------------------

/// The slots of the story, in its order. The match is exhaustive, so a slot that the story
/// does not name would not compile.
fn story_position(slot: Slot) -> usize {
    match slot {
        Slot::Weapon => 0,
        Slot::Helmet => 1,
        Slot::OffHand => 2,
        Slot::BodyArmour => 3,
        Slot::Amulet => 4,
        Slot::Ring => 5,
        Slot::Belt => 6,
        Slot::Gloves => 7,
        Slot::Boots => 8,
    }
}

fn ids(bases: &[BaseItem]) -> Vec<String> {
    bases.iter().map(|base| base.id.clone()).collect()
}

// ---------------------------------------------------------------------------------------
// Helpers: the PO's display rules
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

/// The value the game shows for a raw stat value, by the PO's rules in "Item basics": without
/// a conversion the raw value; with a precision the raw value divided by the divisor as a
/// 32-bit floating-point number, rounded to the precision, exact halves to the even digit;
/// without a precision the exact quotient. Trailing zeros are dropped. A negative divisor
/// reverses the sign.
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

/// The text the PO's rule expects for a mod definition without a rolled value: a range
/// (minimum less than maximum) as `(minimum-maximum)`, a fixed value alone, converted as the
/// game shows it. The rule does not cover ranges of negated or converted values and of negative
/// values; the test stops if the data has one, instead of guessing.
fn expected_range_text(definition: &ModDefinition) -> String {
    render(&definition.text, &definition.placeholders, |stat_id| {
        let stat = definition
            .stats
            .iter()
            .find(|stat| stat.id == stat_id)
            .unwrap_or_else(|| panic!("the placeholder names the missing stat {stat_id:?}"));
        let conversion = definition.conversions.get(stat_id);
        if stat.min < stat.max {
            assert!(
                conversion.is_none() && stat.min >= 0,
                "{stat_id:?}: the PO's rule does not define this range, see poe-crafting-rules"
            );
            format!("({}-{})", stat.min, stat.max)
        } else {
            shown_value(stat.min, conversion)
        }
    })
}

/// The range texts of all implicits of a base, line by line.
fn implicit_lines(base: &BaseItem) -> Vec<String> {
    base.implicits
        .iter()
        .flat_map(|implicit| {
            implicit
                .range_text()
                .lines()
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .collect()
}

fn invented_stat(id: &str, min: i32, max: i32) -> StatDefinition {
    StatDefinition {
        id: id.to_owned(),
        min,
        max,
    }
}

// ---------------------------------------------------------------------------------------
// Helpers: crafting
// ---------------------------------------------------------------------------------------

/// Checks that a rolled prefix or suffix is a mod of `pool`, with its name, type and text.
fn assert_from_pool(
    explicit: &ExplicitMod,
    pool: &BTreeMap<String, ExplicitModDefinition>,
    what: &str,
) {
    let definition = pool.get(&explicit.name).unwrap_or_else(|| {
        panic!(
            "{what}: the mod {:?} is not in the base's pool",
            explicit.name
        )
    });
    assert_eq!(explicit.affix_type, definition.affix_type, "{what}");
    assert_eq!(
        explicit.modifier.family, definition.definition.family,
        "{what}"
    );
    assert_eq!(explicit.modifier.text, definition.definition.text, "{what}");
}

/// The rolled prefix and suffix of a magic item.
fn explicit_mods_of(item: &Item) -> Vec<&ExplicitMod> {
    item.prefix.iter().chain(item.suffix.iter()).collect()
}

// ---------------------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------------------

// AC3: „Der Slot "Ring" zeigt alle importierten Ring-Basen. Alle anderen Slots zeigen "Not
// available yet" und lassen sich nicht auswählen."
// (Tech lead's plan and ADR 0018: a slot is available when its `base_items()` is not empty.
// The rendering of the unavailable slots is checked in the local preview.)
#[test]
fn ac3_the_ring_slot_holds_all_ring_bases_and_every_other_slot_has_none() {
    // Given the bases of crafting-core
    let all_bases = base_items();
    let ring_bases: Vec<String> = all_bases
        .iter()
        .filter(|base| base.item_class == RINGS_CLASS)
        .map(|base| base.id.clone())
        .collect();
    assert_eq!(ring_bases.len(), RING_BASES, "imported ring bases");

    // When listing the slots
    let slots = Slot::ALL;

    // Then there are nine different slots in the order of the story, with the ids of ADR 0018
    let positions: Vec<usize> = slots.iter().map(|slot| story_position(*slot)).collect();
    assert_eq!(positions, (0..9).collect::<Vec<_>>(), "order of Slot::ALL");
    let slot_ids: Vec<&str> = slots.iter().map(|slot| slot.id()).collect();
    assert_eq!(slot_ids, SLOT_IDS, "ids of Slot::ALL");

    // And the Ring slot holds all ring bases, in the order of the data
    let ring_slot = Slot::Ring;
    assert_eq!(ring_slot.id(), RING_SLOT_ID);
    assert_eq!(
        ids(&ring_slot.base_items()),
        ring_bases,
        "Slot::Ring.base_items()"
    );
    // And they are exactly the bases of the slot's item classes
    let ring_classes = ring_slot.item_classes();
    assert_eq!(
        ring_classes,
        vec![RINGS_CLASS.to_owned()],
        "Slot::Ring.item_classes()"
    );
    let of_classes: Vec<String> = all_bases
        .iter()
        .filter(|base| ring_classes.contains(&base.item_class))
        .map(|base| base.id.clone())
        .collect();
    assert_eq!(ids(&ring_slot.base_items()), of_classes);

    // And every other slot has no item class and no base, so it is not available
    for slot in slots.iter().filter(|slot| slot.id() != RING_SLOT_ID) {
        assert!(
            slot.item_classes().is_empty(),
            "the slot {:?} must have no item classes, found {:?}",
            slot.id(),
            slot.item_classes()
        );
        assert!(
            slot.base_items().is_empty(),
            "the slot {:?} must have no bases, found {:?}",
            slot.id(),
            ids(&slot.base_items())
        );
    }

    // And every base belongs to exactly one slot
    let mut in_slots: Vec<String> = slots
        .iter()
        .flat_map(|slot| ids(&slot.base_items()))
        .collect();
    in_slots.sort();
    let mut expected = ids(&all_bases);
    expected.sort();
    assert_eq!(in_slots, expected, "the bases of all slots together");
}

// AC5: „Die Wahl einer Basis schließt die Auswahl und zeigt ein normales Item dieser Basis mit
// ihrem Implicit. Orb of Transmutation und Orb of Alteration würfeln aus dem Pool dieser
// Basis."
// (Tech lead's plan: choosing a base replaces the base, its pool `valid_mods(mod_pool(id), 100)`
// and the item, which becomes `Item::new` of the base, like Reset (ADR 0018). Closing the
// picker and the page are checked in the local preview; `Workbench::select` is covered by the
// developer's unit tests in `web`.)
#[test]
fn ac5_a_chosen_ring_is_a_normal_item_and_the_orbs_roll_from_its_own_pool() {
    let mut rng = SeededRng::new(SEED);
    // Given the Iron Ring's pool, which the page used for every item before this story
    let iron_pool: BTreeSet<String> = mod_pool(IRON_RING_ID)
        .unwrap_or_else(|| panic!("mod_pool({IRON_RING_ID:?}) must offer the Iron Ring's pool"))
        .into_iter()
        .map(|definition| definition.name)
        .collect();
    let mut bases_with_own_mods: BTreeSet<String> = BTreeSet::new();

    for base in Slot::Ring.base_items() {
        let what = format!("{} {}", base.name, base.id);
        // When the player chooses the base
        let item = Item::new(&base, &mut rng);

        // Then the item is a normal item of this base with its implicit
        assert_eq!(item.rarity, Rarity::Normal, "{what}");
        assert_eq!(item.base, base, "{what}");
        assert!(item.prefix.is_none() && item.suffix.is_none(), "{what}");
        assert_eq!(item.implicits.len(), base.implicits.len(), "{what}");
        for (rolled, definition) in item.implicits.iter().zip(&base.implicits) {
            assert_eq!(rolled.family, definition.family, "{what}");
            assert_eq!(rolled.text, definition.text, "{what}");
            assert_eq!(rolled.placeholders, definition.placeholders, "{what}");
            assert_eq!(rolled.conversions, definition.conversions, "{what}");
            assert_eq!(rolled.stats.len(), definition.stats.len(), "{what}");
            for stat in &definition.stats {
                let value = rolled
                    .stats
                    .iter()
                    .find(|rolled_stat| rolled_stat.id == stat.id)
                    .unwrap_or_else(|| panic!("{what}: the implicit must have {:?}", stat.id))
                    .value;
                assert!(
                    (stat.min..=stat.max).contains(&value),
                    "{what}: {} rolled {value}, outside {}..={}",
                    stat.id,
                    stat.min,
                    stat.max
                );
            }
        }

        // And the pool is the base's own pool at the UI's item level
        let own_pool = valid_mods(
            &mod_pool(&base.id)
                .unwrap_or_else(|| panic!("{what}: mod_pool() must offer the base's pool")),
            UI_ITEM_LEVEL,
        );
        assert!(!own_pool.is_empty(), "{what}: the pool must not be empty");
        let pool: BTreeMap<String, ExplicitModDefinition> = own_pool
            .iter()
            .map(|definition| (definition.name.clone(), definition.clone()))
            .collect();
        let has_own_mods = pool.keys().any(|name| !iron_pool.contains(name));
        if has_own_mods {
            bases_with_own_mods.insert(base.id.clone());
        }

        // When Orbs of Transmutation and Alteration are applied to the item
        let mut own_by_transmutation = 0_u64;
        let mut own_by_alteration = 0_u64;
        for _ in 0..ORBS {
            let mut crafted = Item::new(&base, &mut rng);
            apply_orb_of_transmutation(&mut crafted, &own_pool, &mut rng)
                .unwrap_or_else(|e| panic!("{what}: transmutation failed with {e:?}"));
            // Then every mod comes from the base's own pool
            assert_eq!(crafted.rarity, Rarity::Magic, "{what}");
            assert_eq!(crafted.base, base, "{what}");
            for explicit in explicit_mods_of(&crafted) {
                assert_from_pool(explicit, &pool, &format!("{what}, transmutation"));
                if !iron_pool.contains(&explicit.name) {
                    own_by_transmutation += 1;
                }
            }
            apply_orb_of_alteration(&mut crafted, &own_pool, &mut rng)
                .unwrap_or_else(|e| panic!("{what}: alteration failed with {e:?}"));
            assert_eq!(crafted.rarity, Rarity::Magic, "{what}");
            for explicit in explicit_mods_of(&crafted) {
                assert_from_pool(explicit, &pool, &format!("{what}, alteration"));
                if !iron_pool.contains(&explicit.name) {
                    own_by_alteration += 1;
                }
            }
        }

        // And a base with mods of its own also rolls them, unlike the Iron Ring
        if has_own_mods {
            assert!(
                own_by_transmutation > 0,
                "{what}: {ORBS} Orbs of Transmutation rolled none of the base's own mods"
            );
            assert!(
                own_by_alteration > 0,
                "{what}: {ORBS} Orbs of Alteration rolled none of the base's own mods"
            );
        }
    }

    // And the Bone Ring is among the bases with mods of their own (its minion mods)
    assert!(
        bases_with_own_mods.contains(BONE_RING_ID),
        "the Bone Ring's pool must have mods that the Iron Ring's pool does not have, found \
         {bases_with_own_mods:?}"
    );
    // And the page builds the item and the pool of the chosen base with crafting-core
    assert_web_src_uses("Item::new", "the chosen base becomes a new normal item");
    assert_web_src_uses("mod_pool", "the orbs roll from the chosen base's own pool");
    assert_web_src_uses(
        "valid_mods",
        "the pool of the chosen base follows the item level rule",
    );
}

// AC6: „Two-Stone Rings und Shadowed Rings sind in Vorschlägen und Liste durch ihren
// Implicit-Text unterscheidbar."
// (PO decisions on AB#186: every base shows its implicit text with value ranges, bases without
// an implicit show none; a range as `(minimum-maximum)` with a normal hyphen, a fixed value
// alone (`poe-crafting-rules`, "Item basics"). Tech lead's plan: `range_text()` of the
// implicits; the expected text is derived from the data and the PO's rule.)
#[test]
fn ac6_bases_with_the_same_name_differ_by_their_implicit_range_text() {
    // --- The rule with an invented mod ---
    // Given a mod with two ranges, listed in reverse order, and a fixed converted value on a
    // second line
    let invented = ModDefinition {
        family: "Invented".to_owned(),
        level: 1,
        tags: Vec::new(),
        stats: vec![
            invented_stat("invented_maximum", 10, 14),
            invented_stat("invented_per_minute", 135, 135),
            invented_stat("invented_minimum", 3, 4),
        ],
        text: "Adds {low} to {high} Invented Damage\n{regen} Invented per second".to_owned(),
        placeholders: BTreeMap::from([
            ("low".to_owned(), "invented_minimum".to_owned()),
            ("high".to_owned(), "invented_maximum".to_owned()),
            ("regen".to_owned(), "invented_per_minute".to_owned()),
        ]),
        conversions: BTreeMap::from([(
            "invented_per_minute".to_owned(),
            Conversion {
                divisor: 60,
                precision: Some(1),
            },
        )]),
    };
    // When its range text is built
    let text = invented.range_text();
    // Then each range is shown by its stat identifier, the fixed value is converted, and the
    // line break stays
    assert_eq!(
        text, "Adds (3-4) to (10-14) Invented Damage\n2.2 Invented per second",
        "range text of an invented mod"
    );
    assert_eq!(text, expected_range_text(&invented));

    // --- The PO's examples ---
    for (id, expected) in RULE_EXAMPLES {
        let base = base_item(id).unwrap_or_else(|| panic!("crafting-core must offer {id}"));
        assert_eq!(
            implicit_lines(&base),
            vec![expected.to_owned()],
            "the PO's example for {} {id}",
            base.name
        );
    }

    // --- All ring bases ---
    // Given all ring bases
    let bases = Slot::Ring.base_items();
    assert_eq!(bases.len(), RING_BASES, "Slot::Ring.base_items()");
    let mut shown: BTreeMap<(String, Vec<String>), String> = BTreeMap::new();
    for base in &bases {
        let what = format!("{} {}", base.name, base.id);
        // When their implicit texts are built
        for implicit in &base.implicits {
            let text = implicit.range_text();
            // Then each follows the PO's rule, with a normal hyphen
            assert_eq!(text, expected_range_text(implicit), "{what}");
            assert!(
                !text.contains(EN_DASH),
                "{what}: a range uses a normal hyphen, found {text:?}"
            );
            assert!(!text.contains('{'), "{what}: placeholder left in {text:?}");
        }
        // And a base without an implicit shows no text
        let lines = implicit_lines(base);
        assert_eq!(lines.is_empty(), base.implicits.is_empty(), "{what}");
        // And no two bases show the same name with the same implicit text
        if let Some(other) = shown.insert((base.name.clone(), lines), base.id.clone()) {
            panic!("{what} cannot be told apart from {other}");
        }
    }

    // And the three Two-Stone Rings and the three Shadowed Rings differ pairwise
    for group in [TWO_STONE_RINGS, SHADOWED_RINGS] {
        let texts: Vec<Vec<String>> = group
            .iter()
            .map(|id| {
                let base = base_item(id).unwrap_or_else(|| panic!("crafting-core must offer {id}"));
                let lines = implicit_lines(&base);
                assert!(
                    !lines.is_empty(),
                    "{} {id} must show its implicit",
                    base.name
                );
                lines
            })
            .collect();
        let names: BTreeSet<String> = group
            .iter()
            .map(|id| base_item(id).expect("checked above").name)
            .collect();
        assert_eq!(names.len(), 1, "the bases {group:?} share one name");
        let distinct: BTreeSet<&Vec<String>> = texts.iter().collect();
        assert_eq!(
            distinct.len(),
            group.len(),
            "the implicit texts of {names:?} must differ pairwise, found {texts:#?}"
        );
    }
}

// AC7: „Die Texte aus der Description erscheinen wörtlich; Namen von Klassen und Basen kommen
// aus den Spieldaten."
// (Tech lead's plan, ADR 0014: only the structure is checked here: the UI texts are not written
// into files under web/ outside the translation files, class and base names are in no file
// under web/, and the assignment of classes to slots is data (ADR 0018). The file contents are
// meant, not the rendered page. The base name "Ring" (MirrorRing) is also the slot name and is
// left out. The wording is checked in the review and in the local preview.)
#[test]
fn ac7_ui_texts_come_from_translations_and_names_from_the_game_data() {
    // Given the files under web/ and the bases of crafting-core
    let i18n = repo_path("web").join("i18n");
    let web = web_files();
    let bases = base_items();
    // Longest first, so that no id is cut by a shorter one it starts with
    let mut base_ids: Vec<String> = ids(&bases);
    base_ids.sort_by_key(|id| std::cmp::Reverse(id.len()));

    // When searching the files outside web/i18n/ for the UI texts of the description
    let mut hard_coded: Vec<String> = Vec::new();
    for path in web.iter().filter(|path| !path.starts_with(&i18n)) {
        let content = read_path(path);
        for phrase in UI_PHRASES {
            if content.contains(phrase) {
                hard_coded.push(format!("{} contains {phrase:?}", path.display()));
            }
        }
        for word in UI_WORDS {
            let literal = format!("\"{word}\"");
            if content.contains(&literal) {
                hard_coded.push(format!("{} contains {literal}", path.display()));
            }
        }
    }
    // Then none is written there
    assert!(
        hard_coded.is_empty(),
        "UI texts must come from the translation files in web/i18n/: {hard_coded:#?}"
    );

    // When searching all files under web/ for class and base names, without the base ids
    let mut names: BTreeSet<String> = bases
        .iter()
        .map(|base| base.name.clone())
        .filter(|name| name != NAME_SHARED_WITH_SLOT)
        .collect();
    names.extend(bases.iter().map(|base| base.item_class.clone()));
    let mut game_texts: Vec<String> = Vec::new();
    for path in &web {
        let content = base_ids.iter().fold(read_path(path), |content, id| {
            content.replace(id.as_str(), "")
        });
        for name in &names {
            if content.contains(name.as_str()) {
                game_texts.push(format!("{} contains {name:?}", path.display()));
            }
        }
    }
    // Then none is written there
    assert!(
        game_texts.is_empty(),
        "class and base names must come from crafting-core, not from web/: {game_texts:#?}"
    );
    // And the page takes the slots, their bases, the class names and the implicit texts from
    // crafting-core
    assert_web_src_uses("Slot::", "the slots come from crafting-core");
    assert_web_src_uses("base_items", "the bases of a slot come from crafting-core");
    assert_web_src_uses("item_class", "the class names come from the game data");
    assert_web_src_uses("range_text", "the implicit texts come from crafting-core");

    // When reading the assignment of item classes to slots
    let file = load_json(ITEM_SLOTS_JSON);
    let entries = file
        .get("slots")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("{ITEM_SLOTS_JSON} must have an array \"slots\""));
    let mut from_file: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for entry in entries {
        let id = entry
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("{ITEM_SLOTS_JSON}: every slot must have a string \"id\""));
        let classes: Vec<String> = entry
            .get("item_classes")
            .and_then(Value::as_array)
            .unwrap_or_else(|| panic!("{ITEM_SLOTS_JSON}: {id:?} must have \"item_classes\""))
            .iter()
            .map(|class| {
                class
                    .as_str()
                    .unwrap_or_else(|| panic!("{ITEM_SLOTS_JSON}: {id:?} must list strings"))
                    .to_owned()
            })
            .collect();
        assert!(
            from_file.insert(id.to_owned(), classes).is_none(),
            "{ITEM_SLOTS_JSON}: the slot {id:?} is listed twice"
        );
    }
    // Then it is data: every slot has exactly the item classes of its entry, none without one
    for slot in Slot::ALL {
        let expected = from_file.remove(slot.id()).unwrap_or_default();
        assert_eq!(
            slot.item_classes(),
            expected,
            "Slot::item_classes() of {:?} as in {ITEM_SLOTS_JSON}",
            slot.id()
        );
    }
    assert!(
        from_file.is_empty(),
        "{ITEM_SLOTS_JSON} lists unknown slots: {:?}",
        from_file.keys().collect::<Vec<_>>()
    );
    // And the item classes of the slots are the classes of the game data
    let data_classes: BTreeSet<String> = load_json(BASE_ITEMS_JSON)
        .get("base_items")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("{BASE_ITEMS_JSON} must have an array \"base_items\""))
        .iter()
        .map(|base| {
            base.get("item_class")
                .and_then(Value::as_str)
                .unwrap_or_else(|| panic!("{BASE_ITEMS_JSON}: every base needs an item class"))
                .to_owned()
        })
        .collect();
    let slot_classes: BTreeSet<String> = Slot::ALL
        .iter()
        .flat_map(|slot| slot.item_classes())
        .collect();
    assert_eq!(
        slot_classes, data_classes,
        "the item classes of the slots and of {BASE_ITEMS_JSON}"
    );
}
