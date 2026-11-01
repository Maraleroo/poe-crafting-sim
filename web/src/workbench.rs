//! The state of the start page: the item on the workbench and the generator for its rolls.
//!
//! This is plain Rust so it can be tested without a browser (ADR 0010). All crafting rules
//! are in `crafting-core`; this only keeps the state and calls them.

use crate::item_level;
use crafting_core::{
    apply_orb_of_alteration, apply_orb_of_augmentation, apply_orb_of_transmutation,
    apply_regal_orb, base_item, can_apply_orb_of_alteration, can_apply_orb_of_augmentation,
    can_apply_orb_of_transmutation, can_apply_regal_orb, mod_pool, BaseItem, CraftError, Currency,
    ExplicitModDefinition, Item, SeededRng,
};

/// The base item on the workbench (a reference, not item data).
const START_ITEM_ID: &str = "Metadata/Items/Rings/Ring1";

/// The pool of the base item with the given identifier, as the orbs take it. The orbs build
/// the valid pool from it themselves (ADR 0021).
fn base_pool(base_id: &str) -> Vec<ExplicitModDefinition> {
    mod_pool(base_id).unwrap_or_else(|| panic!("crafting-core has no pool for {base_id:?}"))
}

/// The item, its base, the pool of the base, the chosen item level and the generator that
/// serves every roll of the page. The chosen item level is set on every new item (ADR 0021).
pub struct Workbench {
    base: BaseItem,
    item_level: u32,
    pool: Vec<ExplicitModDefinition>,
    item: Item,
    rng: SeededRng,
}

impl Workbench {
    /// A workbench with a new normal item of the start base item at the default item level and
    /// that base item's own pool. The same seed gives the same sequence of results.
    pub fn new(seed: u64) -> Self {
        Self::with_pool(seed, base_pool(START_ITEM_ID))
    }

    /// Like [`Workbench::new`], but with the given pool instead of the pool of the base item.
    pub fn with_pool(seed: u64, pool: Vec<ExplicitModDefinition>) -> Self {
        let base = base_item(START_ITEM_ID)
            .unwrap_or_else(|| panic!("crafting-core has no base item {START_ITEM_ID:?}"));
        let mut rng = SeededRng::new(seed);
        let mut item = Item::new(&base, &mut rng);
        item.item_level = item_level::DEFAULT;
        Self {
            base,
            item_level: item_level::DEFAULT,
            pool,
            item,
            rng,
        }
    }

    /// The item on the workbench.
    pub fn item(&self) -> &Item {
        &self.item
    }

    /// Whether the currency can be applied to the item.
    pub fn applicable(&self, currency: Currency) -> bool {
        self.check(currency).is_ok()
    }

    /// The tooltip of the currency's button, a game text from `crafting-core`: the description
    /// if the currency can be applied, otherwise the failure message for the reason it cannot.
    pub fn tooltip(&self, currency: Currency) -> String {
        match self.check(currency) {
            Ok(()) => currency.description(),
            Err(error) => currency.failure_message(error),
        }
    }

    /// The game's message for a failure of the currency.
    pub fn failure_message(currency: Currency, error: CraftError) -> String {
        currency.failure_message(error)
    }

    /// Applies the currency to the item. On an error the item stays unchanged.
    pub fn apply(&mut self, currency: Currency) -> Result<(), CraftError> {
        match currency {
            Currency::OrbOfTransmutation => {
                apply_orb_of_transmutation(&mut self.item, &self.pool, &mut self.rng)
            }
            Currency::OrbOfAlteration => {
                apply_orb_of_alteration(&mut self.item, &self.pool, &mut self.rng)
            }
            Currency::OrbOfAugmentation => {
                apply_orb_of_augmentation(&mut self.item, &self.pool, &mut self.rng)
            }
            Currency::RegalOrb => apply_regal_orb(&mut self.item, &self.pool, &mut self.rng),
        }
    }

    /// Replaces the item with a new normal item of the same base at the chosen item level.
    pub fn reset(&mut self) {
        self.item = Item::new(&self.base, &mut self.rng);
        self.item.item_level = self.item_level;
    }

    /// The chosen item level.
    pub fn item_level(&self) -> u32 {
        self.item_level
    }

