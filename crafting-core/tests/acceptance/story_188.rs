//! Acceptance tests for AB#188 "Breach-Ringe importieren".
//!
//! The tests use the public interface of `crafting-core` from the tech lead's plan on AB#188 and
//! ADR 0020:
//! - `unblocked_mods(base_id: &str, mods: &[ExplicitModDefinition]) -> Vec<ExplicitModDefinition>`:
//!   the mods of `mods` that the damage type lock of the base does not block, in their order. A
//!   base without a lock, or an identifier that is not in the data, blocks nothing.
//! - unchanged: `base_item(id)`, `mod_pool(id)`, `valid_mods`, `Slot::Ring.base_items()`,
//!   `Item::new`, `Mod::display_text()`, `ModDefinition::range_text()`,
//!   `apply_orb_of_transmutation` and `apply_orb_of_alteration`, which do not filter the pool
//!   themselves (ADR 0013).
//!
//! The lock is the PO's rule in `poe-crafting-rules`, "Breach rings". The ids, the names and the
//! damage type that each ring's line "Cannot roll Modifiers of Non-<damage type> Damage Types"
//! names were taken from the PO's RePoE export 3.29.3.3; the raw files are not in the
//! repository. Every random test uses a fixed seed and depends only on invariants and on events
//! whose absence is practically impossible, never on the order in which the generator is used.
//!
//! Red state before the implementation: the module does not compile, because `unblocked_mods`
//! does not exist yet.
//!
//! What is checked outside `cargo test`, by the PO in the local preview (tech lead's plan):
//! - AC1: choosing the six rings in the picker, in the slot "Ring" and in the suggestions.
//! - AC2: the implicits as the page shows them.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crafting_core::{
    apply_orb_of_alteration, apply_orb_of_transmutation, base_item, base_items, mod_pool,
    unblocked_mods, valid_mods, AffixType, ExplicitMod, ExplicitModDefinition, Item, ModDefinition,
    Rarity, SeededRng, Slot, StatDefinition,
};

// ---------------------------------------------------------------------------------------
// Values from the story, the PO's rule and the RePoE export 3.29.3.3
// ---------------------------------------------------------------------------------------

/// The five damage types of the PO's rule, as tags of the game data.
const DAMAGE_TYPES: [&str; 5] = ["physical", "lightning", "cold", "fire", "chaos"];

/// Tags that the PO's rule names as not being damage types.
const NOT_DAMAGE_TYPES: [&str; 3] = ["elemental", "physical_damage", "elemental_damage"];

/// A ring of the story: its id, its name and the damage type its implicit line names.
struct BreachRing {
    id: &'static str,
    name: &'static str,
    /// The damage type of the line "Cannot roll Modifiers of Non-<damage type> Damage Types";
    /// `None` for the Formless Ring, which has no such line.
    lock: Option<&'static str>,
}

/// The six rings of the story (RePoE ids `RingB1` to `RingB6`).
const BREACH_RINGS: [BreachRing; 6] = [
    BreachRing {
        id: "Metadata/Items/Rings/RingB1",
        name: "Cryonic Ring",
        lock: Some("cold"),
    },
    BreachRing {
        id: "Metadata/Items/Rings/RingB2",
        name: "Enthalpic Ring",
        lock: Some("fire"),
    },
    BreachRing {
        id: "Metadata/Items/Rings/RingB3",
        name: "Synaptic Ring",
        lock: Some("lightning"),
    },
    BreachRing {
        id: "Metadata/Items/Rings/RingB4",
        name: "Organic Ring",
        lock: Some("physical"),
    },
    BreachRing {
        id: "Metadata/Items/Rings/RingB5",
        name: "Fugitive Ring",
        lock: Some("chaos"),
    },
    BreachRing {
        id: "Metadata/Items/Rings/RingB6",
        name: "Formless Ring",
        lock: None,
    },
];

const CRYONIC_RING_ID: &str = "Metadata/Items/Rings/RingB1";
const FORMLESS_RING_ID: &str = "Metadata/Items/Rings/RingB6";
const IRON_RING_ID: &str = "Metadata/Items/Rings/Ring1";
/// An identifier that is not in the data.
const UNKNOWN_BASE_ID: &str = "Metadata/Items/Rings/NoSuchRing";

/// The item class of every ring base in the game data.
const RINGS_CLASS: &str = "Rings";

