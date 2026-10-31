//! Acceptance tests for AB#185 "Ring-Basen aus RePoE importieren".
//!
//! The tests use the public interface from the tech lead's plan on AB#185 and ADR 0017:
//! - `crafting-core`: `mod_pool(base_id) -> Option<Vec<ExplicitModDefinition>>` (the mods of the
//!   base's `mod_pool` in the data file, in that order; `None` for an unknown base) and
//!   `Conversion { divisor: i32, precision }`, where the divisor -1 stands for RePoE's handler
//!   `negate`, together with the unchanged `base_items()`, `base_item(id)`, `explicit_mods()`,
//!   `valid_mods`, `Item::new` and `Mod::display_text()`.
//! - `data-import` (dev-dependency of `crafting-core`): the unchanged `convert(raw, base_ids)`.
//!
//! The expected game data (the 43 bases with name, tags and implicits, the line order, the
//! negated stats, the pools and the additional mods) was taken by a program from the PO's
//! RePoE export 3.29.3.3; the raw files are not in the repository. Lines are in the order of
//! their entries in `stat_translations.json` (ADR 0015). Displayed values follow the PO's
//! rules in `poe-crafting-rules`, "Item basics". Every random test uses a fixed seed.
//!
//! Red state before the implementation: the module does not compile, because `mod_pool` does
//! not exist yet and `Conversion::divisor` is still unsigned.
//!
//! What is checked outside `cargo test`:
//! - AC1: that `data/README.md` is updated (its wording is checked in the review against the
//!   criterion), and that the developer's second converter run on the raw files gives no diff.
//! - AC6: the page itself, by the PO in the local preview.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crafting_core::{
    base_item, base_items, explicit_mods, mod_pool, valid_mods, AffixType, Conversion,
    ExplicitModDefinition, Item, Mod, ModDefinition, SeededRng, StatDefinition,
};
use data_import::{convert, RawFiles};
use serde_json::{json, Value};

// ---------------------------------------------------------------------------------------
// Values from the story, the PO's decisions and the RePoE export 3.29.3.3
// ---------------------------------------------------------------------------------------

const IRON_RING_ID: &str = "Metadata/Items/Rings/Ring1";
const IRON_RING_NAME: &str = "Iron Ring";
/// The item class of every ring base (`item_classes.json`).
const RINGS_CLASS: &str = "Rings";

/// Size of the Iron Ring's pool (categories `prefix` and `suffix`, ADR 0016).
const IRON_POOL_SIZE: usize = 207;
/// Number of mods in the pools of all 43 bases together.
const ALL_MODS: usize = 233;

/// The item level the UI uses by default ("Item level" in `poe-crafting-rules`).
const UI_ITEM_LEVEL: u32 = 100;

/// The bases the PO excluded (criterion 2): the Royale variants. They carry the names of
/// included rings, so they are checked by their ids. The Breach rings `RingB1` to `RingB6` are
/// no longer excluded: AB#188 criterion 1 replaces criterion 2 for them (PO confirmation on
/// AB#188). The Heist rings `RingE1` to `RingE6` are no longer excluded either: AB#189
/// criterion 1 replaces criterion 2 for them (PO confirmation on AB#189).
const EXCLUDED_BASES: [&str; 4] = [
    "Metadata/Items/Rings/RingRoyale1",
    "Metadata/Items/Rings/RingRoyale2",
    "Metadata/Items/Rings/RingRoyale3",
    "Metadata/Items/Rings/RingRoyale4",
];

/// Bases with the same name and different implicits (criterion 4).
const TWO_STONE_RING: (&str, [&str; 3]) = (
    "Two-Stone Ring",
    [
        "Metadata/Items/Rings/Ring12",
        "Metadata/Items/Rings/Ring13",
        "Metadata/Items/Rings/Ring14",
    ],
);
const SHADOWED_RING: (&str, [&str; 3]) = (
    "Shadowed Ring",
    [
        "Metadata/Items/Rings/RingK5a",
        "Metadata/Items/Rings/RingK5b",
        "Metadata/Items/Rings/RingK5c",
    ],
);

/// The example of criterion 5: the first line of the Dusk Ring's implicit.
const DUSK_RING_ID: &str = "Metadata/Items/Rings/RingK1";
const DUSK_RING_LEFT_LINE: &str = "Left ring slot: 15% reduced Skill Effect Duration";

/// The divisor that stands for RePoE's handler `negate` (ADR 0017); the handler has no
/// precision.
const NEGATE_DIVISOR: i32 = -1;

/// Raw value and shown value of a negated stat, by the PO's rules in "Item basics".
const NEGATED_EXAMPLES: [(i32, &str); 5] =
    [(-15, "15"), (-30, "30"), (-50, "50"), (-1, "1"), (7, "-7")];

/// Fixed seed, so every run is identical.
const SEED: u64 = 185;
/// Normal items per base for the display check.
const ROLLS: u64 = 50;

const BASE_ITEMS_JSON: &str = "data/base_items.json";
const MODS_JSON: &str = "data/mods.json";

/// An implicit as expected in the data and in crafting-core.
struct ExpectedImplicit {
    family: &'static str,
    level: u32,
    tags: &'static [&'static str],
    /// Stat identifier, min, max (raw values).
    stats: &'static [(&'static str, i32, i32)],
    /// The stats whose placeholders each line of the text holds, line by line in the game's
    /// order. A line of a stat with the format `ignore` holds none.
    lines: &'static [&'static [&'static str]],
    /// The stats the game data shows through the handler `negate`.
    negated: &'static [&'static str],
}

/// A base as expected in the data and in crafting-core.
struct ExpectedBase {
    id: &'static str,
    name: &'static str,
    tags: &'static [&'static str],
    implicits: &'static [ExpectedImplicit],
    /// The mods of its pool that are not in the Iron Ring's pool.
    extra_mods: &'static [ExtraMod],
}

