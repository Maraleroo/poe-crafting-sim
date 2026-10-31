//! Acceptance tests for AB#189 "Heist-Ringe importieren".
//!
//! The tests use the public interface of `crafting-core` from the tech lead's plan on AB#189 and
//! ADR 0022:
//! - `Item::affix_limits(&self) -> Option<AffixLimits>`: `AffixLimits::of(rarity)` with each
//!   side's limit changed by the base's lines "±N Prefix/Suffix Modifier(s) allowed", never
//!   below 0; `mods` unchanged; `None` for a unique item.
//! - `Item::explicit_text(&self, explicit: &ExplicitMod) -> String`: the text of a prefix or
//!   suffix in the normal view, with the base's increase for the mod's side; with 0 % it equals
//!   `explicit.modifier.display_text()`.
//! - `Item::name()`: the names of all prefixes, the base name and the names of all suffixes,
//!   each list in its order.
//! - unchanged: `AffixLimits::of`, `valid_pool`, `add_random_mod`, both orbs, `mod_pool`,
//!   `base_item`, `Slot::Ring.base_items()`, `ModDefinition::range_text()`,
//!   `Mod::display_text()` (unscaled) and `Item::required_level()`.
//!
//! The PO's rules are `poe-crafting-rules`: "Affix limits", the paragraph on "X% increased ...
//! Modifier magnitudes" and the magic item name in "Item basics", "Adding a random mod" and the
//! currency files of both orbs. The ids, names, implicit stats and implicit lines of the six
//! rings were taken from the PO's RePoE export 3.29.3.3 (the raw files of `data/README.md`); the
//! raw files are not in the repository. All six have the Iron Ring's pool there.
//!
//! Criteria 4 and 5 give the PO's wording for `poe-crafting-rules`. That wording is checked byte
//! for byte at the acceptance (step 2 of the `story` skill), not here; the tests check the
//! behaviour of the rules.
//!
//! Every random test uses a fixed seed. The statistical check compares observed shares with the
//! shares of the rule, with a tolerance of 5 standard deviations. The tests depend only on
//! frequencies and invariants, never on the order in which the generator is used.
//!
//! Red state before the implementation: the module does not compile, because
//! `Item::affix_limits` and `Item::explicit_text` do not exist yet.
//!
//! What is checked outside `cargo test`, by the PO in the local preview (tech lead's plan):
//! - AC1: choosing the six rings in the picker, in the slot "Ring" and in the suggestions.
//! - AC2: the implicits and "Requires Level 24" as the page shows them.
//! - AC4: the page showing the scaled values (here: the page uses `explicit_text`).
//! - AC6: the order of the mods on the page (also a unit test in `web`).
//!
//! Not testable: the part "a fractured mod first" of criterion 5, because the simulator has no
//! fractured mods yet (tech lead's plan).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crafting_core::{
    add_random_mod, apply_orb_of_alteration, apply_orb_of_transmutation, base_item, base_items,
    mod_pool, valid_mods, valid_pool, AffixLimits, AffixType, BaseItem, ExplicitMod,
    ExplicitModDefinition, Item, Mod, Rarity, SeededRng, Slot,
};

// ---------------------------------------------------------------------------------------
// Values from the story, the PO's rules and the RePoE export 3.29.3.3
// ---------------------------------------------------------------------------------------

/// The item class of every ring base in the game data.
const RINGS_CLASS: &str = "Rings";
const IRON_RING_ID: &str = "Metadata/Items/Rings/Ring1";

const COGWORK_RING_ID: &str = "Metadata/Items/Rings/RingE1";
const GEODESIC_RING_ID: &str = "Metadata/Items/Rings/RingE2";
const COMPOSITE_RING_ID: &str = "Metadata/Items/Rings/RingE3";
const MANIFOLD_RING_ID: &str = "Metadata/Items/Rings/RingE4";
const HELICAL_RING_ID: &str = "Metadata/Items/Rings/RingE6";

/// The stats of the Heist ring implicits whose lines name them, from RePoE.
const PREFIXES_ALLOWED_STAT: &str = "local_maximum_prefixes_allowed_+";
const SUFFIXES_ALLOWED_STAT: &str = "local_maximum_suffixes_allowed_+";
const PREFIX_MAGNITUDE_STAT: &str = "local_prefix_mod_effect_+%";
const SUFFIX_MAGNITUDE_STAT: &str = "local_suffix_mod_effect_+%";

/// The level of every Heist ring implicit in RePoE.
const IMPLICIT_LEVEL: u32 = 30;
/// The level requirement a normal Heist ring shows (criterion 2): 80 % of 30.
const NORMAL_HEIST_RING_LEVEL: u32 = 24;

/// The item level the UI uses by default ("Item level" in `poe-crafting-rules`).
const UI_ITEM_LEVEL: u32 = 100;

/// A Heist ring of the story, with the values of its implicit in RePoE.
struct HeistRing {
    id: &'static str,
    name: &'static str,
    /// N of the line "±N Prefix Modifier(s) allowed".
    prefix_change: i32,
    /// N of the line "±N Suffix Modifier(s) allowed".
    suffix_change: i32,
    /// X of the line "X% increased Prefix Modifier magnitudes"; 0 % has no line.
    prefix_increase: u32,
    /// X of the line "X% increased Suffix Modifier magnitudes"; 0 % has no line.
    suffix_increase: u32,
    /// The only side a magic ring has room for (tech lead's plan, from the rule).
    magic_side: AffixType,
    /// The implicit's lines in the normal view of the game, in the game's order.
    lines: &'static [&'static str],
}

