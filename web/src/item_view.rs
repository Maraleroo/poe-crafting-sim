//! The item shown on the start page, as plain data so it can be tested without a browser.
//!
//! The item itself comes from `crafting-core`; this module only picks the labels from the
//! translations and takes the game texts (name, class, mod texts) over unchanged.

use crate::i18n::Translations;
use crafting_core::{base_item, Item, Mod, Rarity, SeededRng};

/// The base item shown on the start page (a reference, not item data).
const START_ITEM_ID: &str = "Metadata/Items/Rings/Ring1";

/// Seed for the rolls of the start item. All ranges of the start item are fixed, so one
/// seed is enough for now (ADR 0009).
const START_ITEM_SEED: u64 = 0;

/// All visible texts of one item.
#[derive(Debug, PartialEq, Eq)]
pub struct ItemView {
    pub name: String,
    pub item_class: String,
    pub rarity: String,
    pub requires_level: String,
    pub implicits: Vec<String>,
}

impl ItemView {
    pub fn new(translations: &Translations, item: &Item) -> Self {
        Self {
            name: item.base.name.clone(),
            item_class: item.base.item_class.clone(),
            rarity: translations.text(rarity_key(item.rarity)).to_owned(),
            requires_level: translations.format(
                "item.requires_level",
                &[("level", &item.base.required_level.to_string())],
            ),
            implicits: item.implicits.iter().map(Mod::display_text).collect(),
        }
    }
}

/// Translation key of the label for a rarity.
fn rarity_key(rarity: Rarity) -> &'static str {
    match rarity {
        Rarity::Normal => "item.rarity.normal",
        Rarity::Magic => "item.rarity.magic",
        Rarity::Rare => "item.rarity.rare",
        Rarity::Unique => "item.rarity.unique",
    }
}

/// The item the start page shows: a new normal item of the start base item.
pub fn start_item() -> Item {
    let base = base_item(START_ITEM_ID)
        .unwrap_or_else(|| panic!("crafting-core has no base item {START_ITEM_ID:?}"));
    Item::new(&base, &mut SeededRng::new(START_ITEM_SEED))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_rarity_has_a_translation() {
        let t = Translations::english();
        for rarity in [Rarity::Normal, Rarity::Magic, Rarity::Rare, Rarity::Unique] {
            assert!(!t.text(rarity_key(rarity)).is_empty());
        }
    }

    #[test]
    fn start_item_is_a_normal_item_of_the_start_base_item() {
        let item = start_item();
        assert_eq!(item.base.id, START_ITEM_ID);
        assert_eq!(item.rarity, Rarity::Normal);
    }

    #[test]
    fn view_takes_game_texts_from_the_item_and_labels_from_translations() {
        let t = Translations::english();
        let item = start_item();
        let view = ItemView::new(&t, &item);
        assert_eq!(view.name, item.base.name);
        assert_eq!(view.item_class, item.base.item_class);
        assert_eq!(view.rarity, t.text("item.rarity.normal"));
        let level = item.base.required_level.to_string();
        assert_eq!(
            view.requires_level,
            t.format("item.requires_level", &[("level", &level)])
        );
        assert!(view.requires_level.contains(&level));
        assert!(!view.requires_level.contains("{level}"));
        assert_eq!(
            view.implicits,
            [item.implicits[0].display_text()],
            "the view shows the rolled implicit"
        );
    }
}