/// A mod of a pool that the Iron Ring does not have, with its values from RePoE.
struct ExtraMod {
    id: &'static str,
    name: &'static str,
    affix_type: &'static str,
    family: &'static str,
    level: u32,
    weight: u32,
    /// Stat identifier, min, max (raw values).
    stat: (&'static str, i32, i32),
}

/// The 43 ring bases of the PO's decisions: item class Ring, release state `released`, without
/// the excluded bases, Golden Hoop (unique only) and Jet Ring (unreleased). The Breach rings
/// `RingB1` to `RingB6` were added by AB#188, the last six, the Heist rings `RingE1` to
/// `RingE6`, by AB#189. A Heist ring's magnitude stat of 0 % has no line (ADR 0022).
const BASES: [ExpectedBase; 43] = [
    ExpectedBase {
        id: "Metadata/Items/Rings/Ring1",
        name: "Iron Ring",
        tags: &["ring", "default"],
        implicits: &[ExpectedImplicit {
            family: "PhysicalDamage",
            level: 2,
            tags: &["physical_damage", "damage", "physical", "attack"],
            stats: &[
                ("attack_minimum_added_physical_damage", 1, 1),
                ("attack_maximum_added_physical_damage", 4, 4),
            ],
            lines: &[&[
                "attack_minimum_added_physical_damage",
                "attack_maximum_added_physical_damage",
            ]],
            negated: &[],
        }],
        extra_mods: &[],
    },
    ExpectedBase {
        id: "Metadata/Items/Rings/Ring2",
        name: "Coral Ring",
        tags: &["ring", "default"],
        implicits: &[ExpectedImplicit {
            family: "IncreasedLife",
            level: 2,
            tags: &["resource", "life"],
            stats: &[("base_maximum_life", 20, 30)],
            lines: &[&["base_maximum_life"]],
            negated: &[],
        }],
        extra_mods: &[],
    },
    ExpectedBase {
        id: "Metadata/Items/Rings/Ring3",
        name: "Paua Ring",
        tags: &["ring", "default"],
        implicits: &[ExpectedImplicit {
            family: "IncreasedMana",
            level: 2,
            tags: &["resource", "mana"],
            stats: &[("base_maximum_mana", 20, 30)],
            lines: &[&["base_maximum_mana"]],
            negated: &[],
        }],
        extra_mods: &[],
    },
    ExpectedBase {
        id: "Metadata/Items/Rings/Ring4",
        name: "Gold Ring",
        tags: &["ring", "default"],
        implicits: &[ExpectedImplicit {
            family: "ItemFoundRarityIncrease",
            level: 25,
            tags: &["drop"],
            stats: &[("base_item_found_rarity_+%", 6, 15)],
            lines: &[&["base_item_found_rarity_+%"]],
            negated: &[],
        }],
        extra_mods: &[],
    },
    ExpectedBase {
        id: "Metadata/Items/Rings/Ring5",
        name: "Topaz Ring",
        tags: &["ring", "default"],
        implicits: &[ExpectedImplicit {
            family: "LightningResistance",
            level: 15,
            tags: &["elemental", "lightning", "resistance"],
            stats: &[("base_lightning_damage_resistance_%", 20, 30)],
            lines: &[&["base_lightning_damage_resistance_%"]],
            negated: &[],
        }],
        extra_mods: &[],
    },
    ExpectedBase {
        id: "Metadata/Items/Rings/Ring6",
        name: "Sapphire Ring",
        tags: &["ring", "default"],
        implicits: &[ExpectedImplicit {
            family: "ColdResistance",
            level: 10,
            tags: &["elemental", "cold", "resistance"],
            stats: &[("base_cold_damage_resistance_%", 20, 30)],
            lines: &[&["base_cold_damage_resistance_%"]],
            negated: &[],
        }],
        extra_mods: &[],
    },
    ExpectedBase {
        id: "Metadata/Items/Rings/Ring7",
        name: "Ruby Ring",
        tags: &["ring", "default"],
        implicits: &[ExpectedImplicit {
            family: "FireResistance",
            level: 20,
            tags: &["elemental", "fire", "resistance"],
            stats: &[("base_fire_damage_resistance_%", 20, 30)],
            lines: &[&["base_fire_damage_resistance_%"]],
            negated: &[],
        }],
        extra_mods: &[],
    },
    ExpectedBase {
        id: "Metadata/Items/Rings/Ring8",
        name: "Prismatic Ring",
        tags: &["ring", "default"],
        implicits: &[ExpectedImplicit {
            family: "AllResistances",
            level: 38,
            tags: &["elemental", "resistance"],
            stats: &[("base_resist_all_elements_%", 8, 10)],
            lines: &[&["base_resist_all_elements_%"]],
            negated: &[],
        }],
        extra_mods: &[],
    },
    ExpectedBase {
        id: "Metadata/Items/Rings/Ring9",
        name: "Moonstone Ring",
        tags: &["ring", "default"],
        implicits: &[ExpectedImplicit {
            family: "IncreasedCastSpeed",
            level: 25,
            tags: &["caster", "speed"],
            stats: &[("base_cast_speed_+%", 7, 10)],
            lines: &[&["base_cast_speed_+%"]],
            negated: &[],
        }],
        extra_mods: &[],
    },
    ExpectedBase {
        id: "Metadata/Items/Rings/Ring10",
        name: "Amethyst Ring",
        tags: &["ring", "default"],
        implicits: &[ExpectedImplicit {
            family: "ChaosResistance",
            level: 38,
            tags: &["chaos", "resistance"],
            stats: &[("base_chaos_damage_resistance_%", 17, 23)],
            lines: &[&["base_chaos_damage_resistance_%"]],
            negated: &[],
        }],
        extra_mods: &[],
    },
    ExpectedBase {
        id: "Metadata/Items/Rings/Ring11",
        name: "Diamond Ring",
        tags: &["ring", "default"],
        implicits: &[ExpectedImplicit {
            family: "CriticalStrikeChanceIncrease",
            level: 25,
            tags: &["critical"],
            stats: &[("critical_strike_chance_+%", 20, 30)],
            lines: &[&["critical_strike_chance_+%"]],
            negated: &[],
        }],
        extra_mods: &[],
    },
    ExpectedBase {
        id: "Metadata/Items/Rings/Ring12",
        name: "Two-Stone Ring",
        tags: &["twostonering", "ring", "default"],
        implicits: &[ExpectedImplicit {
            family: "FireAndLightningResistance",
            level: 25,
            tags: &["elemental", "fire", "lightning", "resistance"],
            stats: &[("fire_and_lightning_damage_resistance_%", 12, 16)],
            lines: &[&["fire_and_lightning_damage_resistance_%"]],
            negated: &[],
        }],
        extra_mods: &[],
    },
    ExpectedBase {
        id: "Metadata/Items/Rings/Ring13",
        name: "Two-Stone Ring",
        tags: &["twostonering", "ring", "default"],
        implicits: &[ExpectedImplicit {
            family: "ColdAndLightningResistance",
            level: 25,
            tags: &["elemental", "cold", "lightning", "resistance"],
            stats: &[("cold_and_lightning_damage_resistance_%", 12, 16)],
            lines: &[&["cold_and_lightning_damage_resistance_%"]],
            negated: &[],
        }],
        extra_mods: &[],
    },
    ExpectedBase {
        id: "Metadata/Items/Rings/Ring14",
        name: "Two-Stone Ring",
        tags: &["twostonering", "ring", "default"],
        implicits: &[ExpectedImplicit {
            family: "FireAndColdResistance",
            level: 25,
            tags: &["elemental", "fire", "cold", "resistance"],
            stats: &[("fire_and_cold_damage_resistance_%", 12, 16)],
            lines: &[&["fire_and_cold_damage_resistance_%"]],
            negated: &[],
        }],
        extra_mods: &[],
    },
    ExpectedBase {
        id: "Metadata/Items/Rings/Ring15",
        name: "Unset Ring",
        tags: &["unset_ring", "ring", "default"],
        implicits: &[ExpectedImplicit {
            family: "Other",
            level: 7,
            tags: &[],
            stats: &[("local_has_X_sockets", 1, 1)],
            lines: &[&[]],
            negated: &[],
        }],
        extra_mods: &UNSET_MODS,
    },
    ExpectedBase {
        id: "Metadata/Items/Rings/Ring16",
        name: "Bone Ring",
        tags: &["ring_can_roll_minion_modifiers", "ring", "default"],
        implicits: &[ExpectedImplicit {
            family: "MinionElementalResistances",
            level: 32,
            tags: &["elemental", "resistance", "minion"],
            stats: &[("minion_elemental_resistance_%", 10, 15)],
            lines: &[&["minion_elemental_resistance_%"]],
            negated: &[],
        }],
        extra_mods: &MINION_MODS,
    },
    ExpectedBase {
        id: "Metadata/Items/Rings/RingAtlas1",
        name: "Steel Ring",
        tags: &[
            "not_for_sale",
            "atlas_base_type",
            "ringatlas1",
            "ring",
            "default",
        ],
        implicits: &[ExpectedImplicit {
            family: "PhysicalDamage",
            level: 100,
            tags: &["physical_damage", "damage", "physical", "attack"],
            stats: &[
                ("attack_minimum_added_physical_damage", 3, 4),
                ("attack_maximum_added_physical_damage", 10, 14),
            ],
            lines: &[&[
                "attack_minimum_added_physical_damage",
                "attack_maximum_added_physical_damage",
            ]],
            negated: &[],
        }],
        extra_mods: &[],
    },
    ExpectedBase {
        id: "Metadata/Items/Rings/RingAtlas2",
        name: "Opal Ring",
        tags: &[
            "not_for_sale",
            "atlas_base_type",
            "ringatlas2",
            "ring",
            "default",
        ],
        implicits: &[ExpectedImplicit {
            family: "ElementalDamagePercent",
            level: 100,
            tags: &["elemental_damage", "damage", "elemental"],
            stats: &[("elemental_damage_+%", 15, 25)],
            lines: &[&["elemental_damage_+%"]],
            negated: &[],
        }],
        extra_mods: &[],
    },
    ExpectedBase {
        id: "Metadata/Items/Rings/RingAtlas3",
        name: "Vermillion Ring",
        tags: &[
            "not_for_sale",
            "atlas_base_type",
            "ringatlas3",
            "ring",
            "default",
        ],
        implicits: &[ExpectedImplicit {
            family: "MaximumLifeIncreasePercent",
            level: 100,
            tags: &["resource", "life"],
            stats: &[("maximum_life_+%", 5, 7)],
            lines: &[&["maximum_life_+%"]],
            negated: &[],
        }],
        extra_mods: &[],
    },
    ExpectedBase {
        id: "Metadata/Items/Rings/RingAtlas4",
        name: "Cerulean Ring",
        tags: &[
            "not_for_sale",
            "atlas_base_type",
            "ringatlas4",
            "ring",
            "default",
        ],
        implicits: &[ExpectedImplicit {
            family: "MaximumManaIncreasePercent",
            level: 100,
            tags: &["resource", "mana"],
            stats: &[("maximum_mana_+%", 8, 10)],
            lines: &[&["maximum_mana_+%"]],
            negated: &[],
        }],
        extra_mods: &[],
    },
    ExpectedBase {
        id: "Metadata/Items/Rings/RingAtlas5",
        name: "Iolite Ring",
        tags: &[
            "not_for_sale",
            "atlas_base_type",
            "ringatlas5",
            "ring",
            "default",
        ],
        implicits: &[ExpectedImplicit {
            family: "IncreasedChaosDamage",
            level: 100,
            tags: &["chaos_damage", "damage", "chaos"],
            stats: &[("chaos_damage_+%", 17, 23)],
            lines: &[&["chaos_damage_+%"]],
            negated: &[],
        }],
        extra_mods: &[],
    },
    ExpectedBase {
        id: "Metadata/Items/Rings/RingK1",
        name: "Dusk Ring",
        tags: &["not_for_sale", "ring", "default"],
        implicits: &[ExpectedImplicit {
            family: "ReflectedDurationRingImplicit",
            level: 30,
            tags: &[],
            stats: &[
                ("local_left_ring_slot_skill_effect_duration_+%", -15, -15),
                ("local_right_ring_slot_skill_effect_duration_+%", 15, 15),
            ],
            lines: &[
                &["local_left_ring_slot_skill_effect_duration_+%"],
                &["local_right_ring_slot_skill_effect_duration_+%"],
            ],
            negated: &["local_left_ring_slot_skill_effect_duration_+%"],
        }],
        extra_mods: &[],
    },
    ExpectedBase {
        id: "Metadata/Items/Rings/RingK2",
        name: "Penumbra Ring",
        tags: &["not_for_sale", "ring", "default"],
        implicits: &[ExpectedImplicit {
            family: "ReflectedCurseEffectOnSelf",
            level: 30,
            tags: &["curse"],
            stats: &[
                ("local_left_ring_slot_curse_effect_on_self_+%", -30, -30),
                ("local_right_ring_slot_curse_effect_on_self_+%", 30, 30),
            ],
            lines: &[
                &["local_left_ring_slot_curse_effect_on_self_+%"],
                &["local_right_ring_slot_curse_effect_on_self_+%"],
            ],
            negated: &["local_left_ring_slot_curse_effect_on_self_+%"],
        }],
        extra_mods: &[],
    },
    ExpectedBase {
        id: "Metadata/Items/Rings/RingK3",
        name: "Gloam Ring",
        tags: &[
            "not_for_sale",
            "ring_can_roll_minion_modifiers",
            "ring",
            "default",
        ],
        implicits: &[ExpectedImplicit {
            family: "ReflectedMinionDamageTaken",
            level: 30,
            tags: &["minion"],
            stats: &[
                ("local_left_ring_slot_minion_damage_taken_+%", -15, -15),
                ("local_right_ring_slot_minion_damage_taken_+%", 15, 15),
            ],
            lines: &[
                &["local_left_ring_slot_minion_damage_taken_+%"],
                &["local_right_ring_slot_minion_damage_taken_+%"],
            ],
            negated: &["local_left_ring_slot_minion_damage_taken_+%"],
        }],
        extra_mods: &MINION_MODS,
    },
    ExpectedBase {
        id: "Metadata/Items/Rings/RingK4",
        name: "Tenebrous Ring",
        tags: &["not_for_sale", "ring", "default"],
        implicits: &[ExpectedImplicit {
            family: "ReflectedAilmentDurationOnSelf",
            level: 30,
            tags: &["ailment"],
            stats: &[
                (
                    "local_left_ring_slot_base_all_ailment_duration_on_self_+%",
                    -30,
                    -30,
                ),
                (
                    "local_right_ring_slot_base_all_ailment_duration_on_self_+%",
                    30,
                    30,
                ),
            ],
            lines: &[
                &["local_left_ring_slot_base_all_ailment_duration_on_self_+%"],
                &["local_right_ring_slot_base_all_ailment_duration_on_self_+%"],
            ],
            negated: &["local_left_ring_slot_base_all_ailment_duration_on_self_+%"],
        }],
        extra_mods: &[],
    },
    ExpectedBase {
        id: "Metadata/Items/Rings/RingK5a",
        name: "Shadowed Ring",
        tags: &["not_for_sale", "ring", "default"],
        implicits: &[ExpectedImplicit {
            family: "ReflectedFireColdDamageTakenConversion",
            level: 30,
            tags: &["elemental", "fire", "cold"],
            stats: &[
                ("local_left_ring_slot_cold_damage_taken_%_as_fire", 25, 25),
                ("local_right_ring_slot_fire_damage_taken_%_as_cold", 25, 25),
            ],
            lines: &[
                &["local_left_ring_slot_cold_damage_taken_%_as_fire"],
                &["local_right_ring_slot_fire_damage_taken_%_as_cold"],
            ],
            negated: &[],
        }],
        extra_mods: &[],
    },
    ExpectedBase {
        id: "Metadata/Items/Rings/RingK5b",
        name: "Shadowed Ring",
        tags: &["not_for_sale", "ring", "default"],
        implicits: &[ExpectedImplicit {
            family: "ReflectedFireLightningDamageTakenConversion",
            level: 30,
            tags: &["elemental", "fire", "lightning"],
            stats: &[
                (
                    "local_left_ring_slot_fire_damage_taken_%_as_lightning",
                    25,
                    25,
                ),
                (
                    "local_right_ring_slot_lightning_damage_taken_%_as_fire",
                    25,
                    25,
                ),
            ],
            lines: &[
                &["local_left_ring_slot_fire_damage_taken_%_as_lightning"],
                &["local_right_ring_slot_lightning_damage_taken_%_as_fire"],
            ],
            negated: &[],
        }],
        extra_mods: &[],
    },
    ExpectedBase {
        id: "Metadata/Items/Rings/RingK5c",
        name: "Shadowed Ring",
        tags: &["not_for_sale", "ring", "default"],
        implicits: &[ExpectedImplicit {
            family: "ReflectedColdLightningDamageTakenConversion",
            level: 30,
            tags: &["elemental", "cold", "lightning"],
            stats: &[
                (
                    "local_left_ring_slot_lightning_damage_taken_%_as_cold",
                    25,
                    25,
                ),
                (
                    "local_right_ring_slot_cold_damage_taken_%_as_lightning",
                    25,
                    25,
                ),
            ],
            lines: &[
                &["local_left_ring_slot_lightning_damage_taken_%_as_cold"],
                &["local_right_ring_slot_cold_damage_taken_%_as_lightning"],
            ],
            negated: &[],
        }],
        extra_mods: &[],
    },
    ExpectedBase {
        id: "Metadata/Items/Rings/RingR1",
        name: "Nameless Ring",
        tags: &["not_for_sale", "ring", "default"],
        implicits: &[ExpectedImplicit {
            family: "ReducedCurseEffect",
            level: 30,
            tags: &["elemental"],
            stats: &[
                ("curse_effect_on_self_+%", -50, -50),
                ("self_elemental_status_duration_-%", -50, -50),
            ],
            lines: &[
                &["self_elemental_status_duration_-%"],
                &["curse_effect_on_self_+%"],
            ],
            negated: &[
                "self_elemental_status_duration_-%",
                "curse_effect_on_self_+%",
            ],
        }],
        extra_mods: &[],
    },
    ExpectedBase {
        id: "Metadata/Items/Rings/BreachRing",
        name: "Breach Ring",
        tags: &["not_for_sale", "ring", "default"],
        implicits: &[ExpectedImplicit {
            family: "StatsDoubledInBreach",
            level: 1,
            tags: &[],
            stats: &[("local_item_stats_are_doubled_in_breach", 1, 1)],
            lines: &[&[]],
            negated: &[],
        }],
        extra_mods: &[],
    },
    ExpectedBase {
        id: "Metadata/Items/Rings/MirrorRing",
        name: "Ring",
        tags: &["not_for_sale", "ring", "default"],
        implicits: &[],
        extra_mods: &[],
    },
    ExpectedBase {
        id: "Metadata/Items/Rings/RingB1",
        name: "Cryonic Ring",
        tags: &["not_for_sale", "ring", "default"],
        implicits: &[ExpectedImplicit {
            family: "TulBreachRingImplicit",
            level: 40,
            tags: &["elemental", "cold", "resistance"],
            stats: &[
                ("breach_ring_tul_implicit", 1, 1),
                ("base_maximum_cold_damage_resistance_%", 2, 2),
            ],
            lines: &[&["base_maximum_cold_damage_resistance_%"], &[]],
            negated: &[],
        }],
        extra_mods: &[],
    },
    ExpectedBase {
        id: "Metadata/Items/Rings/RingB2",
        name: "Enthalpic Ring",
        tags: &["not_for_sale", "ring", "default"],
        implicits: &[ExpectedImplicit {
            family: "XophBreachRingImplicit",
            level: 40,
            tags: &["elemental", "fire", "resistance"],
            stats: &[
                ("breach_ring_xoph_implicit", 1, 1),
                ("base_maximum_fire_damage_resistance_%", 2, 2),
            ],
            lines: &[&["base_maximum_fire_damage_resistance_%"], &[]],
            negated: &[],
        }],
        extra_mods: &[],
    },
    ExpectedBase {
        id: "Metadata/Items/Rings/RingB3",
        name: "Synaptic Ring",
        tags: &["not_for_sale", "ring", "default"],
        implicits: &[ExpectedImplicit {
            family: "EshBreachRingImplicit",
            level: 40,
            tags: &["elemental", "lightning", "resistance"],
            stats: &[
                ("breach_ring_esh_implicit", 1, 1),
                ("base_maximum_lightning_damage_resistance_%", 2, 2),
            ],
            lines: &[&["base_maximum_lightning_damage_resistance_%"], &[]],
            negated: &[],
        }],
        extra_mods: &[],
    },
    ExpectedBase {
        id: "Metadata/Items/Rings/RingB4",
        name: "Organic Ring",
        tags: &["not_for_sale", "ring", "default"],
        implicits: &[ExpectedImplicit {
            family: "UulNetolBreachRingImplicit",
            level: 40,
            tags: &["physical"],
            stats: &[
                ("breach_ring_uulnetol_implicit", 1, 1),
                ("base_additional_physical_damage_reduction_%", 3, 3),
            ],
            lines: &[&["base_additional_physical_damage_reduction_%"], &[]],
            negated: &[],
        }],
        extra_mods: &[],
    },
    ExpectedBase {
        id: "Metadata/Items/Rings/RingB5",
        name: "Fugitive Ring",
        tags: &["not_for_sale", "ring", "default"],
        implicits: &[ExpectedImplicit {
            family: "ChayulaBreachRingImplicit",
            level: 40,
            tags: &["chaos", "resistance"],
            stats: &[
                ("breach_ring_chayula_implicit", 1, 1),
                ("base_maximum_chaos_damage_resistance_%", 2, 2),
            ],
            lines: &[&["base_maximum_chaos_damage_resistance_%"], &[]],
            negated: &[],
        }],
        extra_mods: &[],
    },
    ExpectedBase {
        id: "Metadata/Items/Rings/RingB6",
        name: "Formless Ring",
        tags: &["not_for_sale", "ring", "default"],
        implicits: &[ExpectedImplicit {
            family: "FormlessBreachRingImplicit",
            level: 53,
            tags: &["defences"],
            stats: &[("global_defences_+%", 5, 7)],
            lines: &[&["global_defences_+%"]],
            negated: &[],
        }],
        extra_mods: &[],
    },
    ExpectedBase {
        id: "Metadata/Items/Rings/RingE1",
        name: "Cogwork Ring",
        tags: &["not_for_sale", "experimental_base", "ring", "default"],
        implicits: &[ExpectedImplicit {
            family: "MaxPrefixMaxSuffix",
            level: 30,
            tags: &[],
            stats: &[
                ("local_maximum_prefixes_allowed_+", -1, -1),
                ("local_maximum_suffixes_allowed_+", 1, 1),
                ("local_implicit_mod_cannot_be_changed", 1, 1),
                ("local_suffix_mod_effect_+%", 25, 25),
                ("local_prefix_mod_effect_+%", 0, 0),
            ],
            lines: &[
                &["local_maximum_prefixes_allowed_+"],
                &["local_maximum_suffixes_allowed_+"],
                &[],
                &["local_suffix_mod_effect_+%"],
            ],
            negated: &[],
        }],
        extra_mods: &[],
    },
    ExpectedBase {
        id: "Metadata/Items/Rings/RingE2",
        name: "Geodesic Ring",
        tags: &["not_for_sale", "experimental_base", "ring", "default"],
        implicits: &[ExpectedImplicit {
            family: "MaxPrefixMaxSuffix",
            level: 30,
            tags: &[],
            stats: &[
                ("local_maximum_prefixes_allowed_+", 1, 1),
                ("local_maximum_suffixes_allowed_+", -1, -1),
                ("local_implicit_mod_cannot_be_changed", 1, 1),
                ("local_suffix_mod_effect_+%", 0, 0),
                ("local_prefix_mod_effect_+%", 25, 25),
            ],
            lines: &[
                &["local_maximum_prefixes_allowed_+"],
                &["local_maximum_suffixes_allowed_+"],
                &[],
                &["local_prefix_mod_effect_+%"],
            ],
            negated: &[],
        }],
        extra_mods: &[],
    },
    ExpectedBase {
        id: "Metadata/Items/Rings/RingE3",
        name: "Composite Ring",
        tags: &["not_for_sale", "experimental_base", "ring", "default"],
        implicits: &[ExpectedImplicit {
            family: "MaxPrefixMaxSuffix",
            level: 30,
            tags: &[],
            stats: &[
                ("local_maximum_prefixes_allowed_+", 3, 3),
                ("local_maximum_suffixes_allowed_+", -3, -3),
                ("local_implicit_mod_cannot_be_changed", 1, 1),
                ("local_suffix_mod_effect_+%", 0, 0),
                ("local_prefix_mod_effect_+%", 0, 0),
            ],
            lines: &[
                &["local_maximum_prefixes_allowed_+"],
                &["local_maximum_suffixes_allowed_+"],
                &[],
            ],
            negated: &[],
        }],
        extra_mods: &[],
    },
    ExpectedBase {
        id: "Metadata/Items/Rings/RingE4",
        name: "Manifold Ring",
        tags: &["not_for_sale", "experimental_base", "ring", "default"],
        implicits: &[ExpectedImplicit {
            family: "MaxPrefixMaxSuffix",
            level: 30,
            tags: &[],
            stats: &[
                ("local_maximum_prefixes_allowed_+", 1, 1),
                ("local_maximum_suffixes_allowed_+", -2, -2),
                ("local_implicit_mod_cannot_be_changed", 1, 1),
                ("local_suffix_mod_effect_+%", 0, 0),
                ("local_prefix_mod_effect_+%", 50, 50),
            ],
            lines: &[
                &["local_maximum_prefixes_allowed_+"],
                &["local_maximum_suffixes_allowed_+"],
                &[],
                &["local_prefix_mod_effect_+%"],
            ],
            negated: &[],
        }],
        extra_mods: &[],
    },
    ExpectedBase {
        id: "Metadata/Items/Rings/RingE5",
        name: "Ratcheting Ring",
        tags: &["not_for_sale", "experimental_base", "ring", "default"],
        implicits: &[ExpectedImplicit {
            family: "MaxPrefixMaxSuffix",
            level: 30,
            tags: &[],
            stats: &[
                ("local_maximum_prefixes_allowed_+", -3, -3),
                ("local_maximum_suffixes_allowed_+", 3, 3),
                ("local_implicit_mod_cannot_be_changed", 1, 1),
                ("local_suffix_mod_effect_+%", 0, 0),
                ("local_prefix_mod_effect_+%", 0, 0),
            ],
            lines: &[
                &["local_maximum_prefixes_allowed_+"],
                &["local_maximum_suffixes_allowed_+"],
                &[],
            ],
            negated: &[],
        }],
        extra_mods: &[],
    },
    ExpectedBase {
        id: "Metadata/Items/Rings/RingE6",
        name: "Helical Ring",
        tags: &["not_for_sale", "experimental_base", "ring", "default"],
        implicits: &[ExpectedImplicit {
            family: "MaxPrefixMaxSuffix",
            level: 30,
            tags: &[],
            stats: &[
                ("local_maximum_prefixes_allowed_+", -2, -2),
                ("local_maximum_suffixes_allowed_+", 1, 1),
                ("local_implicit_mod_cannot_be_changed", 1, 1),
                ("local_suffix_mod_effect_+%", 50, 50),
                ("local_prefix_mod_effect_+%", 0, 0),
            ],
            lines: &[
                &["local_maximum_prefixes_allowed_+"],
                &["local_maximum_suffixes_allowed_+"],
                &[],
                &["local_suffix_mod_effect_+%"],
            ],
            negated: &[],
        }],
        extra_mods: &[],
    },
];

/// The 23 minion mods that Bone Ring and Gloam Ring have in addition to the Iron Ring's pool.
const MINION_MODS: [ExtraMod; 23] = [
    ExtraMod {
        id: "MinionDamagePercent1",
        name: "Baroness's",
        affix_type: "prefix",
        family: "MinionDamage",
        level: 4,
        weight: 600,
        stat: ("minion_damage_+%", 5, 10),
    },
    ExtraMod {
        id: "MinionDamagePercent2",
        name: "Viscountess's",
        affix_type: "prefix",
        family: "MinionDamage",
        level: 15,
        weight: 600,
        stat: ("minion_damage_+%", 11, 20),
    },
    ExtraMod {
        id: "MinionDamagePercent3",
        name: "Marchioness's",
        affix_type: "prefix",
        family: "MinionDamage",
        level: 30,
        weight: 600,
        stat: ("minion_damage_+%", 21, 30),
    },
    ExtraMod {
        id: "MinionDamagePercent4",
        name: "Countess's",
        affix_type: "prefix",
        family: "MinionDamage",
        level: 60,
        weight: 600,
        stat: ("minion_damage_+%", 31, 36),
    },
    ExtraMod {
        id: "MinionDamagePercent5",
        name: "Duchess's",
        affix_type: "prefix",
        family: "MinionDamage",
        level: 81,
        weight: 600,
        stat: ("minion_damage_+%", 37, 42),
    },
    ExtraMod {
        id: "MinionAttackAndCastSpeed1",
        name: "of Motivation",
        affix_type: "suffix",
        family: "MinionAttackSpeedAndCastSpeed",
        level: 2,
        weight: 2000,
        stat: ("minion_attack_and_cast_speed_+%", 5, 7),
    },
    ExtraMod {
        id: "MinionAttackAndCastSpeed2",
        name: "of Coercion",
        affix_type: "suffix",
        family: "MinionAttackSpeedAndCastSpeed",
        level: 15,
        weight: 2000,
        stat: ("minion_attack_and_cast_speed_+%", 8, 10),
    },
    ExtraMod {
        id: "MinionAttackAndCastSpeed3_",
        name: "of Incitation",
        affix_type: "suffix",
        family: "MinionAttackSpeedAndCastSpeed",
        level: 30,
        weight: 2000,
        stat: ("minion_attack_and_cast_speed_+%", 11, 13),
    },
    ExtraMod {
        id: "MinionAttackAndCastSpeed4",
        name: "of Agitation",
        affix_type: "suffix",
        family: "MinionAttackSpeedAndCastSpeed",
        level: 40,
        weight: 2000,
        stat: ("minion_attack_and_cast_speed_+%", 14, 16),
    },
    ExtraMod {
        id: "MinionResistancesWeapon1_",
        name: "of Adjustment",
        affix_type: "suffix",
        family: "MinionElementalResistances",
        level: 8,
        weight: 2000,
        stat: ("minion_elemental_resistance_%", 11, 15),
    },
    ExtraMod {
        id: "MinionResistancesWeapon2",
        name: "of Acclimatisation",
        affix_type: "suffix",
        family: "MinionElementalResistances",
        level: 24,
        weight: 2000,
        stat: ("minion_elemental_resistance_%", 16, 20),
    },
    ExtraMod {
        id: "MinionResistancesWeapon3",
        name: "of Adaptation",
        affix_type: "suffix",
        family: "MinionElementalResistances",
        level: 37,
        weight: 2000,
        stat: ("minion_elemental_resistance_%", 21, 23),
    },
    ExtraMod {
        id: "MinionResistancesWeapon4",
        name: "of Evolution",
        affix_type: "suffix",
        family: "MinionElementalResistances",
        level: 50,
        weight: 2000,
        stat: ("minion_elemental_resistance_%", 24, 26),
    },
    ExtraMod {
        id: "MinionResistancesWeapon5",
        name: "of Metamorphosis",
        affix_type: "suffix",
        family: "MinionElementalResistances",
        level: 61,
        weight: 2000,
        stat: ("minion_elemental_resistance_%", 27, 30),
    },
    ExtraMod {
        id: "MinionLifeWeapon1",
        name: "of the Administrator",
        affix_type: "suffix",
        family: "MinionLife",
        level: 10,
        weight: 2000,
        stat: ("minion_maximum_life_+%", 13, 17),
    },
    ExtraMod {
        id: "MinionLifeWeapon2",
        name: "of the Rector",
        affix_type: "suffix",
        family: "MinionLife",
        level: 26,
        weight: 2000,
        stat: ("minion_maximum_life_+%", 18, 22),
    },
    ExtraMod {
        id: "MinionLifeWeapon3",
        name: "of the Overseer",
        affix_type: "suffix",
        family: "MinionLife",
        level: 42,
        weight: 2000,
        stat: ("minion_maximum_life_+%", 23, 27),
    },
    ExtraMod {
        id: "MinionLifeWeapon4__",
        name: "of the Taskmaster",
        affix_type: "suffix",
        family: "MinionLife",
        level: 58,
        weight: 2000,
        stat: ("minion_maximum_life_+%", 28, 32),
    },
    ExtraMod {
        id: "MinionMovementSpeed1",
        name: "of Coordination",
        affix_type: "suffix",
        family: "MinionMovementSpeed",
        level: 1,
        weight: 2000,
        stat: ("minion_movement_speed_+%", 6, 10),
    },
    ExtraMod {
        id: "MinionMovementSpeed2",
        name: "of Collaboration",
        affix_type: "suffix",
        family: "MinionMovementSpeed",
        level: 15,
        weight: 2000,
        stat: ("minion_movement_speed_+%", 11, 15),
    },
    ExtraMod {
        id: "MinionMovementSpeed3",
        name: "of Integration",
        affix_type: "suffix",
        family: "MinionMovementSpeed",
        level: 30,
        weight: 2000,
        stat: ("minion_movement_speed_+%", 16, 20),
    },
    ExtraMod {
        id: "MinionMovementSpeed4",
        name: "of Orchestration",
        affix_type: "suffix",
        family: "MinionMovementSpeed",
        level: 40,
        weight: 2000,
        stat: ("minion_movement_speed_+%", 21, 25),
    },
    ExtraMod {
        id: "MinionMovementSpeed5",
        name: "of Harmony",
        affix_type: "suffix",
        family: "MinionMovementSpeed",
        level: 55,
        weight: 2000,
        stat: ("minion_movement_speed_+%", 26, 30),
    },
];

/// The 3 mods for socketed gems that the Unset Ring has in addition to the Iron Ring's pool.
const UNSET_MODS: [ExtraMod; 3] = [
    ExtraMod {
        id: "LocalIncreaseSocketedGemUnsetRing1",
        name: "Exemplary",
        affix_type: "prefix",
        family: "IncreaseSocketedGemLevel",
        level: 2,
        weight: 1000,
        stat: ("local_socketed_gem_level_+", 1, 1),
    },
    ExtraMod {
        id: "LocalIncreaseSocketedGemUnsetRing2",
        name: "Quintessential",
        affix_type: "prefix",
        family: "IncreaseSocketedGemLevel",
        level: 50,
        weight: 1000,
        stat: ("local_socketed_gem_level_+", 2, 2),
    },
    ExtraMod {
        id: "LocalIncreaseSocketedGemUnsetRing3",
        name: "Flawless",
        affix_type: "prefix",
        family: "IncreaseSocketedGemLevel",
        level: 76,
        weight: 250,
        stat: ("local_socketed_gem_level_+", 3, 3),
    },
];

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

fn read_text(relative: &str) -> String {
    let path = repo_path(relative);
    let bytes = fs::read(&path).unwrap_or_else(|e| panic!("cannot read {relative}: {e}"));
    String::from_utf8_lossy(&bytes).into_owned()
}

fn load_json(relative: &str) -> Value {
    parse_json(&read_text(relative), relative)
}

fn parse_json(text: &str, what: &str) -> Value {
    serde_json::from_str(text).unwrap_or_else(|e| panic!("{what} is not valid JSON: {e}"))
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
    collect(&repo_path("web").join("src"), &mut files);
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
// Helpers: JSON
// ---------------------------------------------------------------------------------------

fn field<'a>(object: &'a Value, key: &str, what: &str) -> &'a Value {
    object
        .get(key)
        .unwrap_or_else(|| panic!("{what} must have the field {key:?}, found {object}"))
}

