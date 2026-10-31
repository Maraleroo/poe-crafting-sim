//! Pure crafting logic for the Path of Exile 1 crafting simulator.
//!
//! This crate has no UI and no I/O. It returns structured results and reason codes,
//! never user-facing text. Game texts (item names, mod texts) are game data and pass
//! through unchanged (ADR 0008).

mod base_effects;
mod conversion;
mod craft;
mod currency;
mod damage_lock;
mod data;
mod model;
mod rng;
mod slot;
mod stat_order;

pub use conversion::Conversion;
pub use craft::{
    add_random_mod, apply_orb_of_alteration, apply_orb_of_augmentation, apply_orb_of_transmutation,
    can_apply_orb_of_alteration, can_apply_orb_of_augmentation, can_apply_orb_of_transmutation,
    valid_mods, valid_pool, CraftError,
};
pub use currency::{Currency, CurrencyDefinition};
pub use damage_lock::unblocked_mods;
pub use data::{base_item, base_items, currencies, explicit_mods, mod_pool};
pub use model::{
    AffixLimits, AffixType, BaseItem, ExplicitMod, ExplicitModDefinition, Item, Mod, ModDefinition,
    Rarity, Stat, StatDefinition, DEFAULT_ITEM_LEVEL,
};
pub use rng::{Rng, SeededRng};
pub use slot::Slot;

/// Version of the `crafting-core` package, for example `"0.1.0"`.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_major_minor_patch() {
        let parts: Vec<&str> = version().split('.').collect();
        assert_eq!(
            parts.len(),
            3,
            "expected MAJOR.MINOR.PATCH, got {}",
            version()
        );
        assert!(parts.iter().all(|p| p.parse::<u32>().is_ok()));
    }
}