    /// Chooses the item level: the item becomes a new normal item of the base at that level,
    /// as after [`Workbench::reset`]. The same level as the current one is no change, and the
    /// item stays.
    ///
    /// # Panics
    /// If the level is not an allowed one (`item_level::ALLOWED`). The page only passes levels
    /// it has read with `item_level::parse`, so that is a programming error.
    pub fn set_item_level(&mut self, level: u32) {
        assert!(
            item_level::ALLOWED.contains(&level),
            "item level {level} is not allowed"
        );
        if level == self.item_level {
            return;
        }
        self.item_level = level;
        self.reset();
    }

    /// Chooses the base item with the given identifier: the base and its pool are replaced,
    /// the item level is set back to the default, and the item becomes a new normal item of
    /// the base, as after [`Workbench::reset`] (ADR 0018, ADR 0019).
    ///
    /// # Panics
    /// If `crafting-core` has no such base item. The identifiers come from `crafting-core`, so
    /// that is a programming error.
    pub fn select(&mut self, base_id: &str) {
        self.base = base_item(base_id)
            .unwrap_or_else(|| panic!("crafting-core has no base item {base_id:?}"));
        self.item_level = item_level::DEFAULT;
        self.pool = base_pool(base_id);
        self.reset();
    }

    fn check(&self, currency: Currency) -> Result<(), CraftError> {
        match currency {
            Currency::OrbOfTransmutation => can_apply_orb_of_transmutation(&self.item),
            Currency::OrbOfAlteration => can_apply_orb_of_alteration(&self.item),
            Currency::OrbOfAugmentation => can_apply_orb_of_augmentation(&self.item),
            Currency::RegalOrb => can_apply_regal_orb(&self.item),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item_level;
    use crafting_core::{Rarity, Slot};

    const TRANSMUTATION: Currency = Currency::OrbOfTransmutation;
    const ALTERATION: Currency = Currency::OrbOfAlteration;

    #[test]
    fn starts_with_a_normal_item_of_the_start_base_item() {
        let workbench = Workbench::new(1);
        assert_eq!(workbench.item().base.id, START_ITEM_ID);
        assert_eq!(workbench.item().rarity, Rarity::Normal);
        assert!(workbench.applicable(TRANSMUTATION));
        assert!(!workbench.applicable(ALTERATION));
    }

    #[test]
    fn the_pool_is_the_pool_of_the_base_and_the_item_has_the_default_item_level() {
        let workbench = Workbench::new(1);
        let pool = mod_pool(START_ITEM_ID).expect("the start base item has a pool");
        assert_eq!(workbench.item_level(), item_level::DEFAULT);
        assert_eq!(workbench.item().item_level, item_level::DEFAULT);
        assert_eq!(workbench.pool, pool);
    }

    /// A base with a damage type lock (a Breach ring).
    const LOCKED_BASE_ID: &str = "Metadata/Items/Rings/RingB1";

    #[test]
    fn the_pool_of_a_locked_base_is_passed_whole_and_the_orbs_leave_out_the_blocked_mods() {
        let all = mod_pool(LOCKED_BASE_ID).expect("the base has a pool");
        let mut workbench = Workbench::new(1);
        workbench.select(LOCKED_BASE_ID);
        assert_eq!(workbench.pool, all);
        // This base is locked to cold: no mod rolled on it has a tag of another damage type.
        const OTHER_DAMAGE_TYPES: [&str; 4] = ["physical", "lightning", "fire", "chaos"];
        for _ in 0..300 {
            workbench.reset();
            workbench.apply(TRANSMUTATION).unwrap();
            workbench.apply(ALTERATION).unwrap();
            for explicit in workbench
                .item()
                .prefixes
                .iter()
                .chain(&workbench.item().suffixes)
            {
                assert!(explicit
                    .modifier
                    .tags
                    .iter()
                    .all(|tag| !OTHER_DAMAGE_TYPES.contains(&tag.as_str())));
            }
        }
    }

    #[test]
    fn the_transmutation_makes_the_item_magic_and_disables_itself() {
        let mut workbench = Workbench::new(1);
        let implicits = workbench.item().implicits.clone();
        assert_eq!(workbench.apply(TRANSMUTATION), Ok(()));
        assert_eq!(workbench.item().rarity, Rarity::Magic);
        assert_eq!(workbench.item().implicits, implicits);
        assert!(!workbench.item().prefixes.is_empty() || !workbench.item().suffixes.is_empty());
        assert!(!workbench.applicable(TRANSMUTATION));
        assert!(workbench.applicable(ALTERATION));
    }

    #[test]
    fn the_transmutation_on_a_magic_item_fails_and_changes_nothing() {
        let mut workbench = Workbench::new(1);
        workbench.apply(TRANSMUTATION).unwrap();
        let magic = workbench.item().clone();
        assert_eq!(
            workbench.apply(TRANSMUTATION),
            Err(CraftError::ItemNotNormal)
        );
        assert_eq!(workbench.item(), &magic);
    }

    #[test]
    fn the_alteration_on_a_normal_item_fails_and_changes_nothing() {
        let mut workbench = Workbench::new(1);
        let normal = workbench.item().clone();
        assert_eq!(workbench.apply(ALTERATION), Err(CraftError::ItemNotMagic));
        assert_eq!(workbench.item(), &normal);
    }

    #[test]
    fn the_alteration_keeps_the_item_magic_and_applicable() {
        let mut workbench = Workbench::new(1);
        workbench.apply(TRANSMUTATION).unwrap();
        let implicits = workbench.item().implicits.clone();
        let mut seen = vec![workbench.item().clone()];
        for _ in 0..20 {
            assert_eq!(workbench.apply(ALTERATION), Ok(()));
            assert_eq!(workbench.item().rarity, Rarity::Magic);
            assert_eq!(workbench.item().implicits, implicits);
            assert!(workbench.applicable(ALTERATION));
            assert!(!workbench.applicable(TRANSMUTATION));
            seen.push(workbench.item().clone());
        }
        assert!(seen.iter().any(|item| item != &seen[0]));
    }

    const AUGMENTATION: Currency = Currency::OrbOfAugmentation;

    /// A workbench with a magic item that has one mod and room for another.
    fn magic_with_room() -> Workbench {
        let mut workbench = Workbench::new(1);
        workbench.apply(TRANSMUTATION).unwrap();
        while workbench.item().prefixes.len() + workbench.item().suffixes.len() != 1 {
            workbench.apply(ALTERATION).unwrap();
        }
        workbench
    }

    #[test]
    fn the_augmentation_on_a_normal_item_fails_and_changes_nothing() {
        let mut workbench = Workbench::new(1);
        let normal = workbench.item().clone();
        assert!(!workbench.applicable(AUGMENTATION));
        assert_eq!(
            workbench.tooltip(AUGMENTATION),
            AUGMENTATION.failure_message(CraftError::ItemNotMagic)
        );
        assert_eq!(workbench.apply(AUGMENTATION), Err(CraftError::ItemNotMagic));
        assert_eq!(workbench.item(), &normal);
    }

    #[test]
    fn the_augmentation_adds_one_mod_and_then_has_no_space() {
        let mut workbench = magic_with_room();
        let before = workbench.item().clone();
        assert!(workbench.applicable(AUGMENTATION));
        assert_eq!(workbench.tooltip(AUGMENTATION), AUGMENTATION.description());
        assert_eq!(workbench.apply(AUGMENTATION), Ok(()));
        let item = workbench.item();
        assert_eq!(item.rarity, Rarity::Magic);
        assert_eq!(item.prefixes.len() + item.suffixes.len(), 2);
        assert!(item.prefixes.starts_with(&before.prefixes));
        assert!(item.suffixes.starts_with(&before.suffixes));

        let full = workbench.item().clone();
        assert!(!workbench.applicable(AUGMENTATION));
        assert_eq!(
            workbench.tooltip(AUGMENTATION),
            AUGMENTATION.failure_message(CraftError::NoSpaceForMods)
        );
        assert_eq!(
            workbench.apply(AUGMENTATION),
            Err(CraftError::NoSpaceForMods)
        );
        assert_eq!(workbench.item(), &full);
    }

    #[test]
    fn the_augmentation_with_room_but_no_valid_mod_fails_and_changes_nothing() {
        let mut workbench = magic_with_room();
        workbench.pool.clear();
        let before = workbench.item().clone();
        assert!(workbench.applicable(AUGMENTATION));
        assert_eq!(workbench.apply(AUGMENTATION), Err(CraftError::NoValidMod));
        assert_eq!(workbench.item(), &before);
    }

    const REGAL: Currency = Currency::RegalOrb;

    #[test]
    fn the_regal_orb_on_a_normal_item_fails_and_changes_nothing() {
        let mut workbench = Workbench::new(1);
        let normal = workbench.item().clone();
        assert!(!workbench.applicable(REGAL));
        assert_eq!(
            workbench.tooltip(REGAL),
            REGAL.failure_message(CraftError::ItemNotMagic)
        );
        assert_eq!(workbench.apply(REGAL), Err(CraftError::ItemNotMagic));
        assert_eq!(workbench.item(), &normal);
    }

    #[test]
    fn the_regal_orb_makes_a_magic_item_rare_and_then_does_not_apply() {
        let mut workbench = magic_with_room();
        let before = workbench.item().clone();
        assert!(workbench.applicable(REGAL));
        assert_eq!(workbench.tooltip(REGAL), REGAL.description());
        assert_eq!(workbench.apply(REGAL), Ok(()));
        let item = workbench.item();
        assert_eq!(item.rarity, Rarity::Rare);
        assert_eq!(item.name(), item.base.name);
        assert_eq!(
            item.prefixes.len() + item.suffixes.len(),
            before.prefixes.len() + before.suffixes.len() + 1
        );
        assert!(item.prefixes.starts_with(&before.prefixes));
        assert!(item.suffixes.starts_with(&before.suffixes));

        let rare = workbench.item().clone();
        assert!(!workbench.applicable(REGAL));
        assert_eq!(
            workbench.tooltip(REGAL),
            REGAL.failure_message(CraftError::ItemNotMagic)
        );
        assert_eq!(workbench.apply(REGAL), Err(CraftError::ItemNotMagic));
        assert_eq!(workbench.item(), &rare);
    }

    #[test]
    fn the_other_orbs_do_not_apply_to_a_rare_item() {
        let mut workbench = magic_with_room();
        workbench.apply(REGAL).unwrap();
        let rare = workbench.item().clone();
        for (currency, error) in [
            (TRANSMUTATION, CraftError::ItemNotNormal),
            (ALTERATION, CraftError::ItemNotMagic),
            (AUGMENTATION, CraftError::ItemNotMagic),
        ] {
            assert!(!workbench.applicable(currency));
            assert_eq!(workbench.tooltip(currency), currency.failure_message(error));
            assert_eq!(workbench.apply(currency), Err(error));
            assert_eq!(workbench.item(), &rare);
        }
    }

    #[test]
    fn the_regal_orb_with_room_but_no_valid_mod_fails_and_the_item_stays_magic() {
        let mut workbench = magic_with_room();
        workbench.pool.clear();
        let before = workbench.item().clone();
        assert!(workbench.applicable(REGAL));
        assert_eq!(workbench.apply(REGAL), Err(CraftError::NoValidMod));
        assert_eq!(workbench.item(), &before);
    }

    #[test]
    fn the_tooltip_is_the_description_while_the_currency_applies() {
        let mut workbench = Workbench::new(1);
        assert_eq!(
            workbench.tooltip(TRANSMUTATION),
            TRANSMUTATION.description()
        );
        workbench.apply(TRANSMUTATION).unwrap();
        assert_eq!(workbench.tooltip(ALTERATION), ALTERATION.description());
    }

    #[test]
    fn the_tooltip_is_the_failure_message_while_the_currency_does_not_apply() {
        let mut workbench = Workbench::new(1);
        assert_eq!(
            workbench.tooltip(ALTERATION),
            ALTERATION.failure_message(CraftError::ItemNotMagic)
        );
        workbench.apply(TRANSMUTATION).unwrap();
        assert_eq!(
            workbench.tooltip(TRANSMUTATION),
            TRANSMUTATION.failure_message(CraftError::ItemNotNormal)
        );
    }

    #[test]
    fn a_failure_has_the_message_of_its_currency_for_its_reason() {
        for (currency, error) in [
            (TRANSMUTATION, CraftError::ItemNotNormal),
            (ALTERATION, CraftError::ItemNotMagic),
        ] {
            assert_eq!(
                Workbench::failure_message(currency, error),
                currency.failure_message(error)
            );
        }
    }

    #[test]
    fn reset_restores_a_normal_item_and_the_applicability() {
        let mut workbench = Workbench::new(1);
        let start = workbench.item().clone();
        workbench.apply(TRANSMUTATION).unwrap();
        workbench.reset();
        assert_eq!(workbench.item(), &start);
        assert!(workbench.applicable(TRANSMUTATION));
        assert!(!workbench.applicable(ALTERATION));
    }

    #[test]
    fn every_click_continues_the_sequence_of_one_generator() {
        let mut workbench = Workbench::new(1);
        let mut results = Vec::new();
        for _ in 0..20 {
            workbench.apply(TRANSMUTATION).unwrap();
            results.push(workbench.item().clone());
            workbench.reset();
        }
        assert!(results.iter().any(|item| item != &results[0]));
    }

    #[test]
    fn the_same_seed_gives_the_same_results() {
        let run = |seed| {
            let mut workbench = Workbench::new(seed);
            workbench.apply(TRANSMUTATION).unwrap();
            workbench.apply(ALTERATION).unwrap();
            workbench.item().clone()
        };
        assert_eq!(run(5), run(5));
    }

    #[test]
    fn different_seeds_can_give_different_results() {
        let run = |seed| {
            let mut workbench = Workbench::new(seed);
            workbench.apply(TRANSMUTATION).unwrap();
            workbench.item().clone()
        };
        let first = run(0);
        assert!((1..50).any(|seed| run(seed) != first));
    }

    /// The base items of the ring slot other than the start base item.
    fn other_bases() -> Vec<BaseItem> {
        Slot::Ring
            .base_items()
            .into_iter()
            .filter(|base| base.id != START_ITEM_ID)
            .collect()
    }

    #[test]
    fn select_gives_a_normal_item_of_the_chosen_base_with_its_own_pool() {
        for base in other_bases() {
            let mut workbench = Workbench::new(1);
            workbench.apply(TRANSMUTATION).unwrap();
            workbench.select(&base.id);
            assert_eq!(workbench.item().base, base, "{}", base.id);
            assert_eq!(workbench.item().rarity, Rarity::Normal, "{}", base.id);
            assert_eq!(workbench.item().implicits.len(), base.implicits.len());
            assert_eq!(workbench.pool, base_pool(&base.id), "{}", base.id);
            assert!(workbench.applicable(TRANSMUTATION));
            assert!(!workbench.applicable(ALTERATION));
        }
    }

    #[test]
    fn select_uses_the_pool_of_the_chosen_base() {
        let own_mods =
            |id: &str| -> Vec<String> { base_pool(id).into_iter().map(|m| m.name).collect() };
        let start = own_mods(START_ITEM_ID);
        let base = other_bases()
            .into_iter()
            .find(|base| own_mods(&base.id) != start)
            .expect("a ring base has a pool of its own");
        let mut workbench = Workbench::new(1);
        workbench.select(&base.id);
        let chosen = own_mods(&base.id);
        for _ in 0..200 {
            workbench.reset();
            workbench.apply(TRANSMUTATION).unwrap();
            let item = workbench.item();
            for explicit in item.prefixes.iter().chain(&item.suffixes) {
                assert!(chosen.contains(&explicit.name));
            }
        }
    }

    #[test]
    fn reset_after_select_keeps_the_chosen_base() {
        let base = other_bases().remove(0);
        let mut workbench = Workbench::new(1);
        workbench.select(&base.id);
        workbench.apply(TRANSMUTATION).unwrap();
        workbench.reset();
        assert_eq!(workbench.item().base, base);
        assert_eq!(workbench.item().rarity, Rarity::Normal);
    }

    #[test]
    #[should_panic(expected = "has no base item")]
    fn selecting_an_unknown_base_is_a_programming_error() {
        Workbench::new(1).select("does/not/exist");
    }

    #[test]
    fn without_any_valid_mod_the_currencies_make_the_item_magic_with_no_mods() {
        let mut workbench = Workbench::with_pool(1, Vec::new());
        assert_eq!(workbench.apply(TRANSMUTATION), Ok(()));
        assert_eq!(workbench.item().rarity, Rarity::Magic);
        assert!(workbench.item().prefixes.is_empty() && workbench.item().suffixes.is_empty());
        assert!(workbench.applicable(ALTERATION));

        let mut magic = Workbench::new(1);
        magic.apply(TRANSMUTATION).unwrap();
        magic.pool.clear();
        assert_eq!(magic.apply(ALTERATION), Ok(()));
        assert_eq!(magic.item().rarity, Rarity::Magic);
        assert!(magic.item().prefixes.is_empty() && magic.item().suffixes.is_empty());
        assert!(magic.applicable(ALTERATION));
    }

    /// A level at which the pool of the start base item is smaller than at the default level
    /// but not empty.
    const LOW_LEVEL: u32 = 10;

    #[test]
    fn the_low_level_leaves_out_some_mods_of_the_pool_but_not_all() {
        let pool = base_pool(START_ITEM_ID);
        let at_low_level = pool
            .iter()
            .filter(|definition| definition.definition.level <= LOW_LEVEL)
            .count();
        assert!(at_low_level > 0);
        assert!(at_low_level < pool.len());
    }

    #[test]
    fn set_item_level_sets_the_level_on_a_new_item_and_keeps_the_pool_of_the_base() {
        let mut workbench = Workbench::new(1);
        workbench.apply(TRANSMUTATION).unwrap();
        workbench.set_item_level(LOW_LEVEL);
        assert_eq!(workbench.item_level(), LOW_LEVEL);
        assert_eq!(workbench.item().item_level, LOW_LEVEL);
        assert_eq!(workbench.pool, base_pool(START_ITEM_ID));
        assert_eq!(workbench.item().base, workbench.base);
        assert_eq!(workbench.item().rarity, Rarity::Normal);
        assert!(workbench.applicable(TRANSMUTATION));
        assert!(!workbench.applicable(ALTERATION));
    }

    #[test]
    fn the_orbs_roll_only_mods_up_to_the_chosen_item_level() {
        let mut workbench = Workbench::new(1);
        workbench.set_item_level(LOW_LEVEL);
        for _ in 0..200 {
            workbench.reset();
            workbench.apply(TRANSMUTATION).unwrap();
            workbench.apply(ALTERATION).unwrap();
            let item = workbench.item();
            for explicit in item.prefixes.iter().chain(&item.suffixes) {
                assert!(explicit.modifier.level <= LOW_LEVEL);
            }
        }
    }

    #[test]
    fn raising_the_item_level_again_allows_the_higher_mods() {
        let mut workbench = Workbench::new(1);
        workbench.set_item_level(LOW_LEVEL);
        workbench.set_item_level(item_level::DEFAULT);
        assert_eq!(workbench.item().item_level, item_level::DEFAULT);
        let highest = (0..300)
            .map(|_| {
                workbench.reset();
                workbench.apply(TRANSMUTATION).unwrap();
                let item = workbench.item();
                item.prefixes
                    .iter()
                    .chain(&item.suffixes)
                    .map(|explicit| explicit.modifier.level)
                    .max()
                    .unwrap_or(0)
            })
            .max()
            .unwrap();
        assert!(highest > LOW_LEVEL);
    }

    #[test]
    fn the_same_item_level_is_no_change_and_the_item_stays() {
        let mut workbench = Workbench::new(1);
        workbench.apply(TRANSMUTATION).unwrap();
        let magic = workbench.item().clone();
        workbench.set_item_level(item_level::DEFAULT);
        assert_eq!(workbench.item(), &magic);

        workbench.set_item_level(LOW_LEVEL);
        workbench.apply(TRANSMUTATION).unwrap();
        let magic = workbench.item().clone();
        workbench.set_item_level(LOW_LEVEL);
        assert_eq!(workbench.item(), &magic);
    }

    #[test]
    fn reset_keeps_the_item_level_and_the_pool() {
        let mut workbench = Workbench::new(1);
        workbench.set_item_level(LOW_LEVEL);
        workbench.apply(TRANSMUTATION).unwrap();
        workbench.reset();
        assert_eq!(workbench.item_level(), LOW_LEVEL);
        assert_eq!(workbench.item().item_level, LOW_LEVEL);
        assert_eq!(workbench.pool, base_pool(START_ITEM_ID));
    }

    #[test]
    fn select_sets_the_item_level_back_to_the_default() {
        for base in other_bases() {
            let mut workbench = Workbench::new(1);
            workbench.set_item_level(LOW_LEVEL);
            workbench.select(&base.id);
            assert_eq!(workbench.item_level(), item_level::DEFAULT, "{}", base.id);
            assert_eq!(
                workbench.item().item_level,
                item_level::DEFAULT,
                "{}",
                base.id
            );
            assert_eq!(workbench.pool, base_pool(&base.id), "{}", base.id);
        }
    }

    #[test]
    fn the_item_level_applies_to_the_chosen_base() {
        let base = other_bases().remove(0);
        let mut workbench = Workbench::new(1);
        workbench.select(&base.id);
        workbench.set_item_level(LOW_LEVEL);
        assert_eq!(workbench.item().base, base);
        assert_eq!(workbench.item().item_level, LOW_LEVEL);
        assert_eq!(workbench.pool, base_pool(&base.id));
    }

    #[test]
    #[should_panic(expected = "is not allowed")]
    fn an_item_level_outside_the_range_is_a_programming_error() {
        Workbench::new(1).set_item_level(101);
    }
}