fn str_field<'a>(object: &'a Value, key: &str, what: &str) -> &'a str {
    field(object, key, what)
        .as_str()
        .unwrap_or_else(|| panic!("{what}.{key} must be a string"))
}

fn u64_field(object: &Value, key: &str, what: &str) -> u64 {
    field(object, key, what)
        .as_u64()
        .unwrap_or_else(|| panic!("{what}.{key} must be a whole number of at least 0"))
}

fn array_field<'a>(object: &'a Value, key: &str, what: &str) -> &'a Vec<Value> {
    field(object, key, what)
        .as_array()
        .unwrap_or_else(|| panic!("{what}.{key} must be an array"))
}

fn strings_field(object: &Value, key: &str, what: &str) -> Vec<String> {
    array_field(object, key, what)
        .iter()
        .map(|value| {
            value
                .as_str()
                .unwrap_or_else(|| panic!("{what}.{key} must hold strings"))
                .to_owned()
        })
        .collect()
}

fn string_set(values: &[&str]) -> BTreeSet<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

/// The entries of a JSON array field, by their `id`.
fn entries_by_id<'a>(file: &'a Value, key: &str, what: &str) -> BTreeMap<String, &'a Value> {
    let entries = array_field(file, key, what);
    let map: BTreeMap<String, &Value> = entries
        .iter()
        .map(|entry| (str_field(entry, "id", what).to_owned(), entry))
        .collect();
    assert_eq!(map.len(), entries.len(), "{what}: ids must be unique");
    map
}

