//! Crafting actions on items. Results are values and reason codes, never UI text.

use crate::damage_lock::{blocked_tags, is_blocked};
use crate::model::{AffixLimits, AffixType, ExplicitMod, ExplicitModDefinition, Item, Mod, Rarity};
use crate::rng::{choose_weighted, roll_below, Rng};

/// Why a currency cannot be applied. A reason code; the UI decides how to show it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CraftError {
    /// The currency needs a normal item.
    ItemNotNormal,
    /// The currency needs a magic item.
    ItemNotMagic,
}

impl CraftError {
    /// Every reason code.
    pub(crate) const ALL: [CraftError; 2] = [CraftError::ItemNotNormal, CraftError::ItemNotMagic];

    /// The identifier of the reason in lower snake case. It is the key of the failure
    /// messages in the game data (ADR 0012).
    pub fn code(self) -> &'static str {
        match self {
            CraftError::ItemNotNormal => "item_not_normal",
            CraftError::ItemNotMagic => "item_not_magic",
        }
    }
}

/// Whether a mod can roll on an item of the given level. This is the only place of the item
/// level rule.
fn rolls_at(definition: &ExplicitModDefinition, item_level: u32) -> bool {
    definition.definition.level <= item_level
}

/// The mods of `mods` that can roll on an item of the given level: the mods whose level is at
/// most `item_level`, in their order (ADR 0013).
pub fn valid_mods(mods: &[ExplicitModDefinition], item_level: u32) -> Vec<ExplicitModDefinition> {
    mods.iter()
        .filter(|definition| rolls_at(definition, item_level))
        .cloned()
        .collect()
}

/// Step 1 of the rule "Adding a random mod": the valid pool of `item` for a random mod from
/// `base_pool`, in the order of `base_pool`.
///
/// A mod is in it if its level is at most the item level, it is not blocked (Breach rings),
/// its side has room under both affix limits, and no prefix or suffix of its family is on the
/// item. Implicit mods count neither toward the limits nor for the family. An item without
/// affix limits (a unique item) has no room, so its valid pool is empty.
///
/// This is the only implementation of step 1 (ADR 0021).
pub fn valid_pool(item: &Item, base_pool: &[ExplicitModDefinition]) -> Vec<ExplicitModDefinition> {
    valid_candidates(item, base_pool)
        .into_iter()
        .cloned()
        .collect()
}

/// The mods of the valid pool as references into `base_pool`. `valid_pool` and
/// `add_random_mod` build the pool here; the references avoid copying the pool for every
/// mod that is added.
fn valid_candidates<'a>(
    item: &Item,
    base_pool: &'a [ExplicitModDefinition],
) -> Vec<&'a ExplicitModDefinition> {
    let Some(limits) = AffixLimits::of(item.rarity) else {
        return Vec::new();
    };
    let total_has_room = item.prefixes.len() + item.suffixes.len() < limits.mods;
    let prefix_has_room = total_has_room && item.prefixes.len() < limits.prefixes;
    let suffix_has_room = total_has_room && item.suffixes.len() < limits.suffixes;
    let blocked = blocked_tags(item);
    base_pool
        .iter()
        .filter(|definition| {
            let side_has_room = match definition.affix_type {
                AffixType::Prefix => prefix_has_room,
                AffixType::Suffix => suffix_has_room,
            };
            rolls_at(definition, item.item_level)
                && !is_blocked(&blocked, definition)
                && side_has_room
                && !item
                    .prefixes
                    .iter()
                    .chain(&item.suffixes)
                    .any(|held| held.modifier.family == definition.definition.family)
        })
        .collect()
}