/// The example of criterion 2 for the Cryonic Ring: its implicit lines in the game's order.
const CRYONIC_LINES: [&str; 2] = [
    "+2% to maximum Cold Resistance",
    "Cannot roll Modifiers of Non-Cold Damage Types",
];
/// The example of criterion 2 for the Formless Ring in the list (without a rolled value).
const FORMLESS_RANGE_TEXT: &str = "(5-7)% increased Global Defences";
/// The range of that example, which a rolled value replaces.
const FORMLESS_RANGE: &str = "(5-7)";

/// The example of criterion 4 for the Cryonic Ring, by the mod families of the game data:
/// All Elemental Resistances is possible ...
const ALL_ELEMENTAL_RESISTANCES: &str = "AllResistances";
/// ... and never Fire, Lightning or Chaos Resistance, Physical Damage or Life Leech.
const CRYONIC_NEVER: [&str; 5] = [
    "FireResistance",
    "LightningResistance",
    "ChaosResistance",
    "PhysicalDamage",
    "LifeLeech",
];

/// The item level the UI uses by default ("Item level" in `poe-crafting-rules`).
const UI_ITEM_LEVEL: u32 = 100;

/// Fixed seed, so every run is identical.
const SEED: u64 = 188;
/// Orbs of each kind per locked ring (AC4). The rarest event the test waits for is a mod of the
/// Fugitive Ring's own damage type (chaos): about 1.7 % per orb, so about 17 are expected and
/// seeing none has a chance of about 10^-7. All Elemental Resistances on the Cryonic Ring has
/// about 5 % per orb.
const ORBS: u64 = 1_000;
/// Orbs of each kind on the Formless Ring (AC5). The rarest damage type is chaos with about
/// 1.2 % per orb, so about 24 are expected and seeing none has a chance of about 10^-10.
const FORMLESS_ORBS: u64 = 2_000;

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
// Helpers: the PO's rule
// ---------------------------------------------------------------------------------------

/// Whether the PO's rule blocks a mod with these tags on a ring with this lock: the mod has at
/// least one tag of a damage type other than the lock. Only the five damage type tags count,
/// compared as whole tags.
fn blocked(lock: Option<&str>, tags: &[String]) -> bool {
    let Some(lock) = lock else {
        return false;
    };
    tags.iter()
        .any(|tag| DAMAGE_TYPES.contains(&tag.as_str()) && tag != lock)
}

/// The damage type tags of a mod.
fn damage_types_of(tags: &[String]) -> BTreeSet<String> {
    tags.iter()
        .filter(|tag| DAMAGE_TYPES.contains(&tag.as_str()))
        .cloned()
        .collect()
}

fn names(mods: &[ExplicitModDefinition]) -> Vec<String> {
    mods.iter()
        .map(|definition| definition.name.clone())
        .collect()
}

/// An invented prefix with the given tags, named after them.
fn invented_mod(name: &str, tags: &[&str]) -> ExplicitModDefinition {
    ExplicitModDefinition {
        name: name.to_owned(),
        affix_type: AffixType::Prefix,
        weight: 1_000,
        definition: ModDefinition {
            family: format!("Invented {name}"),
            level: 1,
            tags: tags.iter().map(|tag| (*tag).to_owned()).collect(),
            stats: vec![StatDefinition {
                id: "invented_stat".to_owned(),
                min: 1,
                max: 2,
            }],
            text: "{value} Invented".to_owned(),
            placeholders: BTreeMap::from([("value".to_owned(), "invented_stat".to_owned())]),
            conversions: BTreeMap::new(),
        },
    }
}

/// Invented mods for every case of the rule: no tag, each damage type alone and after other
/// tags, each tag that is not a damage type, and each pair of two damage types.
fn invented_mods() -> Vec<ExplicitModDefinition> {
    let mut mods = vec![invented_mod("untagged", &[])];
    for damage_type in DAMAGE_TYPES {
        mods.push(invented_mod(damage_type, &[damage_type]));
        mods.push(invented_mod(
            &format!("damage attack {damage_type}"),
            &["damage", "attack", damage_type],
        ));
    }
    for tag in NOT_DAMAGE_TYPES {
        mods.push(invented_mod(tag, &[tag]));
    }
    mods.push(invented_mod("all non damage types", &NOT_DAMAGE_TYPES));
    for (index, first) in DAMAGE_TYPES.iter().enumerate() {
        for second in &DAMAGE_TYPES[index + 1..] {
            mods.push(invented_mod(
                &format!("{first} {second}"),
                &[*first, *second],
            ));
            mods.push(invented_mod(
                &format!("{second} {first}"),
                &[*second, *first],
            ));
        }
    }
    mods
}