/// The entry with the given id, from a map built by `entries_by_id`.
fn entry<'a>(entries: &BTreeMap<String, &'a Value>, id: &str, file: &str) -> &'a Value {
    entries
        .get(id)
        .copied()
        .unwrap_or_else(|| panic!("{file} must contain {id:?}"))
}

/// The `placeholders` object of a mod entry as a map from placeholder name to stat id.
fn placeholders_field(entry: &Value, what: &str) -> BTreeMap<String, String> {
    field(entry, "placeholders", what)
        .as_object()
        .unwrap_or_else(|| panic!("{what}.placeholders must be an object"))
        .iter()
        .map(|(name, stat)| {
            let stat = stat
                .as_str()
                .unwrap_or_else(|| panic!("{what}.placeholders.{name} must be a string"));
            (name.clone(), stat.to_owned())
        })
        .collect()
}

/// The `stats` array of a mod entry as a map from stat id to (min, max).
fn stats_field(entry: &Value, what: &str) -> BTreeMap<String, (i64, i64)> {
    let stats = array_field(entry, "stats", what);
    let map: BTreeMap<String, (i64, i64)> = stats
        .iter()
        .map(|stat| {
            let id = str_field(stat, "id", what).to_owned();
            let bound = |key: &str| {
                field(stat, key, what)
                    .as_i64()
                    .unwrap_or_else(|| panic!("{what}: {id}.{key} must be a whole number"))
            };
            (id.clone(), (bound("min"), bound("max")))
        })
        .collect();
    assert_eq!(map.len(), stats.len(), "{what}: stat ids must be unique");
    map
}