/// Adds a random mod to `item` by the rule "Adding a random mod": builds the valid pool from
/// `base_pool` (`valid_pool`), chooses one mod by weight, rolls it and adds it at the end of
/// its side. Returns whether a mod was added. If the valid pool is empty, no mod is added and
/// the item stays unchanged.
///
/// This is the only implementation of the rule (ADR 0021); every currency that adds a random
/// mod calls it.
///
/// # Panics
/// If a mod of the valid pool has weight zero.
pub fn add_random_mod(
    item: &mut Item,
    base_pool: &[ExplicitModDefinition],
    rng: &mut impl Rng,
) -> bool {
    let candidates = valid_candidates(item, base_pool);
    if candidates.is_empty() {
        return false;
    }
    let weights: Vec<u32> = candidates
        .iter()
        .map(|definition| definition.weight)
        .collect();
    let chosen = candidates[choose_weighted(rng, &weights)];
    let explicit = roll_explicit(chosen, rng);
    match explicit.affix_type {
        AffixType::Prefix => item.prefixes.push(explicit),
        AffixType::Suffix => item.suffixes.push(explicit),
    }
    true
}

/// Whether the orb can be applied to `item`, without looking at any mod pool.
pub fn can_apply_orb_of_transmutation(item: &Item) -> Result<(), CraftError> {
    if item.rarity == Rarity::Normal {
        Ok(())
    } else {
        Err(CraftError::ItemNotNormal)
    }
}

/// Makes a normal item magic and gives it new mods from `pool`, the pool of its base.
///
/// The orb chooses a target number of mods, 1 or 2, each with a chance of 50 %. It then adds
/// random mods (`add_random_mod`) until the item has that many mods or no mod is added. The
/// orb always takes effect: without a valid mod the item becomes magic with no mods.
///
/// On an error the item stays unchanged.
///
/// # Panics
/// If a mod of the valid pool has weight zero.
pub fn apply_orb_of_transmutation(
    item: &mut Item,
    pool: &[ExplicitModDefinition],
    rng: &mut impl Rng,
) -> Result<(), CraftError> {
    can_apply_orb_of_transmutation(item)?;
    item.rarity = Rarity::Magic;
    add_random_mods(item, pool, rng);
    Ok(())
}

/// Whether the orb can be applied to `item`, without looking at any mod pool.
pub fn can_apply_orb_of_alteration(item: &Item) -> Result<(), CraftError> {
    if item.rarity == Rarity::Magic {
        Ok(())
    } else {
        Err(CraftError::ItemNotMagic)
    }
}

/// Removes all prefixes and suffixes of a magic item and adds new mods from `pool`, the pool
/// of its base, exactly as when a normal item becomes magic. By chance the new mods can be the
/// same as the old ones. The orb always takes effect: without a valid mod the item has no mods
/// afterwards.
///
/// On an error the item stays unchanged.
///
/// # Panics
/// If a mod of the valid pool has weight zero.
pub fn apply_orb_of_alteration(
    item: &mut Item,
    pool: &[ExplicitModDefinition],
    rng: &mut impl Rng,
) -> Result<(), CraftError> {
    can_apply_orb_of_alteration(item)?;
    item.prefixes.clear();
    item.suffixes.clear();
    add_random_mods(item, pool, rng);
    Ok(())
}

/// The rule of both orbs: choose a target of 1 or 2 mods, each with a chance of 50 %, then add
/// random mods until the item has that many prefixes and suffixes or no mod is added.
fn add_random_mods(item: &mut Item, pool: &[ExplicitModDefinition], rng: &mut impl Rng) {
    let target = 1 + roll_below(rng, 2) as usize;
    while item.prefixes.len() + item.suffixes.len() < target && add_random_mod(item, pool, rng) {}
}