// ---------------------------------------------------------------------------------------
// Helpers: crafting
// ---------------------------------------------------------------------------------------

fn ring(id: &str) -> &'static BreachRing {
    BREACH_RINGS
        .iter()
        .find(|ring| ring.id == id)
        .unwrap_or_else(|| panic!("the test expects no ring {id:?}"))
}

/// The pool of a base as the page builds it: its mods from the game data without the blocked
/// ones, at the UI's item level.
fn page_pool(base_id: &str) -> Vec<ExplicitModDefinition> {
    let pool = mod_pool(base_id)
        .unwrap_or_else(|| panic!("mod_pool({base_id:?}) must offer the base's pool"));
    valid_mods(&unblocked_mods(base_id, &pool), UI_ITEM_LEVEL)
}

/// The rolled prefix and suffix of a magic item.
fn explicit_mods_of(item: &Item) -> Vec<&ExplicitMod> {
    item.prefix.iter().chain(item.suffix.iter()).collect()
}

/// What Orbs of Transmutation and of Alteration rolled on `orbs` normal items of a base.
#[derive(Default)]
struct Rolled {
    by_transmutation: Vec<ExplicitMod>,
    by_alteration: Vec<ExplicitMod>,
}

/// Applies an Orb of Transmutation and then an Orb of Alteration to `orbs` new normal items of
/// `base`, with `pool` as the valid mods, and collects the rolled mods of each orb.
fn roll_orbs(
    base_id: &str,
    pool: &[ExplicitModDefinition],
    orbs: u64,
    rng: &mut SeededRng,
) -> Rolled {
    let base = base_item(base_id).unwrap_or_else(|| panic!("crafting-core must offer {base_id}"));
    let mut rolled = Rolled::default();
    for _ in 0..orbs {
        let mut item = Item::new(&base, rng);
        apply_orb_of_transmutation(&mut item, pool, rng)
            .unwrap_or_else(|e| panic!("{base_id}: transmutation failed with {e:?}"));
        assert_eq!(item.rarity, Rarity::Magic, "{base_id}");
        rolled
            .by_transmutation
            .extend(explicit_mods_of(&item).into_iter().cloned());
        apply_orb_of_alteration(&mut item, pool, rng)
            .unwrap_or_else(|e| panic!("{base_id}: alteration failed with {e:?}"));
        assert_eq!(item.rarity, Rarity::Magic, "{base_id}");
        rolled
            .by_alteration
            .extend(explicit_mods_of(&item).into_iter().cloned());
    }
    rolled
}

// ---------------------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------------------

// AC1: „Cryonic, Enthalpic, Formless, Fugitive, Organic und Synaptic Ring sind im Slot „Ring“
// und in den Vorschlägen wählbar. (Ersetzt für diese sechs Ringe Kriterium 2 von AB#185.)"
// (Tech lead's plan: the six ids with their names in `Slot::Ring.base_items()` and from
// `base_item(id)`; the picker and its suggestions take their bases from there (ADR 0018) and
// are checked in the local preview. The tests of AB#185 and AB#186 that counted 31 ring bases
// and excluded these six were adapted in the same commit, as the PO confirmed on AB#188.)
#[test]
fn ac1_the_six_breach_rings_are_ring_bases_that_can_be_chosen() {
    let mut rng = SeededRng::new(SEED);
    // Given the bases of the slot "Ring" and all bases of crafting-core
    let in_slot = Slot::Ring.base_items();
    let all_ids: Vec<String> = base_items().into_iter().map(|base| base.id).collect();

    for expected in &BREACH_RINGS {
        let what = format!("{} {}", expected.name, expected.id);
        // When looking for the ring in the slot
        let found: Vec<_> = in_slot
            .iter()
            .filter(|base| base.id == expected.id)
            .collect();

        // Then the slot offers it once, with its name from the game data and the ring class
        assert_eq!(
            found.len(),
            1,
            "{what}: Slot::Ring.base_items() must offer it once"
        );
        let base = found[0];
        assert_eq!(base.name, expected.name, "{what}: name");
        assert_eq!(base.item_class, RINGS_CLASS, "{what}: item class");
        assert!(
            all_ids.contains(&base.id),
            "{what}: base_items() must offer it"
        );
        // And crafting-core offers the same base by its id
        assert_eq!(
            base_item(expected.id).as_ref(),
            Some(base),
            "{what}: base_item()"
        );
        // And it has its implicit and a pool, so choosing it gives a normal item to craft on
        assert!(!base.implicits.is_empty(), "{what}: must have its implicit");
        assert!(
            mod_pool(expected.id).is_some_and(|pool| !pool.is_empty()),
            "{what}: mod_pool() must offer the base's pool"
        );
        let item = Item::new(base, &mut rng);
        assert_eq!(item.rarity, Rarity::Normal, "{what}");
        assert_eq!(&item.base, base, "{what}");
        assert_eq!(item.implicits.len(), base.implicits.len(), "{what}");
    }

    // And the six rings have six different names
    let distinct: BTreeSet<&str> = BREACH_RINGS.iter().map(|ring| ring.name).collect();
    assert_eq!(
        distinct.len(),
        BREACH_RINGS.len(),
        "the test lists six names"
    );
}

