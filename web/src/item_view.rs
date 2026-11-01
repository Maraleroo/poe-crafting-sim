//! The item shown on the start page, as plain data so it can be tested without a browser.
//!
//! The item itself comes from `crafting-core`; this module only picks the labels from the
//! translations and takes the game texts (name, class, mod texts) over unchanged.

use crate::i18n::Translations;
use crafting_core::{Item, Mod, ModBlock, ModKind, Rarity, TextPart};

/// All visible texts of one item.
#[derive(Debug, PartialEq, Eq)]
pub struct ItemView {
    pub name: String,
    pub item_class: String,
    pub rarity: String,
    /// The CSS class for the rarity. It is neither visible text nor a translation.
    pub rarity_class: &'static str,
    /// The line for the level requirement; none if the item shows no requirement.
    pub requires_level: Option<String>,
    /// One entry per line of the implicit mods.
    pub implicits: Vec<String>,
    /// One entry per line of the explicit mods in the normal view of the game: sorted by their
    /// stats, with the lines of the same stats merged (ADR 0023). They come from
    /// `crafting-core`.
    pub explicits: Vec<String>,
    /// One entry per mod of the advanced view (ADR 0024). They come from `crafting-core`.
    pub blocks: Vec<BlockView>,
}

/// All visible texts of one mod in the advanced view.
#[derive(Debug, PartialEq, Eq)]
pub struct BlockView {
    /// The CSS class for the kind of mod. It is neither visible text nor a translation.
    pub kind_class: &'static str,
    /// The short label that is shown on the left of the block.
    pub short_label: String,
    /// The name of the kind, for screen readers and as a tooltip.
    pub kind_name: String,
    /// The tier box; none for an implicit.
    pub tier: Option<String>,
    /// One entry per line; the range of a value is a part of its own.
    pub lines: Vec<Vec<TextPart>>,
    /// The name of a prefix or suffix, a game text.
    pub name: Option<String>,
    /// The separator between name and tags; none unless both are shown.
    pub separator: Option<String>,
    /// The tags, a game text; empty if the mod shows none.
    pub tags: String,
    /// The mark of a mod whose values the base increases.
    pub increase: Option<String>,
}

impl BlockView {
    fn new(translations: &Translations, block: &ModBlock) -> Self {
        let (kind_class, short_key, name_key) = match block.kind {
            ModKind::Implicit => ("implicit", "mod.label.implicit", "mod.kind.implicit"),
            ModKind::Prefix => ("prefix", "mod.label.prefix", "mod.kind.prefix"),
            ModKind::Suffix => ("suffix", "mod.label.suffix", "mod.kind.suffix"),
        };
        Self {
            kind_class,
            short_label: translations.text(short_key).to_owned(),
            kind_name: translations.text(name_key).to_owned(),
            tier: block
                .tier
                .map(|tier| translations.format("mod.tier", &[("tier", &tier.to_string())])),
            lines: block.lines.clone(),
            name: block.name.clone(),
            separator: (block.name.is_some() && !block.tags.is_empty())
                .then(|| translations.text("mod.separator").to_owned()),
            tags: block.tags.clone(),
            increase: block.magnitude_increase.map(|percent| {
                translations.format("mod.increase", &[("percent", &percent.to_string())])
            }),
        }
    }
}

impl ItemView {
    pub fn new(translations: &Translations, item: &Item) -> Self {
        Self {
            name: item.name(),
            item_class: item.base.item_class.clone(),
            rarity: translations.text(rarity_key(item.rarity)).to_owned(),
            rarity_class: rarity_class(item.rarity),
            requires_level: item.required_level().map(|level| {
                translations.format("item.requires_level", &[("level", &level.to_string())])
            }),
            implicits: item.implicits.iter().flat_map(implicit_lines).collect(),
            explicits: item.explicit_texts(),
            blocks: item
                .advanced_mods()
                .iter()
                .map(|block| BlockView::new(translations, block))
                .collect(),
        }
    }
}

/// The lines of the display text of an implicit mod; a text with several lines separates them
/// with line breaks.
fn implicit_lines(modifier: &Mod) -> Vec<String> {
    text_lines(&modifier.display_text())
}

