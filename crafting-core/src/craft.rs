//! Crafting actions on items. Results are values and reason codes, never UI text.

use crate::model::{AffixType, ExplicitMod, ExplicitModDefinition, Item, Mod, Rarity};
use crate::rng::{choose_weighted, roll_below, Rng};

/// Why a currency cannot be applied. A reason code; the UI decides how to show it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CraftError {
    /// The currency needs a normal item.
    ItemNotNormal,
    /// There is no valid mod to add.
    NoValidMod,
}

impl CraftError {
    /// Every reason code.
    pub(crate) const ALL: [CraftError; 2] = [CraftError::ItemNotNormal, CraftError::NoValidMod];

    /// The identifier of the reason in lower snake case. It is the key of the failure
    /// messages in the game data (ADR 0012).
    pub fn code(self) -> &'static str {
        match self {
            CraftError::ItemNotNormal => "item_not_normal",
            CraftError::NoValidMod => "no_valid_mod",
        }
    }
}

/// Whether the orb can be applied to `item`, without looking at any mod pool.
pub fn can_apply_orb_of_transmutation(item: &Item) -> Result<(), CraftError> {
    if item.rarity == Rarity::Normal {
        Ok(())
    } else {
        Err(CraftError::ItemNotNormal)
    }
}

/// Makes a normal item magic and gives it new mods from `pool`, the valid mods.
///
/// The item gets one or two mods, each with a chance of 50 %. One mod is chosen by weight
/// from the whole pool. Two mods are a prefix chosen by weight from the prefixes and a
/// suffix chosen by weight from the suffixes; if a type has no mod, its mod is left out.
///
/// On an error the item stays unchanged.
///
/// # Panics
/// If a mod of `pool` has weight zero.
pub fn apply_orb_of_transmutation(
    item: &mut Item,
    pool: &[ExplicitModDefinition],
    rng: &mut impl Rng,
) -> Result<(), CraftError> {
    can_apply_orb_of_transmutation(item)?;
    if pool.is_empty() {
        return Err(CraftError::NoValidMod);
    }

    let mut prefix = None;
    let mut suffix = None;
    if roll_below(rng, 2) == 0 {
        let chosen = choose_from(rng, pool.iter());
        match chosen.affix_type {
            AffixType::Prefix => prefix = Some(chosen),
            AffixType::Suffix => suffix = Some(chosen),
        }
    } else {
        prefix = choose_of_type(rng, pool, AffixType::Prefix);
        suffix = choose_of_type(rng, pool, AffixType::Suffix);
    }

    item.rarity = Rarity::Magic;
    item.prefix = prefix.map(|definition| roll_explicit(definition, rng));
    item.suffix = suffix.map(|definition| roll_explicit(definition, rng));
    Ok(())
}

/// One mod of the given type chosen by weight, or `None` if there is none.
fn choose_of_type<'a>(
    rng: &mut impl Rng,
    pool: &'a [ExplicitModDefinition],
    affix_type: AffixType,
) -> Option<&'a ExplicitModDefinition> {
    let mut candidates = pool
        .iter()
        .filter(|definition| definition.affix_type == affix_type)
        .peekable();
    candidates.peek()?;
    Some(choose_from(rng, candidates))
}

/// One of the non-empty `candidates` chosen by weight.
fn choose_from<'a>(
    rng: &mut impl Rng,
    candidates: impl Iterator<Item = &'a ExplicitModDefinition>,
) -> &'a ExplicitModDefinition {
    let candidates: Vec<&ExplicitModDefinition> = candidates.collect();
    let weights: Vec<u32> = candidates
        .iter()
        .map(|definition| definition.weight)
        .collect();
    candidates[choose_weighted(rng, &weights)]
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
            level: 1,
            weight,
            definition: ModDefinition {
                family: name.to_owned(),
                tags: Vec::new(),
                stats: vec![StatDefinition {
                    id: "stat".to_owned(),
                    min: 1,
                    max: 5,
                    global: false,
                }],
                text: "+{value}".to_owned(),
                placeholders: BTreeMap::from([("value".to_owned(), "stat".to_owned())]),
            },
        }
    }

    fn normal_item(rng: &mut SeededRng) -> Item {
        let base = BaseItem {
            id: "invented/base".to_owned(),
            name: "Invented Base".to_owned(),
            item_class: "Inventeds".to_owned(),
            required_level: 1,
            tags: Vec::new(),
            implicits: Vec::new(),
        };
        Item::new(&base, rng)
    }

    #[test]
    fn all_lists_every_reason_with_a_unique_lower_snake_case_code() {
        // The exhaustive match makes a new variant a compile error until it is listed.
        for error in CraftError::ALL {
            match error {
                CraftError::ItemNotNormal | CraftError::NoValidMod => {}
            }
        }
        let codes: std::collections::BTreeSet<&str> =
            CraftError::ALL.iter().map(|error| error.code()).collect();
        assert_eq!(codes.len(), CraftError::ALL.len());
        for code in codes {
            assert!(!code.is_empty());
            assert!(code
                .chars()
                .all(|c| c.is_ascii_lowercase() || c == '_'));
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

        let mut normal = normal_item(&mut rng);
        let before = normal.clone();
        assert_eq!(
            apply_orb_of_transmutation(&mut normal, &[], &mut rng),
            Err(CraftError::NoValidMod)
        );
        assert_eq!(normal, before);
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
    fn a_single_mod_pool_gives_exactly_that_mod_in_its_slot() {
        let mut rng = SeededRng::new(3);
        for affix_type in [AffixType::Prefix, AffixType::Suffix] {
            let pool = [mod_of("only", affix_type, 7)];
            for _ in 0..50 {
                let mut item = normal_item(&mut rng);
                apply_orb_of_transmutation(&mut item, &pool, &mut rng).unwrap();
                assert_eq!(item.rarity, Rarity::Magic);
                let (own, other) = match affix_type {
                    AffixType::Prefix => (&item.prefix, &item.suffix),
                    AffixType::Suffix => (&item.suffix, &item.prefix),
                };
                assert_eq!(own.as_ref().map(|m| m.name.as_str()), Some("only"));
                assert_eq!(*other, None);
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
    #[should_panic(expected = "zero weight")]
    fn a_mod_with_weight_zero_is_a_programming_error() {
        let mut rng = SeededRng::new(1);
        let mut item = normal_item(&mut rng);
        let _ = apply_orb_of_transmutation(
            &mut item,
            &[mod_of("zero", AffixType::Prefix, 0)],
            &mut rng,
        );
    }
}
