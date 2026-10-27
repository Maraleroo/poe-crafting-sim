//! The equipment slots of the inventory and the item classes that belong to them (ADR 0018).

use crate::data::{base_items, slot_item_classes};
use crate::model::BaseItem;

/// An equipment slot, as the game's inventory arranges them. A slot has no name here: the
/// page takes slot names from its translation files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    Weapon,
    Helmet,
    OffHand,
    BodyArmour,
    Amulet,
    Ring,
    Belt,
    Gloves,
    Boots,
}

impl Slot {
    /// All slots, in the order of the story.
    pub const ALL: [Slot; 9] = [
        Slot::Weapon,
        Slot::Helmet,
        Slot::OffHand,
        Slot::BodyArmour,
        Slot::Amulet,
        Slot::Ring,
        Slot::Belt,
        Slot::Gloves,
        Slot::Boots,
    ];

    /// The project identifier of the slot in lower snake case, e.g. `body_armour`. Not a game
    /// value.
    pub fn id(self) -> &'static str {
        match self {
            Slot::Weapon => "weapon",
            Slot::Helmet => "helmet",
            Slot::OffHand => "off_hand",
            Slot::BodyArmour => "body_armour",
            Slot::Amulet => "amulet",
            Slot::Ring => "ring",
            Slot::Belt => "belt",
            Slot::Gloves => "gloves",
            Slot::Boots => "boots",
        }
    }

    /// The item classes of the slot, as named in the game data (`data/item_slots.json`). Empty
    /// if the data has no entry for the slot.
    ///
    /// # Panics
    /// If the embedded data is invalid. That is a programming error, caught by the unit tests.
    pub fn item_classes(self) -> Vec<String> {
        slot_item_classes(self.id())
    }

    /// The base items of the slot: those whose item class is one of the slot's, in the order of
    /// `data/base_items.json`. A slot without base items is not available.
    ///
    /// # Panics
    /// If the embedded data is invalid. That is a programming error, caught by the unit tests.
    pub fn base_items(self) -> Vec<BaseItem> {
        let classes = self.item_classes();
        base_items()
            .into_iter()
            .filter(|base| classes.contains(&base.item_class))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn the_slot_ids_are_distinct() {
        let ids: BTreeSet<&str> = Slot::ALL.iter().map(|slot| slot.id()).collect();
        assert_eq!(ids.len(), Slot::ALL.len());
    }

    #[test]
    fn a_slot_without_an_entry_has_no_item_classes_and_no_bases() {
        assert!(Slot::Boots.item_classes().is_empty());
        assert!(Slot::Boots.base_items().is_empty());
    }

    #[test]
    fn every_base_belongs_to_exactly_one_slot() {
        let in_slots: usize = Slot::ALL.iter().map(|slot| slot.base_items().len()).sum();
        assert_eq!(in_slots, base_items().len());
    }
}