/// The six rings of the story (RePoE ids `RingE1` to `RingE6`).
const HEIST_RINGS: [HeistRing; 6] = [
    HeistRing {
        id: COGWORK_RING_ID,
        name: "Cogwork Ring",
        prefix_change: -1,
        suffix_change: 1,
        prefix_increase: 0,
        suffix_increase: 25,
        magic_side: AffixType::Suffix,
        lines: &[
            "-1 Prefix Modifier allowed",
            "+1 Suffix Modifier allowed",
            "Implicit Modifiers Cannot Be Changed",
            "25% increased Suffix Modifier magnitudes",
        ],
    },
    HeistRing {
        id: GEODESIC_RING_ID,
        name: "Geodesic Ring",
        prefix_change: 1,
        suffix_change: -1,
        prefix_increase: 25,
        suffix_increase: 0,
        magic_side: AffixType::Prefix,
        lines: &[
            "+1 Prefix Modifier allowed",
            "-1 Suffix Modifier allowed",
            "Implicit Modifiers Cannot Be Changed",
            "25% increased Prefix Modifier magnitudes",
        ],
    },
    HeistRing {
        id: COMPOSITE_RING_ID,
        name: "Composite Ring",
        prefix_change: 3,
        suffix_change: -3,
        prefix_increase: 0,
        suffix_increase: 0,
        magic_side: AffixType::Prefix,
        lines: &[
            "+3 Prefix Modifiers allowed",
            "-3 Suffix Modifiers allowed",
            "Implicit Modifiers Cannot Be Changed",
        ],
    },
    HeistRing {
        id: MANIFOLD_RING_ID,
        name: "Manifold Ring",
        prefix_change: 1,
        suffix_change: -2,
        prefix_increase: 50,
        suffix_increase: 0,
        magic_side: AffixType::Prefix,
        lines: &[
            "+1 Prefix Modifier allowed",
            "-2 Suffix Modifiers allowed",
            "Implicit Modifiers Cannot Be Changed",
            "50% increased Prefix Modifier magnitudes",
        ],
    },
    HeistRing {
        id: "Metadata/Items/Rings/RingE5",
        name: "Ratcheting Ring",
        prefix_change: -3,
        suffix_change: 3,
        prefix_increase: 0,
        suffix_increase: 0,
        magic_side: AffixType::Suffix,
        lines: &[
            "-3 Prefix Modifiers allowed",
            "+3 Suffix Modifiers allowed",
            "Implicit Modifiers Cannot Be Changed",
        ],
    },
    HeistRing {
        id: HELICAL_RING_ID,
        name: "Helical Ring",
        prefix_change: -2,
        suffix_change: 1,
        prefix_increase: 0,
        suffix_increase: 50,
        magic_side: AffixType::Suffix,
        lines: &[
            "-2 Prefix Modifiers allowed",
            "+1 Suffix Modifier allowed",
            "Implicit Modifiers Cannot Be Changed",
            "50% increased Suffix Modifier magnitudes",
        ],
    },
];

/// A mod of the Iron Ring's pool with fixed raw values on a ring, and its text in the normal
/// view by the PO's rule (criterion 4).
struct MagnitudeExample {
    ring: &'static str,
    mod_name: &'static str,
    /// Raw value of each stat, by stat identifier.
    values: &'static [(&'static str, i32)],
    /// The text of the normal view: scaled, rounded down, then converted.
    shown: &'static str,
    /// The text with the unscaled values, which `Mod::display_text` keeps.
    unscaled: &'static str,
}

const RECOUPED_STAT: &str = "damage_taken_goes_to_life_over_4_seconds_%";
const LIFE_LEECH_STAT: &str = "life_leech_from_physical_attack_damage_permyriad";
const MANA_LEECH_STAT: &str = "mana_leech_from_physical_attack_damage_permyriad";
const LIFE_REGENERATION_STAT: &str = "base_life_regeneration_rate_per_minute";
const COLD_RESISTANCE_STAT: &str = "base_cold_damage_resistance_%";
const LIFE_STAT: &str = "base_maximum_life";
const EVASION_STAT: &str = "base_evasion_rating";