fn roll_explicit(definition: &ExplicitModDefinition, rng: &mut impl Rng) -> ExplicitMod {
    ExplicitMod {
        name: definition.name.clone(),
        affix_type: definition.affix_type,
        modifier: Mod::roll(&definition.definition, rng),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::model::{BaseItem, ModDefinition, StatDefinition};
    use crate::rng::SeededRng;

    fn mod_of(name: &str, affix_type: AffixType, weight: u32) -> ExplicitModDefinition {
        ExplicitModDefinition {
            name: name.to_owned(),
            affix_type,
            weight,
            definition: ModDefinition {
                family: name.to_owned(),
                level: 1,
                tags: Vec::new(),
                stats: vec![StatDefinition {
                    id: "stat".to_owned(),
                    min: 1,
                    max: 5,
                }],
                text: "+{value}".to_owned(),
                placeholders: BTreeMap::from([("value".to_owned(), "stat".to_owned())]),
                conversions: BTreeMap::new(),
            },
        }
    }

    fn base_with_id(id: &str) -> BaseItem {
        BaseItem {
            id: id.to_owned(),
            name: "Invented Base".to_owned(),
            item_class: "Inventeds".to_owned(),
            required_level: 1,
            tags: Vec::new(),
            implicits: Vec::new(),
        }
    }

    fn normal_item(rng: &mut SeededRng) -> Item {
        Item::new(&base_with_id("invented/base"), rng)
    }

    fn magic_item(rng: &mut SeededRng) -> Item {
        let mut item = normal_item(rng);
        item.rarity = Rarity::Magic;
        item
    }

    fn names(mods: &[ExplicitMod]) -> Vec<&str> {
        mods.iter().map(|explicit| explicit.name.as_str()).collect()
    }

    #[test]
    fn all_lists_every_reason_with_a_unique_lower_snake_case_code() {
        // The exhaustive match makes a new variant a compile error until it is listed.
        for error in CraftError::ALL {
            match error {
                CraftError::ItemNotNormal | CraftError::ItemNotMagic => {}
            }
        }
        let codes: std::collections::BTreeSet<&str> =
            CraftError::ALL.iter().map(|error| error.code()).collect();
        assert_eq!(codes.len(), CraftError::ALL.len());
        for code in codes {
            assert!(!code.is_empty());
            assert!(code.chars().all(|c| c.is_ascii_lowercase() || c == '_'));
        }
    }

    #[test]
    fn a_normal_item_can_take_the_orb() {
        let item = normal_item(&mut SeededRng::new(1));
        assert_eq!(can_apply_orb_of_transmutation(&item), Ok(()));
    }

    #[test]
    fn items_of_other_rarities_cannot_take_the_orb() {
        for rarity in [Rarity::Magic, Rarity::Rare, Rarity::Unique] {
            let mut item = normal_item(&mut SeededRng::new(1));
            item.rarity = rarity;
            assert_eq!(
                can_apply_orb_of_transmutation(&item),
                Err(CraftError::ItemNotNormal)
            );
        }
    }

    #[test]
    fn a_failed_orb_leaves_the_item_unchanged() {
        let mut rng = SeededRng::new(1);
        let pool = [mod_of("a", AffixType::Prefix, 1)];
        let mut magic = normal_item(&mut rng);
        apply_orb_of_transmutation(&mut magic, &pool, &mut rng).unwrap();
        let before = magic.clone();
        assert_eq!(
            apply_orb_of_transmutation(&mut magic, &pool, &mut rng),
            Err(CraftError::ItemNotNormal)
        );
        assert_eq!(magic, before);
    }

    #[test]
    fn without_a_valid_mod_the_orb_makes_the_item_magic_with_no_mods() {
        let mut rng = SeededRng::new(1);
        let mut high = mod_of("high", AffixType::Prefix, 1);
        high.definition.level = 2;
        for pool in [Vec::new(), vec![high]] {
            let mut item = normal_item(&mut rng);
            item.item_level = 1;
            assert_eq!(
                apply_orb_of_transmutation(&mut item, &pool, &mut rng),
                Ok(())
            );
            assert_eq!(item.rarity, Rarity::Magic);
            assert!(item.prefixes.is_empty() && item.suffixes.is_empty());
        }
    }

    #[test]
    fn a_non_normal_item_is_reported_before_an_empty_pool() {
        let mut rng = SeededRng::new(1);
        let mut item = normal_item(&mut rng);
        item.rarity = Rarity::Rare;
        assert_eq!(
            apply_orb_of_transmutation(&mut item, &[], &mut rng),
            Err(CraftError::ItemNotNormal)
        );
    }

    #[test]
    fn a_single_mod_pool_gives_exactly_that_mod_on_its_side() {
        let mut rng = SeededRng::new(3);
        for affix_type in [AffixType::Prefix, AffixType::Suffix] {
            let pool = [mod_of("only", affix_type, 7)];
            for _ in 0..50 {
                let mut item = normal_item(&mut rng);
                apply_orb_of_transmutation(&mut item, &pool, &mut rng).unwrap();
                assert_eq!(item.rarity, Rarity::Magic);
                let (own, other) = match affix_type {
                    AffixType::Prefix => (&item.prefixes, &item.suffixes),
                    AffixType::Suffix => (&item.suffixes, &item.prefixes),
                };
                assert_eq!(names(own), ["only"]);
                assert!(other.is_empty());
            }
        }
    }

    #[test]
    fn the_same_seed_gives_the_same_item() {
        let pool = [
            mod_of("p1", AffixType::Prefix, 5),
            mod_of("p2", AffixType::Prefix, 3),
            mod_of("s1", AffixType::Suffix, 2),
        ];
        let make = |seed| {
            let mut rng = SeededRng::new(seed);
            let mut item = normal_item(&mut rng);
            apply_orb_of_transmutation(&mut item, &pool, &mut rng).unwrap();
            item
        };
        assert_eq!(make(9), make(9));
    }

    #[test]
    fn valid_mods_keep_the_mods_up_to_the_item_level_in_their_order() {
        let mut low = mod_of("low", AffixType::Prefix, 1);
        low.definition.level = 59;
        let mut equal = mod_of("equal", AffixType::Suffix, 1);
        equal.definition.level = 60;
        let mut high = mod_of("high", AffixType::Prefix, 1);
        high.definition.level = 61;
        let mods = [high.clone(), equal.clone(), low.clone()];
        assert_eq!(valid_mods(&mods, 60), vec![equal, low]);
        assert_eq!(valid_mods(&mods, 61).len(), 3);
        assert_eq!(valid_mods(&mods, 58), Vec::new());
        assert_eq!(valid_mods(&[], 100), Vec::new());
    }

    #[test]
    fn the_valid_pool_applies_the_four_filters_and_keeps_the_order() {
        let mut rng = SeededRng::new(1);
        let mut high = mod_of("high", AffixType::Prefix, 1);
        high.definition.level = 50;
        let mut fire = mod_of("fire", AffixType::Prefix, 1);
        fire.definition.tags = vec!["fire".to_owned()];
        let mut same_family = mod_of("same family", AffixType::Suffix, 1);
        same_family.definition.family = "held".to_owned();
        let held = mod_of("held", AffixType::Prefix, 1);
        let pool = vec![
            high,
            mod_of("p1", AffixType::Prefix, 1),
            fire,
            mod_of("s1", AffixType::Suffix, 1),
            same_family,
        ];

        // The Cryonic Ring has a lock to cold: the fire mod is blocked.
        let mut item = Item::new(&base_with_id("Metadata/Items/Rings/RingB1"), &mut rng);
        item.rarity = Rarity::Rare;
        item.item_level = 49;
        item.prefixes.push(roll_explicit(&held, &mut rng));
        let valid = valid_pool(&item, &pool);
        let valid_names: Vec<&str> = valid
            .iter()
            .map(|definition| definition.name.as_str())
            .collect();
        assert_eq!(valid_names, ["p1", "s1"]);
    }

    #[test]
    fn an_item_without_affix_limits_has_no_valid_pool() {
        let mut rng = SeededRng::new(1);
        let mut item = normal_item(&mut rng);
        item.rarity = Rarity::Unique;
        let pool = [mod_of("a", AffixType::Prefix, 1)];
        assert!(valid_pool(&item, &pool).is_empty());
        assert!(!add_random_mod(&mut item, &pool, &mut rng));
    }

    #[test]
    fn add_random_mod_appends_to_the_side_and_stops_when_the_limits_are_reached() {
        let mut rng = SeededRng::new(4);
        let pool: Vec<ExplicitModDefinition> = (1..=4)
            .flat_map(|n| {
                [
                    mod_of(&format!("p{n}"), AffixType::Prefix, 1),
                    mod_of(&format!("s{n}"), AffixType::Suffix, 1),
                ]
            })
            .collect();
        for (rarity, expected) in [(Rarity::Magic, (1, 1)), (Rarity::Rare, (3, 3))] {
            let mut item = normal_item(&mut rng);
            item.rarity = rarity;
            let mut added = 0;
            while add_random_mod(&mut item, &pool, &mut rng) {
                added += 1;
            }
            assert_eq!((item.prefixes.len(), item.suffixes.len()), expected);
            assert_eq!(added, expected.0 + expected.1);
            assert!(!add_random_mod(&mut item, &pool, &mut rng));
        }
    }

    #[test]
    #[should_panic(expected = "zero weight")]
    fn a_mod_with_weight_zero_in_the_valid_pool_is_a_programming_error() {
        let mut rng = SeededRng::new(1);
        let mut item = magic_item(&mut rng);
        add_random_mod(&mut item, &[mod_of("zero", AffixType::Prefix, 0)], &mut rng);
    }

    #[test]
    fn a_magic_item_can_take_the_orb_of_alteration() {
        let item = magic_item(&mut SeededRng::new(1));
        assert_eq!(can_apply_orb_of_alteration(&item), Ok(()));
    }

    #[test]
    fn items_of_other_rarities_cannot_take_the_orb_of_alteration() {
        for rarity in [Rarity::Normal, Rarity::Rare, Rarity::Unique] {
            let mut item = magic_item(&mut SeededRng::new(1));
            item.rarity = rarity;
            assert_eq!(
                can_apply_orb_of_alteration(&item),
                Err(CraftError::ItemNotMagic)
            );
        }
    }

    #[test]
    fn a_failed_orb_of_alteration_leaves_the_item_unchanged() {
        let mut rng = SeededRng::new(1);
        let mut normal = normal_item(&mut rng);
        let before = normal.clone();
        let pool = [mod_of("a", AffixType::Prefix, 1)];
        assert_eq!(
            apply_orb_of_alteration(&mut normal, &pool, &mut rng),
            Err(CraftError::ItemNotMagic)
        );
        assert_eq!(normal, before);
    }

    #[test]
    fn a_non_magic_item_is_reported_before_an_empty_pool_by_the_orb_of_alteration() {
        let mut rng = SeededRng::new(1);
        let mut item = normal_item(&mut rng);
        assert_eq!(
            apply_orb_of_alteration(&mut item, &[], &mut rng),
            Err(CraftError::ItemNotMagic)
        );
    }

    #[test]
    fn the_orb_of_alteration_replaces_the_old_mods_and_keeps_the_rarity() {
        let mut rng = SeededRng::new(5);
        let old_prefix = mod_of("old prefix", AffixType::Prefix, 1);
        let old_suffix = mod_of("old suffix", AffixType::Suffix, 1);
        let pool = [mod_of("new", AffixType::Suffix, 7)];
        for _ in 0..50 {
            let mut item = magic_item(&mut rng);
            item.prefixes.push(roll_explicit(&old_prefix, &mut rng));
            item.suffixes.push(roll_explicit(&old_suffix, &mut rng));
            apply_orb_of_alteration(&mut item, &pool, &mut rng).unwrap();
            assert_eq!(item.rarity, Rarity::Magic);
            assert!(item.prefixes.is_empty());
            assert_eq!(names(&item.suffixes), ["new"]);
        }
    }

    #[test]
    fn without_a_valid_mod_the_orb_of_alteration_removes_the_old_mods() {
        let mut rng = SeededRng::new(5);
        let old_prefix = mod_of("old prefix", AffixType::Prefix, 1);
        let mut item = magic_item(&mut rng);
        item.prefixes.push(roll_explicit(&old_prefix, &mut rng));
        assert_eq!(apply_orb_of_alteration(&mut item, &[], &mut rng), Ok(()));
        assert_eq!(item.rarity, Rarity::Magic);
        assert!(item.prefixes.is_empty() && item.suffixes.is_empty());
    }

    #[test]
    fn an_orb_never_adds_a_second_mod_of_the_family_of_the_first() {
        let mut rng = SeededRng::new(6);
        let mut suffix = mod_of("of the family", AffixType::Suffix, 1);
        suffix.definition.family = "p".to_owned();
        let pool = [mod_of("p", AffixType::Prefix, 1), suffix];
        for _ in 0..200 {
            let mut item = normal_item(&mut rng);
            apply_orb_of_transmutation(&mut item, &pool, &mut rng).unwrap();
            assert_eq!(item.prefixes.len() + item.suffixes.len(), 1);
        }
    }
}