/// The `conversions` object of a mod entry as a map from stat id to (divisor, precision).
/// A missing object counts as empty, a missing or null precision as none.
fn conversions_field(entry: &Value, what: &str) -> BTreeMap<String, (i64, Option<u64>)> {
    let Some(conversions) = entry.get("conversions") else {
        return BTreeMap::new();
    };
    conversions
        .as_object()
        .unwrap_or_else(|| panic!("{what}.conversions must be an object"))
        .iter()
        .map(|(stat, conversion)| {
            let divisor = field(conversion, "divisor", what)
                .as_i64()
                .unwrap_or_else(|| {
                    panic!("{what}: the divisor of {stat:?} must be a whole number")
                });
            let precision = match conversion.get("precision") {
                None | Some(Value::Null) => None,
                Some(value) => Some(value.as_u64().unwrap_or_else(|| {
                    panic!("{what}: the precision of {stat:?} must be a whole number or null")
                })),
            };
            (stat.clone(), (divisor, precision))
        })
        .collect()
}

// ---------------------------------------------------------------------------------------
// Helpers: texts and the PO's display rules
// ---------------------------------------------------------------------------------------

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

fn expected_lines(lines: &[&[&str]]) -> Vec<Vec<String>> {
    lines
        .iter()
        .map(|line| line.iter().map(|stat| (*stat).to_owned()).collect())
        .collect()
}

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

/// The text of a converted entry with every placeholder written as `{<stat id>}`.
fn canonical_entry_text(entry: &Value, what: &str) -> String {
    let placeholders = placeholders_field(entry, what);
    render(str_field(entry, "text", what), &placeholders, |stat| {
        format!("{{{stat}}}")
    })
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

/// Rolled value of the stat with the given identifier.
fn stat_value(rolled: &Mod, id: &str) -> i32 {
    rolled
        .stats
        .iter()
        .find(|stat| stat.id == id)
        .unwrap_or_else(|| panic!("the mod must have a stat {id:?}, found {:?}", rolled.stats))
        .value
}

/// The text the PO's rules expect for a rolled mod.
fn expected_display_text(rolled: &Mod) -> String {
    render(&rolled.text, &rolled.placeholders, |stat| {
        shown_value(stat_value(rolled, stat), rolled.conversions.get(stat))
    })
}

// ---------------------------------------------------------------------------------------
// Helpers: expected values
// ---------------------------------------------------------------------------------------

fn expected_base(id: &str) -> &'static ExpectedBase {
    BASES
        .iter()
        .find(|base| base.id == id)
        .unwrap_or_else(|| panic!("the test expects no base {id:?}"))
}

fn expected_stats(expected: &ExpectedImplicit) -> BTreeMap<String, (i64, i64)> {
    expected
        .stats
        .iter()
        .map(|(id, min, max)| ((*id).to_owned(), (i64::from(*min), i64::from(*max))))
        .collect()
}

fn negated_conversions(stats: &[&str]) -> BTreeMap<String, Conversion> {
    stats
        .iter()
        .map(|stat| {
            (
                (*stat).to_owned(),
                Conversion {
                    divisor: NEGATE_DIVISOR,
                    precision: None,
                },
            )
        })
        .collect()
}

fn affix_type_name(affix_type: AffixType) -> &'static str {
    match affix_type {
        AffixType::Prefix => "prefix",
        AffixType::Suffix => "suffix",
    }
}

/// Checks an implicit of the data file against the expected values.
fn assert_data_implicit(entry: &Value, expected: &ExpectedImplicit, what: &str) {
    assert_eq!(
        str_field(entry, "family", what),
        expected.family,
        "{what}: family"
    );
    assert_eq!(
        u64_field(entry, "level", what),
        u64::from(expected.level),
        "{what}: level"
    );
    assert_eq!(
        strings_field(entry, "tags", what),
        expected
            .tags
            .iter()
            .map(|tag| (*tag).to_owned())
            .collect::<Vec<_>>(),
        "{what}: tags, unchanged from RePoE"
    );
    assert_eq!(
        stats_field(entry, what),
        expected_stats(expected),
        "{what}: stats"
    );
    let placeholders = placeholders_field(entry, what);
    let lines = line_stats(str_field(entry, "text", what), &placeholders);
    assert_eq!(
        lines,
        expected_lines(expected.lines),
        "{what}: the stats shown on each line, in the game's order"
    );
    let named: BTreeSet<&String> = placeholders.values().collect();
    let shown: BTreeSet<&String> = lines.iter().flatten().collect();
    assert_eq!(
        named, shown,
        "{what}: the placeholders name the stats the text shows"
    );
    let expected_conversions: BTreeMap<String, (i64, Option<u64>)> = expected
        .negated
        .iter()
        .map(|stat| ((*stat).to_owned(), (i64::from(NEGATE_DIVISOR), None)))
        .collect();
    assert_eq!(
        conversions_field(entry, what),
        expected_conversions,
        "{what}: conversions"
    );
}

/// Checks an implicit of crafting-core against the expected values.
fn assert_core_implicit(definition: &ModDefinition, expected: &ExpectedImplicit, what: &str) {
    assert_eq!(definition.family, expected.family, "{what}: family");
    assert_eq!(definition.level, expected.level, "{what}: level");
    assert_eq!(
        definition.tags,
        expected
            .tags
            .iter()
            .map(|tag| (*tag).to_owned())
            .collect::<Vec<_>>(),
        "{what}: tags"
    );
    let stats: BTreeMap<String, (i64, i64)> = definition
        .stats
        .iter()
        .map(|stat| (stat.id.clone(), (i64::from(stat.min), i64::from(stat.max))))
        .collect();
    assert_eq!(
        stats.len(),
        definition.stats.len(),
        "{what}: stat ids are unique"
    );
    assert_eq!(stats, expected_stats(expected), "{what}: stats");
    let lines = line_stats(&definition.text, &definition.placeholders);
    assert_eq!(
        lines,
        expected_lines(expected.lines),
        "{what}: the stats shown on each line, in the game's order"
    );
    let named: BTreeSet<&String> = definition.placeholders.values().collect();
    let shown: BTreeSet<&String> = lines.iter().flatten().collect();
    assert_eq!(
        named, shown,
        "{what}: the placeholders name the stats the text shows"
    );
    assert_eq!(
        definition.conversions,
        negated_conversions(expected.negated),
        "{what}: conversions"
    );
}

/// An invented mod definition with one stat.
fn one_stat_mod(stat: &str, text: &str) -> ModDefinition {
    ModDefinition {
        family: format!("{stat} family"),
        level: 1,
        tags: vec!["test".to_owned()],
        stats: vec![StatDefinition {
            id: stat.to_owned(),
            min: 1,
            max: 1,
        }],
        text: text.to_owned(),
        placeholders: BTreeMap::from([("value".to_owned(), stat.to_owned())]),
        conversions: BTreeMap::new(),
    }
}

/// A copy of `definition` whose stats all roll the fixed value `value`.
fn fixed(definition: &ModDefinition, value: i32) -> ModDefinition {
    let mut copy = definition.clone();
    for stat in &mut copy.stats {
        stat.min = value;
        stat.max = value;
    }
    copy
}

// ---------------------------------------------------------------------------------------
// Helpers: invented raw files in RePoE's format (AC1, AC5)
// ---------------------------------------------------------------------------------------

/// A ring whose implicit shows its left line through the handler `negate`, like the Dusk Ring.
const TEST_DUSK_RING: &str = "Metadata/Items/Test/TestDuskRing";
const TEST_DUSK_IMPLICIT: &str = "TestReflectedDuration1";
const TEST_LEFT_STAT: &str = "test_local_left_ring_slot_duration_+%";
const TEST_RIGHT_STAT: &str = "test_local_right_ring_slot_duration_+%";
/// A ring whose implicit text does not show its value (format `ignore`), like the Unset Ring.
const TEST_UNSET_RING: &str = "Metadata/Items/Test/TestUnsetRing";
const TEST_SOCKETS_STAT: &str = "test_local_has_X_sockets";

/// Index of the left stat's entry in the invented `stat_translations.json`; its second
/// translation is the negated one.
const LEFT_ENTRY: &str = "0";

/// The six raw files as JSON values, so that test cases can change single values.
#[derive(Clone)]
struct RawSet {
    base_items: Value,
    mods: Value,
    mods_by_base: Value,
    stat_translations: Value,
    stat_value_handlers: Value,
    item_classes: Value,
}

impl RawSet {
    fn files(&self) -> RawFiles {
        RawFiles {
            base_items: self.base_items.to_string(),
            mods: self.mods.to_string(),
            mods_by_base: self.mods_by_base.to_string(),
            stat_translations: self.stat_translations.to_string(),
            stat_value_handlers: self.stat_value_handlers.to_string(),
            item_classes: self.item_classes.to_string(),
        }
    }
}

/// The value at `path` (object keys or array indices) inside `value`.
fn at<'a>(value: &'a mut Value, path: &[&str]) -> &'a mut Value {
    path.iter().fold(value, |current, key| {
        let next = match current {
            Value::Array(items) => {
                let index: usize = key
                    .parse()
                    .unwrap_or_else(|_| panic!("{key:?} is not an array index"));
                items.get_mut(index)
            }
            Value::Object(map) => map.get_mut(*key),
            _ => None,
        };
        next.unwrap_or_else(|| panic!("the invented raw files have no {path:?}"))
    })
}

fn null_properties() -> Value {
    let keys = [
        "armour",
        "energy_shield",
        "evasion",
        "ward",
        "movement_speed",
        "block",
        "description",
        "directions",
        "stack_size",
        "stack_size_currency_tab",
        "full_stack_turns_into",
        "charges_max",
        "charges_per_use",
        "duration",
        "life_per_use",
        "mana_per_use",
        "attack_time",
        "critical_strike_chance",
        "physical_damage_max",
        "physical_damage_min",
        "range",
        "mana_burn_ms",
        "cooldown_ms",
        "monster_id",
        "monster_ability_text",
        "monster_category",
    ];
    Value::Object(
        keys.iter()
            .map(|key| ((*key).to_owned(), Value::Null))
            .collect(),
    )
}

fn raw_base(name: &str, implicits: &[&str]) -> Value {
    json!({
        "domain": "item",
        "drop_level": 30,
        "implicits": implicits,
        "inventory_height": 1,
        "inventory_width": 1,
        "inherits_from": "Metadata/Items/Rings/AbstractRing",
        "item_class": "TestRing",
        "name": name,
        "properties": null_properties(),
        "release_state": "released",
        "tags": ["testring", "default"],
        "visual_identity": { "dds_file": "Art/2DItems/Test.dds", "id": "Test" },
        "requirements": null,
        "grants_buff": null,
        "skills_granted": null
    })
}

fn raw_mod(
    name: &str,
    generation_type: &str,
    group: &str,
    level: u32,
    tags: &[&str],
    stats: Value,
    text: &str,
) -> Value {
    json!({
        "adds_tags": [],
        "domain": "item",
        "generation_type": generation_type,
        "generation_weights": [],
        "grants_effects": [],
        "groups": [group],
        "implicit_tags": tags,
        "is_essence_only": false,
        "name": name,
        "required_level": level,
        "spawn_weights": [
            { "tag": "testring", "weight": 1000 },
            { "tag": "default", "weight": 0 }
        ],
        "stats": stats,
        "text": text,
        "type": group,
        "gold_value": null
    })
}