/// The examples of criterion 4. Most values are chosen so that rounding to the nearest value,
/// with exact halves up or to the even digit, gives another result than rounding down, and so
/// that converting before scaling gives another result than scaling the raw value.
const MAGNITUDE_EXAMPLES: [MagnitudeExample; 13] = [
    // The example of the criterion: 11 × 1.25 = 13.75.
    MagnitudeExample {
        ring: COGWORK_RING_ID,
        mod_name: "of Suturing",
        values: &[(RECOUPED_STAT, 11)],
        shown: "13% of Damage taken Recouped as Life",
        unscaled: "11% of Damage taken Recouped as Life",
    },
    // The PO's evidence 47 → 58 (58.75).
    MagnitudeExample {
        ring: COGWORK_RING_ID,
        mod_name: "of Haast",
        values: &[(COLD_RESISTANCE_STAT, 47)],
        shown: "+58% to Cold Resistance",
        unscaled: "+47% to Cold Resistance",
    },
    // The PO's evidence 0.30 → 0.37: raw 30 × 1.25 = 37.5, rounded down 37, then / 100.
    MagnitudeExample {
        ring: COGWORK_RING_ID,
        mod_name: "of the Remora",
        values: &[(LIFE_LEECH_STAT, 30)],
        shown: "0.37% of Physical Attack Damage Leeched as Life",
        unscaled: "0.3% of Physical Attack Damage Leeched as Life",
    },
    // The PO's evidence 0.31 → 0.38: raw 31 × 1.25 = 38.75, rounded down 38, then / 100.
    MagnitudeExample {
        ring: COGWORK_RING_ID,
        mod_name: "of Thirst",
        values: &[(MANA_LEECH_STAT, 31)],
        shown: "0.38% of Physical Attack Damage Leeched as Mana",
        unscaled: "0.31% of Physical Attack Damage Leeched as Mana",
    },
    // Raw 1788 per minute (29.8 per second) × 1.25 = 2235, / 60 = 37.25, shown with one decimal
    // as 37.2 (exact half to the even digit). This fits the PO's reading 29.8 → 37.2.
    MagnitudeExample {
        ring: COGWORK_RING_ID,
        mod_name: "of the Hydra",
        values: &[(LIFE_REGENERATION_STAT, 1788)],
        shown: "Regenerate 37.2 Life per second",
        unscaled: "Regenerate 29.8 Life per second",
    },
    // A prefix on the Cogwork Ring, which increases only suffixes, stays unscaled.
    MagnitudeExample {
        ring: COGWORK_RING_ID,
        mod_name: "Stout",
        values: &[(LIFE_STAT, 57)],
        shown: "+57 to maximum Life",
        unscaled: "+57 to maximum Life",
    },
    // 62 × 1.25 = 77.5.
    MagnitudeExample {
        ring: GEODESIC_RING_ID,
        mod_name: "Fleet",
        values: &[(EVASION_STAT, 62)],
        shown: "+77 to Evasion Rating",
        unscaled: "+62 to Evasion Rating",
    },
    // A suffix on the Geodesic Ring, which increases only prefixes, stays unscaled.
    MagnitudeExample {
        ring: GEODESIC_RING_ID,
        mod_name: "of Suturing",
        values: &[(RECOUPED_STAT, 11)],
        shown: "11% of Damage taken Recouped as Life",
        unscaled: "11% of Damage taken Recouped as Life",
    },
    // 50 %: 57 × 1.5 = 85.5.
    MagnitudeExample {
        ring: MANIFOLD_RING_ID,
        mod_name: "Stout",
        values: &[(LIFE_STAT, 57)],
        shown: "+85 to maximum Life",
        unscaled: "+57 to maximum Life",
    },
    // 50 % on each stat of a mod with two stats: 5 × 1.5 = 7.5, 11 × 1.5 = 16.5.
    MagnitudeExample {
        ring: MANIFOLD_RING_ID,
        mod_name: "Icy",
        values: &[
            ("attack_minimum_added_cold_damage", 5),
            ("attack_maximum_added_cold_damage", 11),
        ],
        shown: "Adds 7 to 16 Cold Damage to Attacks",
        unscaled: "Adds 5 to 11 Cold Damage to Attacks",
    },
    // 50 %: 13 × 1.5 = 19.5.
    MagnitudeExample {
        ring: HELICAL_RING_ID,
        mod_name: "of Fleshbinding",
        values: &[(RECOUPED_STAT, 13)],
        shown: "19% of Damage taken Recouped as Life",
        unscaled: "13% of Damage taken Recouped as Life",
    },
    // 50 %: raw 31 × 1.5 = 46.5, rounded down 46, then / 100.
    MagnitudeExample {
        ring: HELICAL_RING_ID,
        mod_name: "of Thirst",
        values: &[(MANA_LEECH_STAT, 31)],
        shown: "0.46% of Physical Attack Damage Leeched as Mana",
        unscaled: "0.31% of Physical Attack Damage Leeched as Mana",
    },
    // A ring without a line "... Modifier magnitudes" shows the unscaled value.
    MagnitudeExample {
        ring: COMPOSITE_RING_ID,
        mod_name: "Stout",
        values: &[(LIFE_STAT, 57)],
        shown: "+57 to maximum Life",
        unscaled: "+57 to maximum Life",
    },
];

/// The PO's evidence for criterion 5, as (ring, prefixes in the order they were added,
/// suffixes in the order they were added, name).
type NameExample = (
    &'static str,
    &'static [&'static str],
    &'static [&'static str],
    &'static str,
);
const NAME_EXAMPLES: [NameExample; 6] = [
    (
        COMPOSITE_RING_ID,
        &["Chilled", "Fleet"],
        &[],
        "Chilled Fleet Composite Ring",
    ),
    (
        COMPOSITE_RING_ID,
        &["Sapphire", "Polished"],
        &[],
        "Sapphire Polished Composite Ring",
    ),
    (
        COMPOSITE_RING_ID,
        &["Burning", "Stout"],
        &[],
        "Burning Stout Composite Ring",
    ),
    // The same prefixes added the other way round.
    (
        COMPOSITE_RING_ID,
        &["Fleet", "Chilled"],
        &[],
        "Fleet Chilled Composite Ring",
    ),
    (
        COGWORK_RING_ID,
        &[],
        &["of the Philosopher", "of Haast"],
        "Cogwork Ring of the Philosopher of Haast",
    ),
    (
        COGWORK_RING_ID,
        &[],
        &["of Haast", "of the Philosopher"],
        "Cogwork Ring of Haast of the Philosopher",
    ),
];

/// Fixed seeds, so every run is identical.
const SEED: u64 = 189;
const ORB_SEED: u64 = 18_901;

/// Orbs of each kind per Heist ring (AC3). Each ring gives 400 results; 5 standard deviations
/// of the share of 2 mods (0.5) are then 0.125 per ring and 0.051 over all six rings, so a limit
/// that leaves room for only one mod (share 0) fails clearly.
const ORBS: u64 = 200;
/// Items per ring and rarity filled with mods until no mod is added.
const FILLED_ITEMS: u64 = 50;
/// Rare items per ring base whose texts are compared with the rule (AC4).
const TEXT_ITEMS: u64 = 20;
/// Magic items per ring whose names are checked (AC5).
const NAME_ITEMS: u64 = 200;

// ---------------------------------------------------------------------------------------
// Helpers: files
// ---------------------------------------------------------------------------------------

fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crafting-core must live inside the repository root")
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
    collect(&repo_root().join("web").join("src"), &mut files);
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

fn by_name(pool: &[ExplicitModDefinition]) -> BTreeMap<String, ExplicitModDefinition> {
    pool.iter()
        .map(|definition| (definition.name.clone(), definition.clone()))
        .collect()
}

fn names(pool: &[ExplicitModDefinition]) -> Vec<String> {
    pool.iter()
        .map(|definition| definition.name.clone())
        .collect()
}

