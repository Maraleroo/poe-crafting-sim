//! The state of the start page: the item on the workbench and the generator for its rolls.
//!
//! This is plain Rust so it can be tested without a browser (ADR 0010). All crafting rules
//! are in `crafting-core`; this only keeps the state and calls them.

use crafting_core::{
    apply_orb_of_alteration, apply_orb_of_transmutation, base_item, can_apply_orb_of_alteration,
    can_apply_orb_of_transmutation, mod_pool, valid_mods, BaseItem, CraftError, Currency,
    ExplicitModDefinition, Item, SeededRng,
};

/// The base item on the workbench (a reference, not item data).
const START_ITEM_ID: &str = "Metadata/Items/Rings/Ring1";

/// The item level of the page: the default of the UI (`poe-crafting-rules`, "Item level").
/// The page cannot change it yet (ADR 0013).
const ITEM_LEVEL: u32 = 100;

/// The item, its base and the generator that serves every roll of the page.
pub struct Workbench {
    base: BaseItem,
    pool: Vec<ExplicitModDefinition>,
    item: Item,
    rng: SeededRng,
}

impl Workbench {
    /// A workbench with a new normal item of the start base item and the valid mods of that
    /// base item's own pool for the item level of the page. The same seed gives the same
    /// sequence of results.
    pub fn new(seed: u64) -> Self {
        let pool = mod_pool(START_ITEM_ID)
            .unwrap_or_else(|| panic!("crafting-core has no pool for {START_ITEM_ID:?}"));
        Self::with_pool(seed, valid_mods(&pool, ITEM_LEVEL))
    }

    /// Like [`Workbench::new`], but with the given pool of valid mods.
    pub fn with_pool(seed: u64, pool: Vec<ExplicitModDefinition>) -> Self {
        let base = base_item(START_ITEM_ID)
            .unwrap_or_else(|| panic!("crafting-core has no base item {START_ITEM_ID:?}"));
        let mut rng = SeededRng::new(seed);
        let item = Item::new(&base, &mut rng);
        Self {
            base,
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
        }
    }

    /// Replaces the item with a new normal item of the same base.
    pub fn reset(&mut self) {
        self.item = Item::new(&self.base, &mut self.rng);
    }

    fn check(&self, currency: Currency) -> Result<(), CraftError> {
        match currency {
            Currency::OrbOfTransmutation => can_apply_orb_of_transmutation(&self.item),
            Currency::OrbOfAlteration => can_apply_orb_of_alteration(&self.item),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crafting_core::Rarity;

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
    fn the_pool_holds_the_valid_mods_for_the_item_level_of_the_page() {
        let workbench = Workbench::new(1);
        let pool = mod_pool(START_ITEM_ID).expect("the start base item has a pool");
        assert_eq!(workbench.pool, valid_mods(&pool, ITEM_LEVEL));
    }

    #[test]
    fn the_transmutation_makes_the_item_magic_and_disables_itself() {
        let mut workbench = Workbench::new(1);
        let implicits = workbench.item().implicits.clone();
        assert_eq!(workbench.apply(TRANSMUTATION), Ok(()));
        assert_eq!(workbench.item().rarity, Rarity::Magic);
        assert_eq!(workbench.item().implicits, implicits);
        assert!(workbench.item().prefix.is_some() || workbench.item().suffix.is_some());
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
            (TRANSMUTATION, CraftError::NoValidMod),
            (ALTERATION, CraftError::ItemNotMagic),
            (ALTERATION, CraftError::NoValidMod),
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

    #[test]
    fn without_any_valid_mod_the_currencies_fail_and_change_nothing() {
        let mut workbench = Workbench::with_pool(1, Vec::new());
        let before = workbench.item().clone();
        assert_eq!(workbench.apply(TRANSMUTATION), Err(CraftError::NoValidMod));
        assert_eq!(workbench.item(), &before);
        assert!(workbench.applicable(TRANSMUTATION));

        let mut magic = Workbench::new(1);
        magic.apply(TRANSMUTATION).unwrap();
        magic.pool.clear();
        let before = magic.item().clone();
        assert_eq!(magic.apply(ALTERATION), Err(CraftError::NoValidMod));
        assert_eq!(magic.item(), &before);
        assert!(magic.applicable(ALTERATION));
    }
}