fn text_lines(text: &str) -> Vec<String> {
    text.lines().map(str::to_owned).collect()
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

/// CSS class that the stylesheet uses to colour an item by its rarity.
fn rarity_class(rarity: Rarity) -> &'static str {
    match rarity {
        Rarity::Normal => "normal",
        Rarity::Magic => "magic",
        Rarity::Rare => "rare",
        Rarity::Unique => "unique",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workbench::Workbench;
    use crafting_core::{
        mod_pool, AffixType, BaseItem, Currency, ExplicitMod, ExplicitModDefinition, SeededRng,
        Slot,
    };

    /// A magic item of `base` with the given mods of its pool.
    fn magic_item_with(base: &BaseItem, mods: &[&ExplicitModDefinition]) -> Item {
        let mut rng = SeededRng::new(1);
        let mut item = Item::new(base, &mut rng);
        item.rarity = Rarity::Magic;
        for definition in mods {
            let explicit = ExplicitMod {
                name: definition.name.clone(),
                affix_type: definition.affix_type,
                modifier: Mod::roll(&definition.definition, &mut rng),
            };
            match explicit.affix_type {
                AffixType::Prefix => item.prefixes.push(explicit),
                AffixType::Suffix => item.suffixes.push(explicit),
            }
        }
        item
    }

    /// The base item that the page starts with and its pool.
    fn start_pool() -> (BaseItem, Vec<ExplicitModDefinition>) {
        let base = Workbench::new(1).item().base.clone();
        let pool = mod_pool(&base.id).expect("crafting-core has the pool of the start base");
        (base, pool)
    }

    fn sides(
        pool: &[ExplicitModDefinition],
    ) -> (Vec<&ExplicitModDefinition>, Vec<&ExplicitModDefinition>) {
        let of = |affix_type: AffixType| {
            pool.iter()
                .filter(|m| m.affix_type == affix_type)
                .collect::<Vec<_>>()
        };
        (of(AffixType::Prefix), of(AffixType::Suffix))
    }

    #[test]
    fn every_rarity_has_a_translation() {
        let t = Translations::english();
        for rarity in [Rarity::Normal, Rarity::Magic, Rarity::Rare, Rarity::Unique] {
            assert!(!t.text(rarity_key(rarity)).is_empty());
        }
    }

    #[test]
    fn the_rarity_class_follows_the_rarity_of_the_item() {
        let mut workbench = Workbench::new(1);
        let t = Translations::english();
        assert_eq!(ItemView::new(&t, workbench.item()).rarity_class, "normal");
        workbench.apply(Currency::OrbOfTransmutation).unwrap();
        assert_eq!(ItemView::new(&t, workbench.item()).rarity_class, "magic");
    }

    #[test]
    fn the_four_rarities_have_different_non_empty_classes() {
        let classes: Vec<&str> = [Rarity::Normal, Rarity::Magic, Rarity::Rare, Rarity::Unique]
            .into_iter()
            .map(rarity_class)
            .collect();
        assert!(classes.iter().all(|class| !class.is_empty()));
        let distinct: std::collections::BTreeSet<&str> = classes.iter().copied().collect();
        assert_eq!(distinct.len(), classes.len());
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
        assert_eq!(item.required_level(), None);
        assert_eq!(
            view.requires_level, None,
            "a normal item of a base without a requirement shows none"
        );
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
        assert!(!view.explicits.is_empty());
        assert_eq!(view.explicits, item.explicit_texts());
        assert!(view.explicits.iter().all(|line| !line.contains('{')));
    }

    #[test]
    fn a_magic_item_without_mods_shows_only_its_implicits() {
        let t = Translations::english();
        let mut workbench = Workbench::with_pool(1, Vec::new());
        workbench.apply(Currency::OrbOfTransmutation).unwrap();
        let item = workbench.item();
        let view = ItemView::new(&t, item);
        assert_eq!(view.rarity, t.text("item.rarity.magic"));
        assert_eq!(view.name, item.base.name);
        assert_eq!(
            view.implicits,
            [item.implicits[0].display_text()],
            "the view shows the rolled implicit"
        );
        assert!(view.explicits.is_empty());
    }

    #[test]
    fn view_shows_the_explicit_lines_sorted_by_their_stats_not_prefixes_before_suffixes() {
        // Criterion 2 of AB#258 replaces criterion 6 of AB#189 (prefixes, then suffixes). The
        // pair of a prefix and a suffix is found by what crafting-core shows: the suffix line
        // first. Each line is the text of its mod alone, so the expectation is no copy of the
        // result.
        let (base, pool) = start_pool();
        let (prefixes, suffixes) = sides(&pool);
        let found = prefixes.iter().find_map(|prefix| {
            suffixes.iter().find_map(|suffix| {
                let item = magic_item_with(&base, &[prefix, suffix]);
                let lines = [
                    item.explicit_text(&item.suffixes[0]),
                    item.explicit_text(&item.prefixes[0]),
                ];
                (item.explicit_texts() == lines).then_some((item, lines))
            })
        });
        let (item, lines) = found.expect("a suffix whose line comes before the line of a prefix");
        let view = ItemView::new(&Translations::english(), &item);
        assert_eq!(view.explicits, lines);
    }

    #[test]
    fn view_shows_one_line_for_a_prefix_and_a_suffix_with_the_same_stat() {
        let (base, pool) = start_pool();
        let (prefixes, suffixes) = sides(&pool);
        let item = prefixes
            .iter()
            .find_map(|prefix| {
                suffixes.iter().find_map(|suffix| {
                    let item = magic_item_with(&base, &[prefix, suffix]);
                    (item.explicit_texts().len() == 1).then_some(item)
                })
            })
            .expect("a prefix and a suffix that show one line together");
        let view = ItemView::new(&Translations::english(), &item);
        assert_eq!(view.explicits, item.explicit_texts());
        assert_eq!(view.explicits.len(), 1);
    }

    #[test]
    fn view_keeps_the_implicits_apart_from_a_mod_with_the_same_stat() {
        // A ring base with a mod in its pool that shows a stat of one of its implicits
        let (base, shared) = Slot::Ring
            .base_items()
            .into_iter()
            .find_map(|base| {
                let stats: Vec<String> = base
                    .implicits
                    .iter()
                    .flat_map(|implicit| implicit.placeholders.values().cloned())
                    .collect();
                let shared = mod_pool(&base.id)
                    .expect("every base has a pool")
                    .into_iter()
                    .find(|m| {
                        m.definition
                            .placeholders
                            .values()
                            .any(|s| stats.contains(s))
                    })?;
                Some((base, shared))
            })
            .expect("a ring base whose pool has a mod with the stat of its implicit");
        let item = magic_item_with(&base, &[&shared]);
        let view = ItemView::new(&Translations::english(), &item);
        let implicits: Vec<String> = item.implicits.iter().flat_map(implicit_lines).collect();
        assert!(!implicits.is_empty());
        assert_eq!(view.implicits, implicits);
        let own = item.prefixes.iter().chain(&item.suffixes).next().unwrap();
        assert_eq!(view.explicits, text_lines(&item.explicit_text(own)));
    }

    #[test]
    fn view_shows_the_level_requirement_of_a_magic_item_with_a_high_mod() {
        // The pool holds only the mod with the highest level
        let (_, pool) = start_pool();
        let highest = pool
            .into_iter()
            .max_by_key(|m| m.definition.level)
            .expect("the pool has mods");
        let mut workbench = Workbench::with_pool(1, vec![highest]);
        workbench.apply(Currency::OrbOfTransmutation).unwrap();
        let level = workbench
            .item()
            .required_level()
            .expect("a high mod gives the item a level requirement");
        let t = Translations::english();
        let view = ItemView::new(&t, workbench.item());
        assert_eq!(
            view.requires_level,
            Some(t.format("item.requires_level", &[("level", &level.to_string())]))
        );
    }

    #[test]
    fn view_shows_the_increased_values_of_the_side_that_the_base_increases() {
        // This ring base increases the values of its suffixes by 25 % (ADR 0022)
        let mut workbench = Workbench::new(1);
        workbench.select("Metadata/Items/Rings/RingE1");
        let base = workbench.item().base.clone();
        let pool = mod_pool(&base.id).expect("every base has a pool");
        // A prefix and a suffix with one stat each, no conversion, and different stats
        let (prefixes, suffixes) = sides(&pool);
        let plain = |m: &&ExplicitModDefinition| -> bool {
            m.definition.stats.len() == 1
                && m.definition.conversions.is_empty()
                && m.definition.text.matches('{').count() == 1
        };
        let (prefix, suffix) = prefixes
            .iter()
            .copied()
            .filter(plain)
            .find_map(|prefix| {
                suffixes
                    .iter()
                    .copied()
                    .filter(plain)
                    .find(|s| s.definition.stats[0].id != prefix.definition.stats[0].id)
                    .map(|suffix| (prefix, suffix))
            })
            .expect("a prefix and a suffix with different stats");
        let mut item = magic_item_with(&base, &[prefix, suffix]);
        item.prefixes[0].modifier.stats[0].value = 40;
        item.suffixes[0].modifier.stats[0].value = 40;
        // The prefix keeps its 40, the suffix shows 50
        let mut increased = item.suffixes[0].modifier.clone();
        increased.stats[0].value = 50;
        let mut expected = vec![
            item.prefixes[0].modifier.display_text(),
            increased.display_text(),
        ];
        let mut shown = ItemView::new(&Translations::english(), &item).explicits;
        // The order of the two lines is not the subject of this test
        expected.sort();
        shown.sort();
        assert_eq!(shown, expected);
    }

    #[test]
    fn a_mod_with_two_lines_gives_one_entry_per_line() {
        let two_lines = Slot::Ring
            .base_items()
            .into_iter()
            .flat_map(|base| mod_pool(&base.id).expect("every base has a pool"))
            .find(|m| m.definition.text.contains('\n'))
            .expect("a mod with two lines");
        let mut workbench = Workbench::with_pool(1, vec![two_lines.clone()]);
        workbench.apply(Currency::OrbOfTransmutation).unwrap();
        let view = ItemView::new(&Translations::english(), workbench.item());
        assert_eq!(
            view.explicits.len(),
            two_lines.definition.text.lines().count()
        );
        assert!(view.explicits.iter().all(|line| !line.contains('\n')));
    }

    #[test]
    fn the_blocks_come_from_crafting_core_with_texts_from_the_translations() {
        let t = Translations::english();
        let (base, pool) = start_pool();
        let (prefixes, suffixes) = sides(&pool);
        let item = magic_item_with(&base, &[prefixes[0], suffixes[0]]);
        let view = ItemView::new(&t, &item);
        let blocks = item.advanced_mods();
        assert_eq!(view.blocks.len(), blocks.len());
        let kinds: Vec<&str> = view.blocks.iter().map(|b| b.kind_class).collect();
        assert_eq!(kinds, ["implicit", "prefix", "suffix"]);
        let labels: Vec<(&str, &str)> = view
            .blocks
            .iter()
            .map(|b| (b.short_label.as_str(), b.kind_name.as_str()))
            .collect();
        assert_eq!(
            labels,
            [
                (t.text("mod.label.implicit"), t.text("mod.kind.implicit")),
                (t.text("mod.label.prefix"), t.text("mod.kind.prefix")),
                (t.text("mod.label.suffix"), t.text("mod.kind.suffix")),
            ]
        );
        for (shown, block) in view.blocks.iter().zip(&blocks) {
            assert_eq!(shown.lines, block.lines);
            assert_eq!(shown.name, block.name);
            assert_eq!(shown.tags, block.tags);
            assert_eq!(
                shown.tier,
                block
                    .tier
                    .map(|tier| t.format("mod.tier", &[("tier", &tier.to_string())]))
            );
        }
        assert_eq!(view.blocks[0].tier, None);
        assert_eq!(view.blocks[0].name, None);
        assert_eq!(view.blocks[0].separator, None);
    }

    #[test]
    fn the_separator_is_shown_only_between_a_name_and_tags() {
        let t = Translations::english();
        let (base, pool) = start_pool();
        let item = magic_item_with(&base, &pool.iter().take(1).collect::<Vec<_>>());
        let view = ItemView::new(&t, &item);
        let explicit = view.blocks.last().unwrap();
        assert!(explicit.name.is_some());
        assert_eq!(
            explicit.separator.is_some(),
            !explicit.tags.is_empty(),
            "a mod without tags has no separator"
        );
        if let Some(separator) = &explicit.separator {
            assert_eq!(separator, t.text("mod.separator"));
        }
    }

    #[test]
    fn a_block_of_an_increased_side_shows_the_increase_from_the_translations() {
        // This ring base increases the values of its suffixes by 25 %
        let mut workbench = Workbench::new(1);
        workbench.select("Metadata/Items/Rings/RingE1");
        let base = workbench.item().base.clone();
        let pool = mod_pool(&base.id).expect("every base has a pool");
        let (prefixes, suffixes) = sides(&pool);
        let item = magic_item_with(&base, &[prefixes[0], suffixes[0]]);
        let t = Translations::english();
        let view = ItemView::new(&t, &item);
        let increases: Vec<(&str, Option<&str>)> = view
            .blocks
            .iter()
            .map(|b| (b.kind_class, b.increase.as_deref()))
            .collect();
        let expected = t.format("mod.increase", &[("percent", "25")]);
        assert_eq!(
            increases,
            [
                ("implicit", None),
                ("prefix", None),
                ("suffix", Some(expected.as_str()))
            ]
        );
    }
}