// AC2: „Ihre Implicits erscheinen wie im Spiel, die Zeilen in der Reihenfolge des Spiels.
// Beispiel Cryonic Ring: `+2% to maximum Cold Resistance` steht vor `Cannot roll Modifiers of
// Non-Cold Damage Types`. Beispiel Formless Ring in der Liste: `(5-7)% increased Global
// Defences`."
// (Tech lead's plan: `range_text()` and `display_text()` of the implicits. The game's order of
// the lines is the order of `stat_translations.json` (ADR 0015); for all five locked rings the
// line of the lock comes last there. The line of the lock shows no value (format `ignore`,
// ADR 0017). The texts of all six rings are also checked by the adapted tests of AB#185 AC1 and
// AB#186 AC6. The page itself is checked in the local preview.)
#[test]
fn ac2_the_implicits_appear_as_in_the_game_with_the_lines_in_the_game_order() {
    let mut rng = SeededRng::new(SEED);

    // --- The example of the Cryonic Ring ---
    // Given the Cryonic Ring
    let cryonic = base_item(CRYONIC_RING_ID)
        .unwrap_or_else(|| panic!("crafting-core must offer {CRYONIC_RING_ID}"));
    // When its implicits are shown in the list and on a normal item
    let listed: Vec<String> = cryonic
        .implicits
        .iter()
        .flat_map(|implicit| {
            implicit
                .range_text()
                .lines()
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .collect();
    let item = Item::new(&cryonic, &mut rng);
    let shown: Vec<String> = item
        .implicits
        .iter()
        .flat_map(|implicit| {
            implicit
                .display_text()
                .lines()
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .collect();
    // Then both show the two lines of the example, the resistance before the lock
    assert_eq!(listed, CRYONIC_LINES, "the Cryonic Ring in the list");
    assert_eq!(shown, CRYONIC_LINES, "the Cryonic Ring as an item");

    // --- The example of the Formless Ring ---
    // Given the Formless Ring
    let formless = base_item(FORMLESS_RING_ID)
        .unwrap_or_else(|| panic!("crafting-core must offer {FORMLESS_RING_ID}"));
    assert_eq!(formless.implicits.len(), 1, "the Formless Ring's implicit");
    // When its implicit is shown in the list
    // Then it shows the range of the example
    assert_eq!(
        formless.implicits[0].range_text(),
        FORMLESS_RANGE_TEXT,
        "the Formless Ring in the list"
    );
    // And a normal Formless Ring shows its rolled value in place of the range
    for _ in 0..20 {
        let item = Item::new(&formless, &mut rng);
        let implicit = &item.implicits[0];
        assert_eq!(
            implicit.stats.len(),
            1,
            "the Formless Ring's implicit has one stat"
        );
        let value = implicit.stats[0].value;
        assert!(
            (5..=7).contains(&value),
            "the Formless Ring rolled {value}, outside 5..=7"
        );
        assert_eq!(
            implicit.display_text(),
            FORMLESS_RANGE_TEXT.replace(FORMLESS_RANGE, &value.to_string()),
            "the Formless Ring as an item"
        );
    }

    // --- The order of the lines on all five locked rings ---
    for expected in BREACH_RINGS.iter().filter(|ring| ring.lock.is_some()) {
        let what = format!("{} {}", expected.name, expected.id);
        // Given the ring's implicit
        let base =
            base_item(expected.id).unwrap_or_else(|| panic!("crafting-core must offer {what}"));
        assert_eq!(base.implicits.len(), 1, "{what}: one implicit");
        let implicit = &base.implicits[0];
        let lines: Vec<&str> = implicit.text.split('\n').collect();
        // Then it has two lines: first the line with the value, last the line of the lock,
        // which shows no value
        assert_eq!(lines.len(), 2, "{what}: two lines, found {lines:?}");
        assert!(
            lines[0].contains('{'),
            "{what}: the first line shows the value, found {lines:?}"
        );
        assert!(
            !lines[1].contains('{'),
            "{what}: the last line is the lock without a value, found {lines:?}"
        );
        // And the list and a normal item show the same lines, because all values are fixed
        let item = Item::new(&base, &mut rng);
        assert_eq!(
            item.implicits[0].display_text(),
            implicit.range_text(),
            "{what}"
        );
        let shown = implicit.range_text();
        assert_eq!(shown.lines().count(), 2, "{what}: {shown:?}");
        assert!(
            !shown.contains('{'),
            "{what}: placeholder left in {shown:?}"
        );
    }
}

// AC3: „Regel des PO: Es gibt genau fünf Schadensarten: Physical, Lightning, Cold, Fire, Chaos.
// Die Zeile „Cannot roll Modifiers of Non-<Schadensart> Damage Types“ sperrt jeden Mod, der
// mindestens einen Tag einer der vier anderen Schadensarten trägt; ein gesperrter Mod kann auf
// diesem Ring nie rollen. Maßgeblich sind die Tags des Mods in den Spieldaten; Schadensarten
// sind dort genau die Tags `physical`, `lightning`, `cold`, `fire` und `chaos`. Andere Tags wie
// `elemental`, `physical_damage` oder `elemental_damage` sind keine Schadensarten."
// (Followed by the binding English wording, which is the rule "Breach rings" in
// `poe-crafting-rules`. Tech lead's plan: `unblocked_mods` with invented mods on the five locked
// rings and the Formless Ring. That a blocked mod never rolls is checked with the real pools in
// AC4.)
#[test]
fn ac3_the_lock_blocks_every_mod_with_a_tag_of_another_damage_type() {
    // Given invented mods for every case of the rule
    let mods = invented_mods();

    for ring in &BREACH_RINGS {
        let what = format!("{} {}", ring.name, ring.id);
        // When the lock of the ring is applied
        let kept = unblocked_mods(ring.id, &mods);

        // Then exactly the mods without a tag of another damage type are kept, unchanged and in
        // their order
        let expected: Vec<ExplicitModDefinition> = mods
            .iter()
            .filter(|definition| !blocked(ring.lock, &definition.definition.tags))
            .cloned()
            .collect();
        assert_eq!(
            names(&kept),
            names(&expected),
            "{what}: the kept mods, lock {:?}",
            ring.lock
        );
        assert_eq!(kept, expected, "{what}: the kept mods are unchanged");

        let kept_names: BTreeSet<String> = names(&kept).into_iter().collect();
        // And the tags that are not damage types block nothing
        for tag in NOT_DAMAGE_TYPES {
            assert!(kept_names.contains(tag), "{what}: {tag:?} must not block");
        }
        assert!(kept_names.contains("all non damage types"), "{what}");
        assert!(kept_names.contains("untagged"), "{what}");
        for damage_type in DAMAGE_TYPES {
            let alone = kept_names.contains(damage_type);
            let after_others = kept_names.contains(&format!("damage attack {damage_type}"));
            match ring.lock {
                // And on a locked ring, only the own damage type is kept, wherever its tag is
                Some(lock) => {
                    assert_eq!(alone, damage_type == lock, "{what}: {damage_type:?}");
                    assert_eq!(after_others, damage_type == lock, "{what}: {damage_type:?}");
                }
                // And the Formless Ring keeps every damage type
                None => assert!(alone && after_others, "{what}: {damage_type:?}"),
            }
        }
        // And a mod with two damage types is blocked on every locked ring, also when one of
        // them is the ring's own damage type
        let pairs = kept
            .iter()
            .filter(|definition| damage_types_of(&definition.definition.tags).len() == 2)
            .count();
        let expected_pairs = if ring.lock.is_some() { 0 } else { 20 };
        assert_eq!(
            pairs, expected_pairs,
            "{what}: mods with two damage types kept"
        );
    }

    // Given the Iron Ring, which has no lock, and an identifier that is not in the data
    for id in [IRON_RING_ID, UNKNOWN_BASE_ID] {
        // When their lock is applied
        // Then nothing is blocked
        assert_eq!(unblocked_mods(id, &mods), mods, "{id}: nothing is blocked");
    }
    // And no mods stay no mods
    assert!(unblocked_mods(CRYONIC_RING_ID, &[]).is_empty());
}

// AC4: „Orb of Transmutation und Orb of Alteration würfeln auf diesen fünf Ringen nur Mods, die
// nach Kriterium 3 nicht gesperrt sind. Beispiel Cryonic Ring: nie Fire-, Lightning- oder
// Chaos-Resistenz, nie Physical Damage, nie Life Leech; All Elemental Resistances ist möglich."
// (Tech lead's plan: real data, the pool `valid_mods(unblocked_mods(id, mod_pool(id)), 100)` as
// the page builds it (ADR 0020); the orbs take the valid mods and do not filter them again
// (ADR 0013). The examples are the mod families of the game data. The page builds its pool with
// `unblocked_mods`.)
#[test]
fn ac4_the_orbs_roll_only_unblocked_mods_on_the_five_locked_rings() {
    let mut rng = SeededRng::new(SEED);

    for expected in BREACH_RINGS.iter().filter(|ring| ring.lock.is_some()) {
        let what = format!("{} {}", expected.name, expected.id);
        let lock = expected.lock;
        // Given the ring's pool as the page builds it
        let all_mods = mod_pool(expected.id)
            .unwrap_or_else(|| panic!("{what}: mod_pool() must offer the base's pool"));
        let pool = page_pool(expected.id);

        // Then it is the ring's pool without exactly the mods the rule blocks
        let expected_pool: Vec<ExplicitModDefinition> = valid_mods(&all_mods, UI_ITEM_LEVEL)
            .into_iter()
            .filter(|definition| !blocked(lock, &definition.definition.tags))
            .collect();
        assert_eq!(
            names(&pool),
            names(&expected_pool),
            "{what}: the page's pool"
        );
        assert_eq!(pool, expected_pool, "{what}: the mods of the page's pool");
        assert!(
            pool.len() < all_mods.len(),
            "{what}: the lock must block mods of the ring's pool"
        );
        let pool_names: BTreeSet<String> = names(&pool).into_iter().collect();

        // When Orbs of Transmutation and Alteration are applied
        let rolled = roll_orbs(expected.id, &pool, ORBS, &mut rng);

        for (orb, mods) in [
            ("Orb of Transmutation", &rolled.by_transmutation),
            ("Orb of Alteration", &rolled.by_alteration),
        ] {
            // Then every rolled mod comes from that pool and has no tag of another damage type
            for explicit in mods {
                assert!(
                    pool_names.contains(&explicit.name),
                    "{what}, {orb}: {:?} is not in the page's pool",
                    explicit.name
                );
                assert!(
                    !blocked(lock, &explicit.modifier.tags),
                    "{what}, {orb}: rolled the blocked mod {:?} with the tags {:?}",
                    explicit.name,
                    explicit.modifier.tags
                );
            }
            // And mods of the ring's own damage type still roll
            let own = lock.expect("only locked rings");
            assert!(
                mods.iter()
                    .any(|explicit| explicit.modifier.tags.iter().any(|tag| tag == own)),
                "{what}, {orb}: {ORBS} orbs rolled no mod of the damage type {own:?}"
            );
        }
    }

    // --- The example of the Cryonic Ring ---
    // Given the Cryonic Ring's pool as the page builds it
    let pool = page_pool(CRYONIC_RING_ID);
    let families: BTreeSet<&str> = pool
        .iter()
        .map(|definition| definition.definition.family.as_str())
        .collect();
    // Then it has none of the mods of the example, but All Elemental Resistances
    for family in CRYONIC_NEVER {
        assert!(
            !families.contains(family),
            "the Cryonic Ring's pool must not contain {family:?}"
        );
    }
    assert!(
        families.contains(ALL_ELEMENTAL_RESISTANCES),
        "the Cryonic Ring's pool must contain {ALL_ELEMENTAL_RESISTANCES:?}"
    );
    // When Orbs of Transmutation and Alteration are applied to Cryonic Rings
    let rolled = roll_orbs(CRYONIC_RING_ID, &pool, ORBS, &mut rng);
    for (orb, mods) in [
        ("Orb of Transmutation", &rolled.by_transmutation),
        ("Orb of Alteration", &rolled.by_alteration),
    ] {
        // Then they never roll the mods of the example
        for explicit in mods.iter() {
            assert!(
                !CRYONIC_NEVER.contains(&explicit.modifier.family.as_str()),
                "Cryonic Ring, {orb}: rolled {:?} ({})",
                explicit.name,
                explicit.modifier.family
            );
        }
        // And they roll All Elemental Resistances
        assert!(
            mods.iter()
                .any(|explicit| explicit.modifier.family == ALL_ELEMENTAL_RESISTANCES),
            "Cryonic Ring, {orb}: {ORBS} orbs rolled no {ALL_ELEMENTAL_RESISTANCES:?}"
        );
    }

    // And the page builds its pool with the lock
    assert_web_src_uses(
        "unblocked_mods",
        "the orbs on the page roll only mods the ring's lock does not block",
    );
}

// AC5: „Formless Ring hat keine Sperre und würfelt aus demselben Pool wie der Iron Ring."
// (Tech lead's plan: `mod_pool` of the Formless Ring is the Iron Ring's pool and
// `unblocked_mods` does not change it; the orbs roll mods of all five damage types.)
#[test]
fn ac5_the_formless_ring_has_no_lock_and_rolls_from_the_iron_ring_pool() {
    let mut rng = SeededRng::new(SEED);
    assert_eq!(
        ring(FORMLESS_RING_ID).lock,
        None,
        "the test expects no lock"
    );
    // Given the pools of the Formless Ring and of the Iron Ring
    let formless_mods = mod_pool(FORMLESS_RING_ID)
        .unwrap_or_else(|| panic!("mod_pool({FORMLESS_RING_ID:?}) must offer the ring's pool"));
    let iron_mods = mod_pool(IRON_RING_ID)
        .unwrap_or_else(|| panic!("mod_pool({IRON_RING_ID:?}) must offer the ring's pool"));

    // Then the Formless Ring's pool is the Iron Ring's pool, in the same order
    assert_eq!(
        names(&formless_mods),
        names(&iron_mods),
        "the Formless Ring's pool"
    );
    assert_eq!(
        formless_mods, iron_mods,
        "the mods of the Formless Ring's pool"
    );
    // And the lock blocks none of its mods
    assert_eq!(
        unblocked_mods(FORMLESS_RING_ID, &formless_mods),
        formless_mods,
        "the Formless Ring has no lock"
    );
    // And the page's pool is the Iron Ring's pool at the UI's item level
    let pool = page_pool(FORMLESS_RING_ID);
    assert_eq!(
        pool,
        valid_mods(&iron_mods, UI_ITEM_LEVEL),
        "the page's pool for the Formless Ring"
    );
    let iron_names: BTreeSet<String> = names(&iron_mods).into_iter().collect();

    // When Orbs of Transmutation and Alteration are applied to Formless Rings
    let rolled = roll_orbs(FORMLESS_RING_ID, &pool, FORMLESS_ORBS, &mut rng);
    for (orb, mods) in [
        ("Orb of Transmutation", &rolled.by_transmutation),
        ("Orb of Alteration", &rolled.by_alteration),
    ] {
        // Then every rolled mod comes from the Iron Ring's pool
        for explicit in mods {
            assert!(
                iron_names.contains(&explicit.name),
                "Formless Ring, {orb}: {:?} is not in the Iron Ring's pool",
                explicit.name
            );
        }
        // And mods of all five damage types roll
        let seen: BTreeSet<String> = mods
            .iter()
            .flat_map(|explicit| damage_types_of(&explicit.modifier.tags))
            .collect();
        let all: BTreeSet<String> = DAMAGE_TYPES.iter().map(|t| (*t).to_owned()).collect();
        assert_eq!(
            seen, all,
            "Formless Ring, {orb}: damage types of the rolled mods"
        );
    }
}