/// One condition of a translation entry.
fn condition(min: Option<i32>, max: Option<i32>) -> Value {
    json!({ "min": min, "max": max, "negated": null })
}

/// One English translation of an entry with one stat.
fn translation(condition: Value, format: &str, handlers: &[&str], string: &str) -> Value {
    json!({
        "condition": [condition],
        "format": [format],
        "index_handlers": [handlers],
        "string": string,
        "reminder_text": null,
        "is_markup": null
    })
}

fn translation_entry(id: &str, english: Vec<Value>) -> Value {
    json!({
        "English": english,
        "ids": [id],
        "trade_stats": [],
        "hidden": null,
        "French": null,
        "German": null,
        "Japanese": null,
        "Korean": null,
        "Portuguese": null,
        "Russian": null,
        "Spanish": null,
        "Thai": null,
        "Traditional Chinese": null
    })
}

/// A value handler of `stat_value_handlers.json`.
fn handler(divisor: Option<f64>, multiplier: Option<f64>, precision: Option<u32>) -> Value {
    json!({
        "addend": null,
        "divisor": divisor,
        "multiplier": multiplier,
        "precision": precision,
        "fixed": null,
        "type": "int"
    })
}

/// Small invented raw files in RePoE's format: a ring with a two-line implicit whose left line
/// the game data shows through `negate` (raw -15, shown as "15% reduced"), and a ring whose
/// implicit text shows no value (format `ignore`). Both share a pool of one prefix and one
/// suffix.
fn invented_raw_files() -> RawSet {
    let base_items = json!({
        TEST_DUSK_RING: raw_base("Test Dusk Ring", &[TEST_DUSK_IMPLICIT]),
        TEST_UNSET_RING: raw_base("Test Unset Ring", &["TestHasOneSocket"]),
    });
    let mods = json!({
        TEST_DUSK_IMPLICIT: raw_mod("", "unique", "TestReflectedDuration", 30, &[],
            json!([
                { "id": TEST_LEFT_STAT, "max": -15, "min": -15 },
                { "id": TEST_RIGHT_STAT, "max": 15, "min": 15 }
            ]),
            "Left test slot: 15% reduced Test Duration\nRight test slot: 15% increased Test Duration"),
        "TestHasOneSocket": raw_mod("", "unique", "TestOther", 7, &[],
            json!([{ "id": TEST_SOCKETS_STAT, "max": 1, "min": 1 }]),
            "Has 1 Test Socket"),
        "TestLife1": raw_mod("Testy", "prefix", "TestLife", 1, &["resource", "life"],
            json!([{ "id": "test_maximum_life", "max": 9, "min": 3 }]),
            "+(3-9) to maximum Test Life"),
        "TestStrength1": raw_mod("of the Test", "suffix", "TestStrength", 1, &["attribute"],
            json!([{ "id": "test_strength", "max": 12, "min": 8 }]),
            "+(8-12) to Test Strength"),
    });
    let mods_by_base = json!({
        "Test Rings": {
            "testring,default": {
                "bases": [TEST_DUSK_RING, TEST_UNSET_RING],
                "mods": {
                    "prefix": { "TestLife": { "TestLife1": 1000 } },
                    "suffix": { "TestStrength": { "TestStrength1": 1000 } }
                },
                "conditional_mods": null
            }
        }
    });
    let stat_translations = json!([
        translation_entry(
            TEST_LEFT_STAT,
            vec![
                translation(
                    condition(Some(1), None),
                    "#",
                    &[],
                    "Left test slot: {0}% increased Test Duration"
                ),
                translation(
                    condition(None, Some(-1)),
                    "#",
                    &["negate"],
                    "Left test slot: {0}% reduced Test Duration"
                ),
            ]
        ),
        translation_entry(
            TEST_RIGHT_STAT,
            vec![
                translation(
                    condition(Some(1), None),
                    "#",
                    &[],
                    "Right test slot: {0}% increased Test Duration"
                ),
                translation(
                    condition(None, Some(-1)),
                    "#",
                    &["negate"],
                    "Right test slot: {0}% reduced Test Duration"
                ),
            ]
        ),
        translation_entry(
            TEST_SOCKETS_STAT,
            vec![
                translation(
                    condition(Some(1), Some(1)),
                    "ignore",
                    &[],
                    "Has 1 Test Socket"
                ),
                translation(condition(Some(2), None), "#", &[], "Has {0} Test Sockets"),
            ]
        ),
        translation_entry(
            "test_maximum_life",
            vec![translation(
                condition(Some(1), None),
                "+#",
                &[],
                "{0} to maximum Test Life"
            ),]
        ),
        translation_entry(
            "test_strength",
            vec![translation(
                condition(Some(1), None),
                "+#",
                &[],
                "{0} to Test Strength"
            ),]
        ),
    ]);
    let stat_value_handlers = json!({
        "tq_noop": { "type": "noop" },
        "negate": handler(None, Some(-1.0), None),
        "negate_and_double": handler(None, Some(-2.0), None),
    });
    let item_classes = json!({
        "TestRing": { "category": "Ring", "category_id": "Ring", "name": "Test Rings",
            "influence_tags": [] },
    });
    RawSet {
        base_items,
        mods,
        mods_by_base,
        stat_translations,
        stat_value_handlers,
        item_classes,
    }
}

/// The only implicit of a converted base.
fn converted_implicit(base_items_json: &str, base_id: &str) -> Value {
    let base_items = parse_json(base_items_json, "the converted base_items.json");
    let bases = entries_by_id(&base_items, "base_items", "the converted base_items.json");
    let base = bases
        .get(base_id)
        .unwrap_or_else(|| panic!("the converted base_items.json must contain {base_id}"));
    let implicits = array_field(base, "implicits", base_id);
    assert_eq!(implicits.len(), 1, "{base_id}: one implicit");
    implicits[0].clone()
}

