//! The state of the start page: the item on the workbench and the generator for its rolls.
//!
//! This is plain Rust so it can be tested without a browser (ADR 0010). All crafting rules
//! are in `crafting-core`; this only keeps the state and calls them.

use crafting_core::{
    apply_orb_of_transmutation, base_item, can_apply_orb_of_transmutation, explicit_mods, BaseItem,
    CraftError, ExplicitModDefinition, Item, SeededRng,
};

/// The base item on the workbench (a reference, not item data).
const START_ITEM_ID: &str = "Metadata/Items/Rings/Ring1";

/// The item, its base and the generator that serves every roll of the page.
pub struct Workbench {
    base: BaseItem,
    pool: Vec<ExplicitModDefinition>,
    item: Item,
    rng: SeededRng,
}

impl Workbench {
    /// A workbench with a new normal item of the start base item and the mod pool of the
    /// game data. The same seed gives the same sequence of results.
    pub fn new(seed: u64) -> Self {
        Self::with_pool(seed, explicit_mods())
    }

    /// Like [`Workbench::new`], but with the given mod pool.
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

    /// Whether the orb button can be used. The reason, if not, comes with a later story.
    pub fn orb_enabled(&self) -> bool {
        can_apply_orb_of_transmutation(&self.item).is_ok()
    }

    /// Applies the orb to the item. On an error the item stays unchanged.
    pub fn transmute(&mut self) -> Result<(), CraftError> {
        apply_orb_of_transmutation(&mut self.item, &self.pool, &mut self.rng)
    }

    /// Replaces the item with a new normal item of the same base.
    pub fn reset(&mut self) {
        self.item = Item::new(&self.base, &mut self.rng);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crafting_core::Rarity;

    #[test]
    fn starts_with_a_normal_item_of_the_start_base_item() {
        let workbench = Workbench::new(1);
        assert_eq!(workbench.item().base.id, START_ITEM_ID);
        assert_eq!(workbench.item().rarity, Rarity::Normal);
        assert!(workbench.orb_enabled());
    }

    #[test]
    fn the_orb_makes_the_item_magic_and_disables_the_orb() {
        let mut workbench = Workbench::new(1);
        let implicits = workbench.item().implicits.clone();
        assert_eq!(workbench.transmute(), Ok(()));
        assert_eq!(workbench.item().rarity, Rarity::Magic);
        assert_eq!(workbench.item().implicits, implicits);
        assert!(workbench.item().prefix.is_some() || workbench.item().suffix.is_some());
        assert!(!workbench.orb_enabled());
    }

    #[test]
    fn the_orb_on_a_magic_item_fails_and_changes_nothing() {
        let mut workbench = Workbench::new(1);
        workbench.transmute().unwrap();
        let magic = workbench.item().clone();
        assert_eq!(workbench.transmute(), Err(CraftError::ItemNotNormal));
        assert_eq!(workbench.item(), &magic);
    }

    #[test]
    fn reset_restores_a_normal_item_and_enables_the_orb() {
        let mut workbench = Workbench::new(1);
        let start = workbench.item().clone();
        workbench.transmute().unwrap();
        workbench.reset();
        assert_eq!(workbench.item(), &start);
        assert!(workbench.orb_enabled());
    }

    #[test]
    fn every_click_continues_the_sequence_of_one_generator() {
        let mut workbench = Workbench::new(1);
        let mut results = Vec::new();
        for _ in 0..20 {
            workbench.transmute().unwrap();
            results.push(workbench.item().clone());
            workbench.reset();
        }
        assert!(results.iter().any(|item| item != &results[0]));
    }

    #[test]
    fn the_same_seed_gives_the_same_results() {
        let run = |seed| {
            let mut workbench = Workbench::new(seed);
            workbench.transmute().unwrap();
            workbench.item().clone()
        };
        assert_eq!(run(5), run(5));
    }

    #[test]
    fn different_seeds_can_give_different_results() {
        let run = |seed| {
            let mut workbench = Workbench::new(seed);
            workbench.transmute().unwrap();
            workbench.item().clone()
        };
        let first = run(0);
        assert!((1..50).any(|seed| run(seed) != first));
    }

    #[test]
    fn without_any_valid_mod_the_orb_fails_and_changes_nothing() {
        let mut workbench = Workbench::with_pool(1, Vec::new());
        let before = workbench.item().clone();
        assert_eq!(workbench.transmute(), Err(CraftError::NoValidMod));
        assert_eq!(workbench.item(), &before);
        assert!(workbench.orb_enabled());
    }
}