/// A new item of `base` with the given rarity and without prefixes and suffixes.
fn item_of(base: &BaseItem, rarity: Rarity, rng: &mut SeededRng) -> Item {
    let mut item = Item::new(base, rng);
    item.rarity = rarity;
    item
}

/// The mod `name` of `pool`, rolled and then given the raw values `values`, which must lie in
/// the ranges of its stats.
fn explicit_with(
    pool: &[ExplicitModDefinition],
    name: &str,
    values: &[(&str, i32)],
    rng: &mut SeededRng,
) -> ExplicitMod {
    let definition = find(pool, name);
    let mut modifier = Mod::roll(&definition.definition, rng);
    assert_eq!(
        modifier.stats.len(),
        values.len(),
        "{name:?}: the example gives a value for each stat"
    );
    for (id, value) in values {
        let stat = modifier
            .stats
            .iter_mut()
            .find(|stat| stat.id == *id)
            .unwrap_or_else(|| panic!("{name:?} must have the stat {id:?}"));
        assert!(
            (stat.min..=stat.max).contains(value),
            "{name:?}: {value} lies outside {}..={} of {id:?}",
            stat.min,
            stat.max
        );
        stat.value = *value;
    }
    ExplicitMod {
        name: definition.name.clone(),
        affix_type: definition.affix_type,
        modifier,
    }
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

/// The lines of an implicit text.
fn lines_of(text: &str) -> Vec<String> {
    text.split('\n').map(str::to_owned).collect()
}

/// The stats whose placeholders each line of `text` holds, line by line, in their order.
fn line_stats(text: &str, placeholders: &BTreeMap<String, String>) -> Vec<Vec<String>> {
    text.split('\n')
        .map(|line| {
            let mut stats = Vec::new();
            let mut rest = line;
            while let Some(start) = rest.find('{') {
                let end = start
                    + rest[start..]
                        .find('}')
                        .unwrap_or_else(|| panic!("unclosed placeholder in {text:?}"));
                let name = &rest[start + 1..end];
                let stat = placeholders.get(name).unwrap_or_else(|| {
                    panic!("the placeholder {name:?} of {text:?} names no stat")
                });
                stats.push(stat.clone());
                rest = &rest[end + 1..];
            }
            stats
        })
        .collect()
}

// ---------------------------------------------------------------------------------------
// Helpers: the PO's rules
// ---------------------------------------------------------------------------------------

/// A side's limit changed by N, never below 0 ("Affix limits").
fn changed(limit: usize, change: i32) -> usize {
    let limit = i64::try_from(limit).expect("a small limit") + i64::from(change);
    usize::try_from(limit.max(0)).expect("a limit of at least 0")
}

/// The affix limits of an item of the ring and rarity by the rule "Affix limits": the limits of
/// the rarity with each side changed by its line, never below 0; the total limit unchanged.
fn expected_limits(ring: &HeistRing, rarity: Rarity) -> Option<AffixLimits> {
    AffixLimits::of(rarity).map(|limits| AffixLimits {
        mods: limits.mods,
        prefixes: changed(limits.prefixes, ring.prefix_change),
        suffixes: changed(limits.suffixes, ring.suffix_change),
    })
}

/// The increase of the ring for a side, 0 for every other base.
fn increase_of(base_id: &str, affix_type: AffixType) -> u32 {
    HEIST_RINGS
        .iter()
        .find(|ring| ring.id == base_id)
        .map_or(0, |ring| match affix_type {
            AffixType::Prefix => ring.prefix_increase,
            AffixType::Suffix => ring.suffix_increase,
        })
}

/// The text of the normal view by the PO's rule: each stat value multiplied by (1 + X/100) and
/// rounded down to a whole number, then converted for display as usual. The usual display is
/// `Mod::display_text`, which the tests of AB#185 protect.
fn expected_explicit_text(explicit: &ExplicitMod, increase: u32) -> String {
    let mut scaled = explicit.modifier.clone();
    for stat in &mut scaled.stats {
        assert!(
            increase == 0 || stat.value >= 0,
            "{:?}: rounding down a scaled negative value is not evidenced (ADR 0022); the \
             pools of the Heist rings have no value below 0",
            explicit.name
        );
        let value = i64::from(stat.value) * i64::from(100 + increase) / 100;
        stat.value = i32::try_from(value).expect("a scaled value fits");
    }
    scaled.display_text()
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

/// Asserts that `after` is `before` with exactly one more mod at the end of its side.
fn assert_appended(before: &Item, after: &Item, ctx: &str) {
    let (side_before, side_after, other_before, other_after) =
        if after.prefixes.len() > before.prefixes.len() {
            (
                &before.prefixes,
                &after.prefixes,
                &before.suffixes,
                &after.suffixes,
            )
        } else {
            (
                &before.suffixes,
                &after.suffixes,
                &before.prefixes,
                &after.prefixes,
            )
        };
    assert_eq!(
        side_after.len(),
        side_before.len() + 1,
        "{ctx}: one mod is added, before {:?}, after {:?}",
        outcome(before),
        outcome(after)
    );
    assert_eq!(
        &side_after[..side_before.len()],
        &side_before[..],
        "{ctx}: the mods added before stay in their order, the new mod comes last"
    );
    assert_eq!(other_after, other_before, "{ctx}: the other side stays");
}

// ---------------------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------------------

// AC1: „Cogwork, Composite, Geodesic, Ratcheting, Helical und Manifold Ring sind im Slot „Ring“
// und in den Vorschlägen wählbar. (Ersetzt für diese sechs Ringe Kriterium 2 von AB#185.)"
// (Tech lead's plan: the six ids with their names in `Slot::Ring.base_items()` and from
// `base_item(id)`, each with the Iron Ring's pool, as in RePoE. The picker and its suggestions
// take their bases from there (ADR 0018) and are checked in the local preview. The tests of
// AB#185, AB#186 and AB#248 that counted 37 ring bases and excluded these six were adapted in the
// same commit, as the PO confirmed on AB#189.)
#[test]
fn ac1_the_six_heist_rings_are_ring_bases_that_can_be_chosen() {
    let mut rng = SeededRng::new(SEED);
    // Given the bases of the slot "Ring", all bases of crafting-core and the Iron Ring's pool
    let in_slot = Slot::Ring.base_items();
    let all_ids: Vec<String> = base_items().into_iter().map(|base| base.id).collect();
    let iron_pool = by_name(&pool_of(IRON_RING_ID));

    for expected in &HEIST_RINGS {
        let what = format!("{} {}", expected.name, expected.id);
        // When looking for the ring in the slot
        let found: Vec<&BaseItem> = in_slot
            .iter()
            .filter(|base| base.id == expected.id)
            .collect();

        // Then the slot offers it once, with its name from the game data and the ring class
        assert_eq!(
            found.len(),
            1,
            "{what}: Slot::Ring.base_items() must offer it once"
        );
        let ring = found[0];
        assert_eq!(ring.name, expected.name, "{what}: name");
        assert_eq!(ring.item_class, RINGS_CLASS, "{what}: item class");
        assert_eq!(
            ring.required_level, 0,
            "{what}: a ring has no own requirement"
        );
        assert!(
            all_ids.contains(&ring.id),
            "{what}: base_items() must offer it"
        );
        // And crafting-core offers the same base by its id
        assert_eq!(
            base_item(expected.id).as_ref(),
            Some(ring),
            "{what}: base_item()"
        );
        // And its pool is the Iron Ring's pool, with the same mods and weights
        assert_eq!(
            by_name(&pool_of(expected.id)),
            iron_pool,
            "{what}: mod_pool() must be the Iron Ring's pool"
        );
        // And choosing it gives a normal item with its implicit
        let item = Item::new(ring, &mut rng);
        assert_eq!(item.rarity, Rarity::Normal, "{what}");
        assert_eq!(&item.base, ring, "{what}");
        assert_eq!(item.implicits.len(), 1, "{what}: one implicit");
        assert!(
            item.prefixes.is_empty() && item.suffixes.is_empty(),
            "{what}"
        );
    }
}

// AC2: „Ihre Implicits erscheinen wie in der normalen Ansicht des Spiels, in dieser Reihenfolge:
// Prefix-Zeile, Suffix-Zeile, `Implicit Modifiers Cannot Be Changed`, Verstärkung. Bei ±1 in der
// Einzahl (`-1 Prefix Modifier allowed`), sonst in der Mehrzahl (`+3 Prefix Modifiers allowed`).
// Eine Verstärkung von 0 % erscheint nicht; der Composite Ring zeigt also drei Zeilen. Ein
// normaler Heist-Ring zeigt „Requires Level 24“."
// (Tech lead's plan: `range_text()` (the list of bases) and `display_text()` (the item) of the
// implicits, line by line as the game's normal view shows them, which is the text of the
// implicit in RePoE; the order also by the stats each line shows. "Appears not" is the rendered
// output; all values are fixed, so the list and the item show the same lines. The level
// requirement is `required_level()`. The page itself is checked in the local preview.)
#[test]
fn ac2_the_implicits_appear_as_in_the_normal_view_of_the_game() {
    let mut rng = SeededRng::new(SEED);

    for expected in &HEIST_RINGS {
        let what = format!("{} {}", expected.name, expected.id);
        // Given the ring's implicit
        let ring = base(expected.id);
        assert_eq!(ring.implicits.len(), 1, "{what}: one implicit");
        let implicit = &ring.implicits[0];
        assert_eq!(implicit.level, IMPLICIT_LEVEL, "{what}: implicit level");

        // When it is shown in the list of bases and on a normal item
        let listed = lines_of(&implicit.range_text());
        let item = Item::new(&ring, &mut rng);
        let shown = lines_of(&item.implicits[0].display_text());

        // Then both show the lines of the game's normal view: the prefix line, the suffix line,
        // "Implicit Modifiers Cannot Be Changed" and the increase, which a ring with 0 % does
        // not show; singular at ±1, plural otherwise
        assert_eq!(listed, expected.lines, "{what}: in the list of bases");
        assert_eq!(shown, expected.lines, "{what}: on a normal item");

        // And the lines are in this order by the stats they show
        let mut order = vec![
            vec![PREFIXES_ALLOWED_STAT.to_owned()],
            vec![SUFFIXES_ALLOWED_STAT.to_owned()],
            Vec::new(),
        ];
        if expected.prefix_increase > 0 {
            order.push(vec![PREFIX_MAGNITUDE_STAT.to_owned()]);
        }
        if expected.suffix_increase > 0 {
            order.push(vec![SUFFIX_MAGNITUDE_STAT.to_owned()]);
        }
        assert_eq!(
            line_stats(&implicit.text, &implicit.placeholders),
            order,
            "{what}: the stats shown on each line"
        );

        // And a normal ring shows the level requirement 24
        assert_eq!(
            item.required_level(),
            Some(NORMAL_HEIST_RING_LEVEL),
            "{what}: level requirement of a normal ring"
        );
    }

    // And the Composite Ring, whose increases are 0 %, shows three lines
    let composite = Item::new(&base(COMPOSITE_RING_ID), &mut rng);
    assert_eq!(
        composite.implicits[0].display_text().lines().count(),
        3,
        "the Composite Ring shows three lines"
    );
}

// AC3: „Die Zeilen „±N Prefix/Suffix Modifier(s) allowed“ ändern die Affix-Grenzen nach der
// Regel aus AB#248. Beispiel: Ein magischer Composite Ring hat nach Transmutation oder
// Alteration nur Prefixe, 1 oder 2; ein magischer Cogwork Ring nur Suffixe, 1 oder 2."
// (Tech lead's plan: `Item::affix_limits()` of the six rings for every rarity, from the rule
// "Affix limits"; rings without such lines keep `AffixLimits::of`. `valid_pool`, the orbs and
// `add_random_mod` use the changed limits: as a magic item, each Heist ring has room on one side
// only, and both orbs give 1 or 2 mods of that side, each about half the time.)
#[test]
fn ac3_the_lines_change_the_affix_limits_by_the_rule_of_ab_248() {
    let mut rng = SeededRng::new(SEED);

    // --- The limits ---
    for expected in &HEIST_RINGS {
        let ring = base(expected.id);
        for rarity in [Rarity::Normal, Rarity::Magic, Rarity::Rare, Rarity::Unique] {
            // Given an item of the ring and the rarity
            let item = item_of(&ring, rarity, &mut rng);
            // When asking for its affix limits
            // Then each side's limit is changed by its line, never below 0, and the total limit
            // is the rarity's
            assert_eq!(
                item.affix_limits(),
                expected_limits(expected, rarity),
                "{} as a {rarity:?} item",
                expected.name
            );
        }
        // And a magic ring has room on the side of the story's example only
        let magic = expected_limits(expected, Rarity::Magic).expect("magic items have limits");
        let (with_room, without_room) = match expected.magic_side {
            AffixType::Prefix => (magic.prefixes, magic.suffixes),
            AffixType::Suffix => (magic.suffixes, magic.prefixes),
        };
        assert!(
            with_room >= magic.mods && without_room == 0,
            "{}: the test expects room for 2 mods on one side only, found {magic:?}",
            expected.name
        );
    }
    // And every other ring keeps the limits of its rarity
    for ring in Slot::Ring.base_items() {
        if HEIST_RINGS.iter().any(|heist| heist.id == ring.id) {
            continue;
        }
        for rarity in [Rarity::Normal, Rarity::Magic, Rarity::Rare, Rarity::Unique] {
            let item = item_of(&ring, rarity, &mut rng);
            assert_eq!(
                item.affix_limits(),
                AffixLimits::of(rarity),
                "{} {} as a {rarity:?} item",
                ring.name,
                ring.id
            );
        }
    }

    // --- The valid pool of a new item ---
    for expected in &HEIST_RINGS {
        let ring = base(expected.id);
        let whole = pool_of(expected.id);
        let rollable = valid_mods(&whole, UI_ITEM_LEVEL);
        for rarity in [Rarity::Normal, Rarity::Magic, Rarity::Rare] {
            // Given a new item of the ring without prefixes and suffixes
            let item = item_of(&ring, rarity, &mut rng);
            let limits = expected_limits(expected, rarity).expect("limits");
            // When building its valid pool
            // Then it holds the mods of the ring's pool of each side with room, and only them
            let expected_pool: Vec<String> = rollable
                .iter()
                .filter(|definition| {
                    limits.mods > 0
                        && match definition.affix_type {
                            AffixType::Prefix => limits.prefixes > 0,
                            AffixType::Suffix => limits.suffixes > 0,
                        }
                })
                .map(|definition| definition.name.clone())
                .collect();
            assert_eq!(
                names(&valid_pool(&item, &whole)),
                expected_pool,
                "{} as a {rarity:?} item",
                expected.name
            );
        }
    }

    // --- Both limits hold at all times ---
    for expected in &HEIST_RINGS {
        let ring = base(expected.id);
        let whole = pool_of(expected.id);
        for rarity in [Rarity::Magic, Rarity::Rare] {
            let limits = expected_limits(expected, rarity).expect("limits");
            let filled = (
                limits.prefixes.min(limits.mods),
                limits.suffixes.min(limits.mods),
            );
            assert!(
                filled.0 + filled.1 <= limits.mods,
                "{}: the test expects the sides to fit the total limit",
                expected.name
            );
            for trial in 0..FILLED_ITEMS {
                let ctx = format!("{} as a {rarity:?} item, {trial}", expected.name);
                // Given an item of the ring without prefixes and suffixes
                let mut item = item_of(&ring, rarity, &mut rng);
                // When random mods are added until no mod is added
                while add_random_mod(&mut item, &whole, &mut rng) {
                    // Then after every added mod, both limits hold
                    assert!(
                        item.prefixes.len() <= limits.prefixes
                            && item.suffixes.len() <= limits.suffixes
                            && item.prefixes.len() + item.suffixes.len() <= limits.mods,
                        "{ctx}: {:?} breaks {limits:?}",
                        outcome(&item)
                    );
                }
                // And the item holds as many prefixes and suffixes as the limits allow
                assert_eq!(
                    (item.prefixes.len(), item.suffixes.len()),
                    filled,
                    "{ctx}: found {:?}",
                    outcome(&item)
                );
            }
        }
    }

    // --- The story's example with both orbs, on all six rings ---
    let mut orb_rng = SeededRng::new(ORB_SEED);
    let (mut all_results, mut all_two_mods) = (0_u64, 0_u64);
    for expected in &HEIST_RINGS {
        let ring = base(expected.id);
        let whole = pool_of(expected.id);
        let mut two_mods = 0_u64;
        let mut altered = Item::new(&ring, &mut orb_rng);
        apply_orb_of_transmutation(&mut altered, &whole, &mut orb_rng).expect("the orb applies");
        for trial in 0..ORBS {
            // When the Orb of Transmutation is applied to a new ring, and the Orb of Alteration
            // again and again to the same ring
            let mut transmuted = Item::new(&ring, &mut orb_rng);
            apply_orb_of_transmutation(&mut transmuted, &whole, &mut orb_rng)
                .unwrap_or_else(|e| panic!("{}: transmutation failed with {e:?}", expected.name));
            apply_orb_of_alteration(&mut altered, &whole, &mut orb_rng)
                .unwrap_or_else(|e| panic!("{}: alteration failed with {e:?}", expected.name));
            for (orb, item) in [
                ("Orb of Transmutation", &transmuted),
                ("Orb of Alteration", &altered),
            ] {
                let ctx = format!("{}, {orb}, {trial}", expected.name);
                // Then the ring is magic with 1 or 2 mods, all of the side with room
                assert_eq!(item.rarity, Rarity::Magic, "{ctx}");
                let (own, other) = match expected.magic_side {
                    AffixType::Prefix => (&item.prefixes, &item.suffixes),
                    AffixType::Suffix => (&item.suffixes, &item.prefixes),
                };
                assert!(
                    other.is_empty() && (1..=2).contains(&own.len()),
                    "{ctx}: 1 or 2 mods of the side {:?} only, found {:?}",
                    expected.magic_side,
                    outcome(item)
                );
                if own.len() == 2 {
                    two_mods += 1;
                }
            }
        }
        // And 1 and 2 mods each occur about half the time
        assert_share(
            &format!("{}: results with 2 mods", expected.name),
            two_mods,
            2 * ORBS,
            0.5,
        );
        all_results += 2 * ORBS;
        all_two_mods += two_mods;
    }
    assert_share(
        "all Heist rings: results with 2 mods",
        all_two_mods,
        all_results,
        0.5,
    );
}

// AC4: „Regel des PO, Wortlaut für poe-crafting-rules (verbindlich): Where a mod line "X%
// increased Prefix Modifier magnitudes", "X% increased Suffix Modifier magnitudes" or "X%
// increased Explicit Modifier magnitudes" applies, each affected stat value of the game data is
// multiplied by (1 + X/100) and rounded down to a whole number; the result is then converted for
// display as usual. The normal view shows this value. The extended text (Alt view, Ctrl+Alt+C)
// shows the unscaled value and range and marks the mod with "— X% Increased". Evidence: Cogwork
// Ring ("25% increased Suffix Modifier magnitudes"), values read by the PO on 2026-10-31 in the
// normal and the Alt view. Whole numbers: 11 values, including 6 → 7 (7.5), 10 → 12 (12.5), 22 →
// 27 (27.5), 47 → 58 (58.75). Leech (raw value / 100): 0.30 → 0.37, 0.31 → 0.38, 0.34 → 0.42,
// 0.35 → 0.43, 0.38 → 0.47, 0.39 → 0.48. Two life regeneration values (29.8 → 37.2, 25.2 →
// 31.5) fit, but their raw values are not unique. Measured only at +25 %; +50 % and +100 % are
// assumed to follow the same rule.
// Der Simulator zeigt im Item den verstärkten, abgerundeten Wert. Beispiel: unverstärkt 11 →
// `13% of Damage taken Recouped as Life`."
// (The wording is checked at the acceptance. Tech lead's plan: `Item::explicit_text` with fixed
// values on the rings with 25 % and 50 %, for the side with the increase and the other side, and
// on a ring without an increase; `Mod::display_text` keeps the unscaled value for a later
// extended view, which the simulator does not have yet. Then every mod of rare items of all ring
// bases against the rule. No base has the line "Explicit Modifier magnitudes" (ADR 0022). The
// page shows prefixes and suffixes through `explicit_text`, a lean text check (ADR 0004).)
#[test]
fn ac4_the_item_shows_values_scaled_by_the_magnitude_lines_and_rounded_down() {
    let mut rng = SeededRng::new(SEED);

    // --- The examples ---
    for example in &MAGNITUDE_EXAMPLES {
        let ring = base(example.ring);
        let what = format!("{:?} on the {}", example.mod_name, ring.name);
        // Given a rare ring that holds the mod with the example's raw values
        let pool = pool_of(example.ring);
        let explicit = explicit_with(&pool, example.mod_name, example.values, &mut rng);
        let mut item = item_of(&ring, Rarity::Rare, &mut rng);
        match explicit.affix_type {
            AffixType::Prefix => item.prefixes.push(explicit.clone()),
            AffixType::Suffix => item.suffixes.push(explicit.clone()),
        }
        // When the item shows the mod
        // Then it shows the scaled value, rounded down and then converted
        assert_eq!(item.explicit_text(&explicit), example.shown, "{what}");
        // And the mod itself keeps its unscaled values and text
        assert_eq!(
            explicit.modifier.display_text(),
            example.unscaled,
            "{what}: Mod::display_text"
        );
        for (id, value) in example.values {
            let kept = explicit
                .modifier
                .stats
                .iter()
                .find(|stat| stat.id == *id)
                .map(|stat| stat.value);
            assert_eq!(kept, Some(*value), "{what}: the raw value of {id:?} stays");
        }
        // And the rule gives the same text
        assert_eq!(
            expected_explicit_text(&explicit, increase_of(example.ring, explicit.affix_type)),
            example.shown,
            "{what}: the test's rule"
        );
    }

    // --- Every mod of rare items of all ring bases ---
    for ring in Slot::Ring.base_items() {
        let pool = pool_of(&ring.id);
        for trial in 0..TEXT_ITEMS {
            // Given a rare item of the ring filled with random mods
            let mut item = item_of(&ring, Rarity::Rare, &mut rng);
            while add_random_mod(&mut item, &pool, &mut rng) {}
            for explicit in explicits(&item) {
                // When the item shows the mod
                // Then the text follows the rule with the ring's increase for the mod's side:
                // the unscaled text on every ring without an increase
                let increase = increase_of(&ring.id, explicit.affix_type);
                assert_eq!(
                    item.explicit_text(explicit),
                    expected_explicit_text(explicit, increase),
                    "{} {}, item {trial}: {:?} with {increase} %",
                    ring.name,
                    ring.id,
                    explicit.name
                );
                if increase == 0 {
                    assert_eq!(
                        item.explicit_text(explicit),
                        explicit.modifier.display_text(),
                        "{} {}: {:?} without an increase",
                        ring.name,
                        ring.id,
                        explicit.name
                    );
                }
            }
        }
    }

    // --- The page ---
    // Then the page shows prefixes and suffixes with the text of crafting-core
    assert_web_src_uses(
        "explicit_text",
        "the item on the page shows prefixes and suffixes with their scaled values",
    );
}

// AC5: „Regel des PO, Wortlaut für poe-crafting-rules (verbindlich), ersetzt in „Item basics“
// den Satz, der mit „A magic item's name is“ beginnt: A magic item's name is the names of its
// prefixes, the base name and the names of its suffixes, each side in the order its mods were
// added (a fractured mod first), separated by spaces; a side without mods leaves its part out.
// Evidence: "Chilled Fleet Composite Ring", "Sapphire Polished Composite Ring" (Sapphire before an
// Orb of Augmentation, Polished added by it), "Burning Stout Composite Ring", "Cogwork Ring of the
// Philosopher of Haast" (fractured suffix first)."
// (The wording is checked at the acceptance. Tech lead's plan: `Item::name()` of magic items
// with two prefixes or two suffixes, built with `add_random_mod` in the order of the PO's
// evidence and the other way round; then the names of magic Heist rings after both orbs. The
// simulator has neither fractured mods nor an Orb of Augmentation; "a fractured mod first" is not
// testable yet, and `add_random_mod` stands in for the orb, as both use the rule of AB#248.)
#[test]
fn ac5_a_magic_name_has_all_prefixes_and_suffixes_in_the_order_they_were_added() {
    let mut rng = SeededRng::new(SEED);

    // --- The PO's evidence ---
    for (ring_id, prefixes, suffixes, name) in NAME_EXAMPLES {
        let ring = base(ring_id);
        let pool = pool_of(ring_id);
        // Given a magic ring without mods
        let mut item = item_of(&ring, Rarity::Magic, &mut rng);
        assert_eq!(item.name(), ring.name, "{name}: a magic ring without mods");
        // When the mods are added one after the other
        for mod_name in prefixes.iter().chain(suffixes.iter()) {
            let only = std::slice::from_ref(find(&pool, mod_name));
            assert!(
                add_random_mod(&mut item, only, &mut rng),
                "{name}: {mod_name:?} can be added"
            );
        }
        // Then the name has all prefixes, the base name and all suffixes, each in the order the
        // mods were added
        assert_eq!(item.name(), name, "{:?}", outcome(&item));
    }

    // --- Both sides on a magic Iron Ring ---
    let iron = base(IRON_RING_ID);
    let iron_pool = pool_of(IRON_RING_ID);
    let mut item = item_of(&iron, Rarity::Magic, &mut rng);
    item.suffixes.push(explicit_with(
        &iron_pool,
        "of the Philosopher",
        &[("additional_intelligence", 30)],
        &mut rng,
    ));
    item.prefixes.push(explicit_with(
        &iron_pool,
        "Stout",
        &[(LIFE_STAT, 57)],
        &mut rng,
    ));
    assert_eq!(item.name(), "Stout Iron Ring of the Philosopher");

    // --- After both orbs ---
    for expected in &HEIST_RINGS {
        let ring = base(expected.id);
        let pool = pool_of(expected.id);
        let mut altered = Item::new(&ring, &mut rng);
        apply_orb_of_transmutation(&mut altered, &pool, &mut rng).expect("the orb applies");
        for trial in 0..NAME_ITEMS {
            // Given magic rings made by both orbs
            let mut transmuted = Item::new(&ring, &mut rng);
            apply_orb_of_transmutation(&mut transmuted, &pool, &mut rng).expect("the orb applies");
            apply_orb_of_alteration(&mut altered, &pool, &mut rng).expect("the orb applies");
            for item in [&transmuted, &altered] {
                // When asking for the name
                // Then it has every prefix and every suffix in the order of its list
                let expected_name: Vec<&str> = item
                    .prefixes
                    .iter()
                    .map(|explicit| explicit.name.as_str())
                    .chain(std::iter::once(ring.name.as_str()))
                    .chain(item.suffixes.iter().map(|explicit| explicit.name.as_str()))
                    .collect();
                assert_eq!(
                    item.name(),
                    expected_name.join(" "),
                    "{} {trial}: {:?}",
                    expected.name,
                    outcome(item)
                );
            }
        }
    }
}

// AC6: „Bis zu einer eigenen Story zur Sortierung zeigt der Simulator die Mods eines Items als
// Prefixe, dann Suffixe, jeweils in der Reihenfolge des Hinzufügens."
// (Tech lead's plan: `add_random_mod` puts each mod at the end of its side, so `prefixes` and
// `suffixes` are in the order the mods were added, also with several mods on one side. The view
// itself, prefixes before suffixes, is checked by a unit test in `web` and in the local preview.)
#[test]
fn ac6_each_added_mod_comes_last_on_its_side() {
    let mut rng = SeededRng::new(SEED);
    for expected in &HEIST_RINGS {
        let ring = base(expected.id);
        let pool = pool_of(expected.id);
        for rarity in [Rarity::Magic, Rarity::Rare] {
            for trial in 0..FILLED_ITEMS {
                let ctx = format!("{} as a {rarity:?} item, {trial}", expected.name);
                // Given an item of the ring without prefixes and suffixes
                let mut item = item_of(&ring, rarity, &mut rng);
                let mut added = 0;
                loop {
                    let before = item.clone();
                    // When a random mod is added
                    if !add_random_mod(&mut item, &pool, &mut rng) {
                        break;
                    }
                    added += 1;
                    // Then it comes last on its side, after the mods added before
                    assert_appended(&before, &item, &ctx);
                }
                // And the item holds at least two mods, so their order was checked
                assert!(
                    added >= 2,
                    "{ctx}: at least two mods, found {:?}",
                    outcome(&item)
                );
            }
        }
    }
}
