//! The item shown on the start page, as plain data so it can be tested without a browser.
//!
//! The item itself comes from `crafting-core`; this module only picks the labels from the
//! translations and takes the game texts (name, class, mod texts) over unchanged.

use crate::i18n::Translations;
use crafting_core::{Item, Mod, Rarity};

/// All visible texts of one item.
#[derive(Debug, PartialEq, Eq)]
pub struct ItemView {
    pub name: String,
    pub item_class: String,
    pub rarity: String,
    pub requires_level: String,
    /// One entry per line of the implicit mods.
    pub implicits: Vec<String>,
    /// One entry per line of the prefix, then of the suffix.
    pub explicits: Vec<String>,
}

impl ItemView {
    pub fn new(translations: &Translations, item: &Item) -> Self {
        Self {
            name: item.name(),
            item_class: item.base.item_class.clone(),
            rarity: translations.text(rarity_key(item.rarity)).to_owned(),
            requires_level: translations.format(
                "item.requires_level",
                &[("level", &item.base.required_level.to_string())],
            ),
            implicits: item.implicits.iter().flat_map(lines).collect(),
            explicits: item
                .prefix
                .iter()
                .chain(&item.suffix)
                .flat_map(|explicit| lines(&explicit.modifier))
                .collect(),
        }
    }
}

/// The lines of the display text of a mod; a text with several lines separates them with
/// line breaks.
fn lines(modifier: &Mod) -> Vec<String> {
    modifier.display_text().lines().map(str::to_owned).collect()
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::workbench::Workbench;
    use crafting_core::{
        AffixType, Currency, ExplicitModDefinition, ModDefinition, StatDefinition,
    };

    #[test]
    fn every_rarity_has_a_translation() {
        let t = Translations::english();
        for rarity in [Rarity::Normal, Rarity::Magic, Rarity::Rare, Rarity::Unique] {
            assert!(!t.text(rarity_key(rarity)).is_empty());
        }
    }

    #[test]
    fn view_takes_game_texts_from_the_item_and_labels_from_translations() {
        let t = Translations::english();
        let workbench = Workbench::new(1);
        let item = workbench.item();
        let view = ItemView::new(&t, item);
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
        assert!(view.explicits.is_empty());
    }

    #[test]
    fn view_of_a_magic_item_shows_name_rarity_and_mod_lines() {
        let t = Translations::english();
        let mut workbench = Workbench::new(1);
        let implicits = ItemView::new(&t, workbench.item()).implicits;
        workbench.apply(Currency::OrbOfTransmutation).unwrap();
        let item = workbench.item();
        let view = ItemView::new(&t, item);

        assert_eq!(view.name, item.name());
        assert_eq!(view.rarity, t.text("item.rarity.magic"));
        assert_eq!(view.implicits, implicits);
        let expected: Vec<String> = item
            .prefix
            .iter()
            .chain(&item.suffix)
            .flat_map(|explicit| {
                explicit
                    .modifier
                    .display_text()
                    .split('\n')
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .collect();
        assert!(!expected.is_empty());
        assert_eq!(view.explicits, expected);
        assert!(view.explicits.iter().all(|line| !line.contains('{')));
    }

    #[test]
    fn a_mod_with_two_lines_gives_one_entry_per_line() {
        let stat = |id: &str, value: i32| StatDefinition {
            id: id.to_owned(),
            min: value,
            max: value,
            global: false,
        };
        let pool = vec![ExplicitModDefinition {
            name: "Invented".to_owned(),
            affix_type: AffixType::Suffix,
            level: 1,
            weight: 1,
            definition: ModDefinition {
                family: "Invented".to_owned(),
                tags: Vec::new(),
                stats: vec![stat("second stat", 2), stat("first stat", 1)],
                text: "first {a}
second {b}"
                    .to_owned(),
                placeholders: BTreeMap::from([
                    ("a".to_owned(), "first stat".to_owned()),
                    ("b".to_owned(), "second stat".to_owned()),
                ]),
            },
        }];
        let mut workbench = Workbench::with_pool(1, pool);
        workbench.apply(Currency::OrbOfTransmutation).unwrap();
        let view = ItemView::new(&Translations::english(), workbench.item());
        assert_eq!(view.explicits, ["first 1", "second 2"]);
    }
}