/// Runs the converter and expects an error message that names at least one of `hints`.
fn assert_conversion_fails(label: &str, raw: &RawFiles, base_ids: &[&str], hints: &[&str]) {
    let result = std::panic::catch_unwind(|| convert(raw, base_ids));
    let result = result.unwrap_or_else(|_| {
        panic!("{label}: the converter must stop with a message, not with a panic")
    });
    match result {
        Ok(_) => panic!("{label}: the converter must stop with a message, but it converted"),
        Err(message) => {
            assert!(
                !message.trim().is_empty(),
                "{label}: the converter must give a message"
            );
            assert!(
                hints.iter().any(|hint| message.contains(hint)),
                "{label}: the message {message:?} must name one of {hints:?}"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------------------

// AC1: „`data/` enthält die Ring-Basen nach den Entscheidungen des PO, jede mit ihren
// Implicits und ihrem Mod-Pool aus RePoE, erzeugt mit dem Konverter. `data/README.md` ist
// aktualisiert."
// (The PO's decisions give 43 bases (ADR 0017, ADR 0020, ADR 0022). The data files are checked
// against values taken from the raw files; that they are the converter's output is checked by
// the developer's second run without a diff. Implicits whose text shows no value (format
// `ignore`, Breach Ring, Heist rings and Unset Ring) keep their stat without a placeholder
// (ADR 0017), and so does a Heist ring's magnitude of 0 %, which has no line (ADR 0022); the
// converter's handling of them is checked with invented raw files. The update of
// `data/README.md` is wording and is checked in the review.)
#[test]
fn ac1_data_holds_the_selected_ring_bases_with_their_implicits_and_pools() {
    // Given the data files and crafting-core
    let base_items_file = load_json(BASE_ITEMS_JSON);
    let mods_file = load_json(MODS_JSON);
    let bases = entries_by_id(&base_items_file, "base_items", BASE_ITEMS_JSON);
    let mod_entries = entries_by_id(&mods_file, "mods", MODS_JSON);
    let expected_ids: BTreeSet<String> = BASES.iter().map(|base| base.id.to_owned()).collect();
    assert_eq!(
        expected_ids.len(),
        BASES.len(),
        "the test lists each base once"
    );
    let mut rng = SeededRng::new(SEED);

    // Then the data file holds exactly the ring bases of the PO's decisions
    assert_eq!(
        bases.keys().cloned().collect::<BTreeSet<_>>(),
        expected_ids,
        "{BASE_ITEMS_JSON}: the bases"
    );
    // And crafting-core offers exactly these bases
    let core_ids: BTreeSet<String> = base_items().into_iter().map(|base| base.id).collect();
    assert_eq!(core_ids, expected_ids, "base_items(): the bases");

    let mut all_pool_ids: BTreeSet<String> = BTreeSet::new();
    for expected in &BASES {
        let entry = bases[expected.id];
        let what = format!("{BASE_ITEMS_JSON}: {:?}", expected.id);
        // And each base has its name, the item class, no own level requirement and its tags
        assert_eq!(
            str_field(entry, "name", &what),
            expected.name,
            "{what}: name"
        );
        assert_eq!(
            str_field(entry, "item_class", &what),
            RINGS_CLASS,
            "{what}: item class"
        );
        assert_eq!(
            u64_field(entry, "required_level", &what),
            0,
            "{what}: a ring has no own level requirement"
        );
        assert_eq!(
            strings_field(entry, "tags", &what)
                .into_iter()
                .collect::<BTreeSet<_>>(),
            string_set(expected.tags),
            "{what}: tags"
        );
        // And its implicits from RePoE
        let implicits = array_field(entry, "implicits", &what);
        assert_eq!(
            implicits.len(),
            expected.implicits.len(),
            "{what}: number of implicits"
        );
        for (implicit, expected_implicit) in implicits.iter().zip(expected.implicits) {
            assert_data_implicit(implicit, expected_implicit, &format!("{what}, implicit"));
        }
        // And its pool: each mod once, each in mods.json, the size of the Iron Ring's pool plus
        // its additional mods (their ids are checked in AC3)
        let pool = strings_field(entry, "mod_pool", &what);
        let unique: BTreeSet<&String> = pool.iter().collect();
        assert_eq!(
            unique.len(),
            pool.len(),
            "{what}: the pool lists each mod once"
        );
        assert_eq!(
            pool.len(),
            IRON_POOL_SIZE + expected.extra_mods.len(),
            "{what}: size of the pool"
        );
        for id in &pool {
            assert!(
                mod_entries.contains_key(id),
                "{MODS_JSON} must contain the pool mod {id:?} of {:?}",
                expected.id
            );
        }
        all_pool_ids.extend(pool);

        // And crafting-core offers the same base
        let base = base_item(expected.id)
            .unwrap_or_else(|| panic!("crafting-core must offer the base {:?}", expected.id));
        let what = format!("base_item({:?})", expected.id);
        assert_eq!(base.name, expected.name, "{what}: name");
        assert_eq!(base.item_class, RINGS_CLASS, "{what}: item class");
        assert_eq!(base.required_level, 0, "{what}: no own level requirement");
        assert_eq!(
            base.tags.iter().cloned().collect::<BTreeSet<_>>(),
            string_set(expected.tags),
            "{what}: tags"
        );
        assert_eq!(
            base.implicits.len(),
            expected.implicits.len(),
            "{what}: number of implicits"
        );
        for (implicit, expected_implicit) in base.implicits.iter().zip(expected.implicits) {
            assert_core_implicit(implicit, expected_implicit, &format!("{what}, implicit"));
        }
        // When normal items of the base are created
        for _ in 0..ROLLS {
            let item = Item::new(&base, &mut rng);
            // Then each implicit shows its rolled values by the rules, also one whose text
            // shows no value
            assert_eq!(item.implicits.len(), expected.implicits.len(), "{what}");
            for rolled in &item.implicits {
                assert_eq!(
                    rolled.display_text(),
                    expected_display_text(rolled),
                    "{what}: implicit text"
                );
            }
        }
    }

    // And mods.json holds exactly the mods of all pools
    assert_eq!(
        mod_entries.keys().cloned().collect::<BTreeSet<_>>(),
        all_pool_ids,
        "{MODS_JSON} holds the mods of the pools and no other"
    );
    assert_eq!(mod_entries.len(), ALL_MODS, "{MODS_JSON}: number of mods");
    assert_eq!(
        explicit_mods().len(),
        ALL_MODS,
        "explicit_mods() offers every mod of {MODS_JSON}"
    );

    // Given invented raw files with an implicit whose text shows no value (format ignore)
    let raw = invented_raw_files();
    // When the converter runs
    let converted = convert(&raw.files(), &[TEST_DUSK_RING, TEST_UNSET_RING])
        .expect("the invented raw files convert");
    // Then the implicit keeps its stat with the raw range, with a text without a placeholder
    // and without a conversion
    let implicit = converted_implicit(&converted.base_items_json, TEST_UNSET_RING);
    let what = "converted implicit with the format ignore";
    assert_eq!(
        stats_field(&implicit, what),
        BTreeMap::from([(TEST_SOCKETS_STAT.to_owned(), (1, 1))]),
        "{what}: the stat stays"
    );
    assert_eq!(
        line_stats(
            str_field(&implicit, "text", what),
            &placeholders_field(&implicit, what)
        ),
        vec![Vec::<String>::new()],
        "{what}: one line without a placeholder"
    );
    assert!(
        !placeholders_field(&implicit, what)
            .values()
            .any(|stat| stat == TEST_SOCKETS_STAT),
        "{what}: no placeholder names the stat"
    );
    assert!(
        conversions_field(&implicit, what).is_empty(),
        "{what}: no conversion"
    );
}

// AC2: „Royale-Varianten, Breach-Ringe und Heist-Ringe aus der Description sind nicht
// enthalten."
// (Tech lead's plan: the content of `data/base_items.json` and `data/mods.json`, checked by the
// base ids, because the Royale variants carry the names of included rings. crafting-core
// offers neither the bases nor a pool for them.)
#[test]
fn ac2_royale_variants_breach_rings_and_heist_rings_are_not_included() {
    // Given the data files and the bases of crafting-core
    let base_items_text = read_text(BASE_ITEMS_JSON);
    let mods_text = read_text(MODS_JSON);
    let core_ids: BTreeSet<String> = base_items().into_iter().map(|base| base.id).collect();

    for id in EXCLUDED_BASES {
        // When looking for an excluded base by its id
        let quoted = format!("\"{id}\"");
        // Then neither data file names it
        assert!(
            !base_items_text.contains(&quoted),
            "{BASE_ITEMS_JSON} must not contain {id}"
        );
        assert!(
            !mods_text.contains(&quoted),
            "{MODS_JSON} must not contain {id}"
        );
        // And crafting-core offers neither the base nor a pool for it
        assert!(!core_ids.contains(id), "base_items() must not offer {id}");
        assert!(base_item(id).is_none(), "base_item({id:?}) must be None");
        assert!(mod_pool(id).is_none(), "mod_pool({id:?}) must be None");
    }
    // And the excluded bases are none of the included ones
    for id in EXCLUDED_BASES {
        assert!(
            BASES.iter().all(|base| base.id != id),
            "the test must not expect {id}"
        );
    }
}

// AC3: „Bone Ring und Gloam Ring enthalten ihre zusätzlichen Minion-Mods, der Unset Ring seine
// zusätzlichen Mods. Die anderen Ringe haben den Pool des Iron Rings."
// (The Iron Ring's pool itself is checked against RePoE by the tests of AB#167. Here: every
// base's pool, in the data file and from `mod_pool`, is the Iron Ring's pool plus the base's
// additional mods from RePoE, with the same weights; the additional mods have their values
// from RePoE.)
#[test]
fn ac3_bone_gloam_and_unset_rings_add_their_mods_and_the_others_have_the_iron_ring_pool() {
    // Given the data files, the Iron Ring's pool and all mods of crafting-core
    let base_items_file = load_json(BASE_ITEMS_JSON);
    let mods_file = load_json(MODS_JSON);
    let bases = entries_by_id(&base_items_file, "base_items", BASE_ITEMS_JSON);
    let mod_entries = entries_by_id(&mods_file, "mods", MODS_JSON);
    let iron_ring = bases
        .get(IRON_RING_ID)
        .unwrap_or_else(|| panic!("{BASE_ITEMS_JSON} must contain {IRON_RING_ID}"));
    let iron_pool: BTreeSet<String> = strings_field(iron_ring, "mod_pool", BASE_ITEMS_JSON)
        .into_iter()
        .collect();
    assert_eq!(iron_pool.len(), IRON_POOL_SIZE, "the Iron Ring's pool");
    let all_mods: BTreeMap<String, ExplicitModDefinition> = explicit_mods()
        .into_iter()
        .map(|definition| (definition.name.clone(), definition))
        .collect();

    // Then the additional mods are not in the Iron Ring's pool and have their values from RePoE
    for extra in MINION_MODS.iter().chain(UNSET_MODS.iter()) {
        let what = format!("{MODS_JSON}: {:?}", extra.id);
        assert!(
            !iron_pool.contains(extra.id),
            "{what} must not be in the Iron Ring's pool"
        );
        let entry = mod_entries
            .get(extra.id)
            .unwrap_or_else(|| panic!("{MODS_JSON} must contain {:?}", extra.id));
        let (stat, min, max) = extra.stat;
        assert_eq!(str_field(entry, "name", &what), extra.name, "{what}: name");
        assert_eq!(
            str_field(entry, "affix_type", &what),
            extra.affix_type,
            "{what}: affix type"
        );
        assert_eq!(
            str_field(entry, "family", &what),
            extra.family,
            "{what}: family"
        );
        assert_eq!(
            u64_field(entry, "level", &what),
            u64::from(extra.level),
            "{what}: level"
        );
        assert_eq!(
            u64_field(entry, "weight", &what),
            u64::from(extra.weight),
            "{what}: weight"
        );
        assert_eq!(
            stats_field(entry, &what),
            BTreeMap::from([(stat.to_owned(), (i64::from(min), i64::from(max)))]),
            "{what}: stats"
        );
        assert_eq!(
            line_stats(
                str_field(entry, "text", &what),
                &placeholders_field(entry, &what)
            ),
            vec![vec![stat.to_owned()]],
            "{what}: one line that shows its stat"
        );
        assert!(
            conversions_field(entry, &what).is_empty(),
            "{what}: shown as rolled"
        );
        // And crafting-core offers it with the same values
        let definition = all_mods
            .get(extra.name)
            .unwrap_or_else(|| panic!("explicit_mods() must offer {:?}", extra.name));
        let what = format!("explicit_mods(): {:?}", extra.name);
        assert_eq!(
            affix_type_name(definition.affix_type),
            extra.affix_type,
            "{what}"
        );
        assert_eq!(definition.weight, extra.weight, "{what}");
        assert_eq!(definition.definition.level, extra.level, "{what}");
        assert_eq!(definition.definition.family, extra.family, "{what}");
        assert_eq!(
            definition
                .definition
                .stats
                .iter()
                .map(|s| (s.id.as_str(), s.min, s.max))
                .collect::<Vec<_>>(),
            vec![extra.stat],
            "{what}"
        );
        assert!(definition.definition.conversions.is_empty(), "{what}");
    }

    for expected in &BASES {
        // When reading the base's pool from the data file and from crafting-core
        let what = format!("the pool of {:?} ({})", expected.id, expected.name);
        let file_pool = strings_field(
            entry(&bases, expected.id, BASE_ITEMS_JSON),
            "mod_pool",
            BASE_ITEMS_JSON,
        );
        let core_pool = mod_pool(expected.id)
            .unwrap_or_else(|| panic!("mod_pool({:?}) must offer the base's pool", expected.id));

        // Then it is the Iron Ring's pool plus the base's additional mods
        let mut expected_pool = iron_pool.clone();
        expected_pool.extend(expected.extra_mods.iter().map(|extra| extra.id.to_owned()));
        assert_eq!(
            file_pool.iter().cloned().collect::<BTreeSet<_>>(),
            expected_pool,
            "{what} in {BASE_ITEMS_JSON}"
        );
        // And crafting-core offers the mods of that pool in its order
        let file_names: Vec<&str> = file_pool
            .iter()
            .map(|id| str_field(entry(&mod_entries, id, MODS_JSON), "name", MODS_JSON))
            .collect();
        let core_names: Vec<&str> = core_pool.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(core_names, file_names, "{what} from mod_pool()");
        // And with the values of the data file, so a mod has the same weight in every pool
        for definition in &core_pool {
            assert_eq!(
                Some(definition),
                all_mods.get(&definition.name),
                "{what}: {:?} as in explicit_mods()",
                definition.name
            );
        }
    }
}

// AC4: „Die drei Two-Stone Rings und die drei Shadowed Rings sind getrennte Basen mit ihrem
// jeweiligen Implicit."
// (Tech lead's plan: the bases are told apart by their ids, Ring12/13/14 and RingK5a/b/c.)
#[test]
fn ac4_two_stone_and_shadowed_rings_are_separate_bases_with_their_own_implicit() {
    let mut rng = SeededRng::new(SEED);
    // Given the data file and the bases of crafting-core
    let base_items_file = load_json(BASE_ITEMS_JSON);
    let file_bases = array_field(&base_items_file, "base_items", BASE_ITEMS_JSON);
    let core_bases = base_items();

    for (name, ids) in [TWO_STONE_RING, SHADOWED_RING] {
        // When looking for the bases with this name
        let expected_ids = string_set(&ids);
        let in_core: BTreeSet<String> = core_bases
            .iter()
            .filter(|base| base.name == name)
            .map(|base| base.id.clone())
            .collect();
        let in_file: BTreeSet<String> = file_bases
            .iter()
            .filter(|base| str_field(base, "name", BASE_ITEMS_JSON) == name)
            .map(|base| str_field(base, "id", BASE_ITEMS_JSON).to_owned())
            .collect();

        // Then there are three separate bases, each with its own id
        assert_eq!(
            in_core, expected_ids,
            "base_items(): the bases named {name:?}"
        );
        assert_eq!(
            in_file, expected_ids,
            "{BASE_ITEMS_JSON}: the bases named {name:?}"
        );

        let mut implicit_stats: BTreeSet<Vec<String>> = BTreeSet::new();
        for id in ids {
            let expected = expected_base(id);
            let base = base_item(id).unwrap_or_else(|| panic!("crafting-core must offer {id}"));
            let what = format!("{name} {id}");
            // And each has its own implicit from RePoE
            assert_eq!(base.name, name, "{what}");
            assert_eq!(base.implicits.len(), 1, "{what}: one implicit");
            assert_core_implicit(&base.implicits[0], &expected.implicits[0], &what);
            let mut stats: Vec<String> = base.implicits[0]
                .stats
                .iter()
                .map(|stat| stat.id.clone())
                .collect();
            stats.sort();
            // And the implicits of the three bases differ
            assert!(
                implicit_stats.insert(stats),
                "{what}: the implicit must differ from those of the other {name}s"
            );
            // When a normal item of the base is created
            let item = Item::new(&base, &mut rng);
            // Then it has the base's implicit
            assert_eq!(item.base.id, id, "{what}");
            assert_eq!(item.implicits.len(), 1, "{what}");
            for (stat_id, min, max) in expected.implicits[0].stats {
                let value = stat_value(&item.implicits[0], stat_id);
                assert!(
                    (*min..=*max).contains(&value),
                    "{what}: {stat_id} rolled {value}, outside {min}..={max}"
                );
            }
            // And the base has its own pool
            assert!(
                mod_pool(id).is_some_and(|pool| !pool.is_empty()),
                "{what}: mod_pool() must offer the base's pool"
            );
        }
    }
}

// AC5: „Wo die Spieldaten für die Anzeige die Umrechnung "negate" vorgeben, erscheint der Wert
// mit umgekehrtem Vorzeichen in der Formulierung der Spieldaten (z. B. Dusk Ring: "Left ring
// slot: 15% reduced Skill Effect Duration")."
// (ADR 0017: negation is the divisor -1 with the handler's precision (none for `negate`); the
// display divides by the divisor, so the rounding rules of "Item basics" apply unchanged.
// The wording is the translation that the converter chose for the raw range. A handler with
// another multiplier still stops the converter (ADR 0016, ADR 0017).)
#[test]
fn ac5_negated_values_appear_with_reversed_sign_in_the_wording_of_the_game_data() {
    let mut rng = SeededRng::new(SEED);

    // --- The rule with invented mods ---
    // Given a stat that is converted with the divisor -1, without and with a precision
    for precision in [None, Some(1)] {
        let mut negated = one_stat_mod("invented_negated", "{value}% reduced Invented Duration");
        negated.conversions = BTreeMap::from([(
            "invented_negated".to_owned(),
            Conversion {
                divisor: NEGATE_DIVISOR,
                precision,
            },
        )]);
        for (raw, shown) in NEGATED_EXAMPLES {
            // When the mod rolls the raw value
            let rolled = Mod::roll(&fixed(&negated, raw), &mut rng);
            // Then the text shows the value with the reversed sign
            assert_eq!(
                rolled.display_text(),
                format!("{shown}% reduced Invented Duration"),
                "raw value {raw}, precision {precision:?}"
            );
            assert_eq!(
                shown_value(raw, negated.conversions.get("invented_negated")),
                shown
            );
            // And the rolled mod keeps the conversion and the raw value
            assert_eq!(rolled.conversions, negated.conversions);
            assert_eq!(stat_value(&rolled, "invented_negated"), raw);
        }
    }

    // --- The ring bases ---
    // Given the implicits of all ring bases in the data file and in crafting-core
    let base_items_file = load_json(BASE_ITEMS_JSON);
    let bases = entries_by_id(&base_items_file, "base_items", BASE_ITEMS_JSON);
    let expected_negated: BTreeSet<(String, String)> = BASES
        .iter()
        .flat_map(|base| {
            base.implicits.iter().flat_map(move |implicit| {
                implicit
                    .negated
                    .iter()
                    .map(move |stat| (base.id.to_owned(), (*stat).to_owned()))
            })
        })
        .collect();
    let mut file_negated: BTreeSet<(String, String)> = BTreeSet::new();
    let mut core_negated: BTreeSet<(String, String)> = BTreeSet::new();
    for expected in &BASES {
        let what = format!("{BASE_ITEMS_JSON}: {:?}", expected.id);
        for implicit in array_field(
            entry(&bases, expected.id, BASE_ITEMS_JSON),
            "implicits",
            &what,
        ) {
            for (stat, conversion) in conversions_field(implicit, &what) {
                // Then every conversion of a ring implicit is a negation without a precision
                assert_eq!(
                    conversion,
                    (i64::from(NEGATE_DIVISOR), None),
                    "{what}: conversion of {stat:?}"
                );
                file_negated.insert((expected.id.to_owned(), stat));
            }
        }
        let base = base_item(expected.id)
            .unwrap_or_else(|| panic!("crafting-core must offer {:?}", expected.id));
        for implicit in &base.implicits {
            for (stat, conversion) in &implicit.conversions {
                assert_eq!(
                    conversion,
                    &Conversion {
                        divisor: NEGATE_DIVISOR,
                        precision: None
                    },
                    "base_item({:?}): conversion of {stat:?}",
                    expected.id
                );
                core_negated.insert((expected.id.to_owned(), stat.clone()));
            }
        }
    }
    // And exactly the stats that the game data shows through negate are negated
    assert_eq!(
        file_negated, expected_negated,
        "{BASE_ITEMS_JSON}: negated stats"
    );
    assert_eq!(
        core_negated, expected_negated,
        "crafting-core: negated stats"
    );

    // When a normal Dusk Ring is created
    let dusk_ring = base_item(DUSK_RING_ID).unwrap_or_else(|| panic!("no base {DUSK_RING_ID}"));
    let item = Item::new(&dusk_ring, &mut rng);
    // Then its implicit shows the example of the criterion
    let text = item.implicits[0].display_text();
    assert_eq!(
        text.lines().next(),
        Some(DUSK_RING_LEFT_LINE),
        "the Dusk Ring's implicit, found {text:?}"
    );

    // And every ring with a negated value shows it by the rules, without a minus sign
    for expected in BASES.iter().filter(|base| {
        base.implicits
            .iter()
            .any(|implicit| !implicit.negated.is_empty())
    }) {
        let base = base_item(expected.id)
            .unwrap_or_else(|| panic!("crafting-core must offer {:?}", expected.id));
        for _ in 0..ROLLS {
            let item = Item::new(&base, &mut rng);
            for rolled in &item.implicits {
                let text = rolled.display_text();
                assert_eq!(text, expected_display_text(rolled), "{:?}", expected.name);
                assert!(
                    !text.contains('-'),
                    "{:?}: a negated value is shown without a minus sign, found {text:?}",
                    expected.name
                );
            }
        }
    }

    // --- The converter ---
    // Given invented raw files with an implicit whose left line the game data shows through
    // the handler negate
    let raw = invented_raw_files();
    // When the converter runs
    let converted = convert(&raw.files(), &[TEST_DUSK_RING, TEST_UNSET_RING])
        .expect("the invented raw files convert");
    // Then the left stat gets the divisor -1 without a precision, the right stat no conversion
    let implicit = converted_implicit(&converted.base_items_json, TEST_DUSK_RING);
    let what = "converted implicit with negate";
    assert_eq!(
        conversions_field(&implicit, what),
        BTreeMap::from([(TEST_LEFT_STAT.to_owned(), (i64::from(NEGATE_DIVISOR), None))]),
        "{what}: conversions"
    );
    assert_eq!(
        stats_field(&implicit, what),
        BTreeMap::from([
            (TEST_LEFT_STAT.to_owned(), (-15, -15)),
            (TEST_RIGHT_STAT.to_owned(), (15, 15)),
        ]),
        "{what}: the raw values stay"
    );
    // And the text has the wording of the translation for the raw range, line by line
    assert_eq!(
        canonical_entry_text(&implicit, what),
        format!(
            "Left test slot: {{{TEST_LEFT_STAT}}}% reduced Test Duration\n\
             Right test slot: {{{TEST_RIGHT_STAT}}}% increased Test Duration"
        ),
        "{what}: text"
    );

    // When the negated translation uses a handler with another multiplier
    let mut changed = raw.clone();
    *at(
        &mut changed.stat_translations,
        &[LEFT_ENTRY, "English", "1", "index_handlers", "0"],
    ) = json!(["negate_and_double"]);
    // Then the converter stops with a message that names it, and does not panic
    assert_conversion_fails(
        "a handler with the multiplier -2",
        &changed.files(),
        &[TEST_DUSK_RING],
        &[
            "negate_and_double",
            TEST_LEFT_STAT,
            TEST_DUSK_IMPLICIT,
            TEST_DUSK_RING,
        ],
    );
}

// AC6: „Die Website zeigt weiterhin den Iron Ring wie bisher."
// (Tech lead's plan: the page builds its pool with `valid_mods` from `mod_pool` of its start
// base, the Iron Ring, so the Iron Ring keeps its own pool and rolls no other base's mods. The
// page itself is checked by the PO in the local preview.)
// (Adapted for AB#248, PO-confirmed: criterion 4 of AB#248 replaces the check that the page
// builds the pool with `valid_mods`. The page passes the base's pool from `mod_pool`, and the
// orbs apply the item level rule themselves (ADR 0021, `story_248::ac4`). The rest stays.)
#[test]
fn ac6_the_website_still_shows_the_iron_ring_with_its_own_pool() {
    // Given the Iron Ring's pool in the data file
    let base_items_file = load_json(BASE_ITEMS_JSON);
    let mods_file = load_json(MODS_JSON);
    let bases = entries_by_id(&base_items_file, "base_items", BASE_ITEMS_JSON);
    let mod_entries = entries_by_id(&mods_file, "mods", MODS_JSON);
    let file_names: Vec<&str> = strings_field(
        entry(&bases, IRON_RING_ID, BASE_ITEMS_JSON),
        "mod_pool",
        BASE_ITEMS_JSON,
    )
    .iter()
    .map(|id| str_field(entry(&mod_entries, id, MODS_JSON), "name", MODS_JSON))
    .collect();

    // When crafting-core gives the Iron Ring's pool
    let pool = mod_pool(IRON_RING_ID)
        .unwrap_or_else(|| panic!("mod_pool({IRON_RING_ID:?}) must offer the Iron Ring's pool"));

    // Then it is exactly the Iron Ring's own pool, in the order of the data file
    let names: Vec<&str> = pool.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(names.len(), IRON_POOL_SIZE, "size of the Iron Ring's pool");
    assert_eq!(names, file_names, "the Iron Ring's pool");
    // And it holds none of the other bases' additional mods
    for extra in MINION_MODS.iter().chain(UNSET_MODS.iter()) {
        assert!(
            !names.contains(&extra.name),
            "the Iron Ring's pool must not contain {:?}",
            extra.name
        );
    }
    // And at the UI's item level, the page's pool is the Iron Ring's whole pool, as before
    assert_eq!(
        valid_mods(&pool, UI_ITEM_LEVEL),
        pool,
        "valid mods at item level {UI_ITEM_LEVEL}"
    );
    // And the Iron Ring itself is unchanged
    let iron_ring = base_item(IRON_RING_ID).unwrap_or_else(|| panic!("no base {IRON_RING_ID}"));
    assert_eq!(iron_ring.name, IRON_RING_NAME);
    let expected = expected_base(IRON_RING_ID);
    assert_eq!(iron_ring.implicits.len(), expected.implicits.len());
    assert_core_implicit(
        &iron_ring.implicits[0],
        &expected.implicits[0],
        "the Iron Ring's implicit",
    );
    // And the page starts with the Iron Ring and builds its pool from the Iron Ring's own pool
    assert_web_src_uses(IRON_RING_ID, "the page starts with the Iron Ring");
    assert_web_src_uses("mod_pool", "the page takes the Iron Ring's own pool");
}
