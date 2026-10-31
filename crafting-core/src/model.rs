//! Items, mods and stats, as defined by the game data and as rolled for an item.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::conversion::Conversion;
use crate::rng::{roll_in_range, Rng};

/// Rarity of an item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rarity {
    Normal,
    Magic,
    Rare,
    Unique,
}

/// A stat of a mod in the game data: its identifier and the range it rolls in.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct StatDefinition {
    /// Identifier as in the game data, e.g. `attack_minimum_added_physical_damage`.
    pub id: String,
    pub min: i32,
    pub max: i32,
}

/// A mod in the game data.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ModDefinition {
    pub family: String,
    /// Level of the mod. For an explicit mod it decides whether the mod can roll on an item
    /// (item level rule); for every mod it counts in the level requirement of the item.
    pub level: u32,
    pub tags: Vec<String>,
    /// One or more stats.
    pub stats: Vec<StatDefinition>,
    /// Display text with named placeholders such as `{minimum}`.
    pub text: String,
    /// Maps each placeholder name of `text` to the identifier of a stat.
    pub placeholders: BTreeMap<String, String>,
    /// Maps a stat identifier to the conversion the game applies for display. A stat that is
    /// not in the map is shown as rolled.
    pub conversions: BTreeMap<String, Conversion>,
}

impl ModDefinition {
    /// The display text of the definition without a rolled value (ADR 0018), as
    /// `poe-crafting-rules` defines it for suggestions and lists of bases: a stat with a range
    /// is shown as `(minimum-maximum)` with a normal hyphen, a fixed value alone, converted as
    /// the game does where the mod has a conversion for the stat. Line breaks stay, a text
    /// without placeholders is unchanged.
    ///
    /// # Panics
    /// If the rule does not define the text: a range of a stat with a conversion, or a range of
    /// negative values. Or if a placeholder names a stat the mod does not have. That is an
    /// error in the data.
    pub fn range_text(&self) -> String {
        self.try_range_text()
            .unwrap_or_else(|message| panic!("{message}"))
    }

    /// Like `range_text`, but returns the reason instead of panicking.
    pub(crate) fn try_range_text(&self) -> Result<String, String> {
        let mut out = String::new();
        let mut rest = self.text.as_str();
        while let Some(start) = rest.find('{') {
            out.push_str(&rest[..start]);
            let end = start
                + rest[start..]
                    .find('}')
                    .ok_or_else(|| "unclosed placeholder in the mod text".to_owned())?;
            let name = &rest[start + 1..end];
            let stat_id = self
                .placeholders
                .get(name)
                .ok_or_else(|| format!("the mod text uses the unmapped placeholder {name:?}"))?;
            let stat = self
                .stats
                .iter()
                .find(|stat| stat.id == *stat_id)
                .ok_or_else(|| {
                    format!("placeholder {name:?} names the missing stat {stat_id:?}")
                })?;
            let conversion = self.conversions.get(stat_id);
            if stat.min < stat.max {
                if conversion.is_some() || stat.min < 0 {
                    return Err(format!(
                        "the range of stat {stat_id:?} is not defined by poe-crafting-rules: \
                         ranges of converted or negative values"
                    ));
                }
                out.push_str(&format!("({}-{})", stat.min, stat.max));
            } else {
                match conversion {
                    Some(conversion) => out.push_str(&conversion.show(stat.min)),
                    None => out.push_str(&stat.min.to_string()),
                }
            }
            rest = &rest[end + 1..];
        }
        out.push_str(rest);
        Ok(out)
    }
}

/// Whether a mod is a prefix or a suffix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AffixType {
    Prefix,
    Suffix,
}

/// A prefix or suffix in the game data: the mod together with what decides whether and how
/// likely it is rolled on an item.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ExplicitModDefinition {
    /// Name shown in the item name: before the base name for a prefix, after it for a suffix.
    pub name: String,
    pub affix_type: AffixType,
    /// Spawn weight, greater than zero.
    pub weight: u32,
    #[serde(flatten)]
    pub definition: ModDefinition,
}

/// A base item in the game data.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct BaseItem {
    pub id: String,
    pub name: String,
    pub item_class: String,
    pub required_level: u32,
    pub tags: Vec<String>,
    /// The mods the base item always has.
    pub implicits: Vec<ModDefinition>,
}

/// A stat of a created mod, with its rolled value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stat {
    pub id: String,
    pub min: i32,
    pub max: i32,
    /// Rolled value, within `min..=max`.
    pub value: i32,
}

/// A mod of an item, with rolled stat values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mod {
    pub family: String,
    pub level: u32,
    pub tags: Vec<String>,
    pub stats: Vec<Stat>,
    pub text: String,
    pub placeholders: BTreeMap<String, String>,
    pub conversions: BTreeMap<String, Conversion>,
}

impl Mod {
    /// Creates the mod and rolls a value for each stat within its range.
    pub fn roll(definition: &ModDefinition, rng: &mut impl Rng) -> Mod {
        let stats = definition
            .stats
            .iter()
            .map(|stat| Stat {
                id: stat.id.clone(),
                min: stat.min,
                max: stat.max,
                value: roll_in_range(rng, stat.min, stat.max),
            })
            .collect();
        Mod {
            family: definition.family.clone(),
            level: definition.level,
            tags: definition.tags.clone(),
            stats,
            text: definition.text.clone(),
            placeholders: definition.placeholders.clone(),
            conversions: definition.conversions.clone(),
        }
    }

    /// The display text with every placeholder replaced by the value of the stat it names,
    /// converted as the game does where the mod has a conversion for the stat. Stats are found
    /// by identifier, never by their position.
    ///
    /// # Panics
    /// If a placeholder names a stat the mod does not have. That is an error in the data.
    pub fn display_text(&self) -> String {
        self.placeholders
            .iter()
            .fold(self.text.clone(), |text, (name, stat_id)| {
                let stat = self
                    .stats
                    .iter()
                    .find(|stat| stat.id == *stat_id)
                    .unwrap_or_else(|| {
                        panic!("placeholder {name:?} names the missing stat {stat_id:?}")
                    });
                let shown = match self.conversions.get(stat_id) {
                    Some(conversion) => conversion.show(stat.value),
                    None => stat.value.to_string(),
                };
                text.replace(&format!("{{{name}}}"), &shown)
            })
    }
}

/// A prefix or suffix of an item: its name from the game data and the rolled mod.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplicitMod {
    pub name: String,
    pub affix_type: AffixType,
    pub modifier: Mod,
}

/// The item level a new item has: the default of the rule "Item level" in
/// `poe-crafting-rules`. The page uses it as the default of its item level field.
pub const DEFAULT_ITEM_LEVEL: u32 = 100;

/// How many mods an item can hold (rule "Affix limits" in `poe-crafting-rules`). Only
/// prefixes and suffixes count; implicit mods do not. Both the total limit and the limit of
/// the side hold at all times.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AffixLimits {
    /// At most this many prefixes and suffixes together.
    pub mods: usize,
    /// At most this many prefixes.
    pub prefixes: usize,
    /// At most this many suffixes.
    pub suffixes: usize,
}

impl AffixLimits {
    /// The limits of an item of the given rarity: none for a normal item, 2 mods (1 prefix
    /// and 1 suffix) for a magic item and 6 mods (3 and 3) for a rare item. `None` for a unique
    /// item, which has no rule yet. This is the only place of the limits.
    pub fn of(rarity: Rarity) -> Option<AffixLimits> {
        let (mods, prefixes, suffixes) = match rarity {
            Rarity::Normal => (0, 0, 0),
            Rarity::Magic => (2, 1, 1),
            Rarity::Rare => (6, 3, 3),
            Rarity::Unique => return None,
        };
        Some(AffixLimits {
            mods,
            prefixes,
            suffixes,
        })
    }
}

/// An item: a base item with a rarity, an item level, rolled implicit mods and the prefixes
/// and suffixes it holds, as many as its affix limits allow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub base: BaseItem,
    pub rarity: Rarity,
    /// Decides which mods can roll on the item (rule "Item level").
    pub item_level: u32,
    pub implicits: Vec<Mod>,
    /// In the order they were added.
    pub prefixes: Vec<ExplicitMod>,
    /// In the order they were added.
    pub suffixes: Vec<ExplicitMod>,
}

impl Item {
    /// A new normal item of `base` at `DEFAULT_ITEM_LEVEL`, with its implicit mods rolled.
    pub fn new(base: &BaseItem, rng: &mut impl Rng) -> Item {
        Item {
            base: base.clone(),
            rarity: Rarity::Normal,
            item_level: DEFAULT_ITEM_LEVEL,
            implicits: base
                .implicits
                .iter()
                .map(|definition| Mod::roll(definition, rng))
                .collect(),
            prefixes: Vec::new(),
            suffixes: Vec::new(),
        }
    }

    /// The level requirement the item shows: the higher of the base item's own requirement
    /// and 80 % of the highest level of all its mods (implicit and explicit), rounded down.
    /// None if it is 1 or lower.
    pub fn required_level(&self) -> Option<u32> {
        let highest_mod_level = self
            .implicits
            .iter()
            .chain(
                self.prefixes
                    .iter()
                    .chain(&self.suffixes)
                    .map(|explicit| &explicit.modifier),
            )
            .map(|modifier| modifier.level)
            .max()
            .unwrap_or(0);
        let requirement = self.base.required_level.max(highest_mod_level * 4 / 5);
        (requirement > 1).then_some(requirement)
    }

    /// The name of a magic item: `<prefix name> <base name> <suffix name>`. A missing prefix
    /// or suffix leaves its part out. Names of rare items are not defined yet; for them, the
    /// name uses the first prefix and the first suffix.
    pub fn name(&self) -> String {
        [
            self.prefixes.first().map(|prefix| prefix.name.as_str()),
            Some(self.base.name.as_str()),
            self.suffixes.first().map(|suffix| suffix.name.as_str()),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::SeededRng;

    const LOW: &str = "invented low stat";
    const HIGH: &str = "invented high stat";

    fn stat(id: &str, value: i32) -> StatDefinition {
        StatDefinition {
            id: id.to_owned(),
            min: value,
            max: value,
        }
    }

    fn definition(stats: Vec<StatDefinition>) -> ModDefinition {
        ModDefinition {
            family: "Invented".to_owned(),
            level: 1,
            tags: Vec::new(),
            stats,
            text: "Adds {low} to {high} Physical Damage to Attacks".to_owned(),
            placeholders: BTreeMap::from([
                ("low".to_owned(), LOW.to_owned()),
                ("high".to_owned(), HIGH.to_owned()),
            ]),
            conversions: BTreeMap::new(),
        }
    }

    fn text_of(definition: &ModDefinition) -> String {
        Mod::roll(definition, &mut SeededRng::new(1)).display_text()
    }

    #[test]
    fn text_uses_the_values_of_the_named_stats() {
        let in_order = definition(vec![stat(LOW, 3), stat(HIGH, 12)]);
        assert_eq!(
            text_of(&in_order),
            "Adds 3 to 12 Physical Damage to Attacks"
        );
    }

    #[test]
    fn text_does_not_depend_on_the_order_of_the_stats() {
        let swapped = definition(vec![stat(HIGH, 12), stat(LOW, 3)]);
        assert_eq!(text_of(&swapped), "Adds 3 to 12 Physical Damage to Attacks");
    }

    #[test]
    fn text_follows_the_rolled_values() {
        let mut varying = definition(vec![stat(HIGH, 12), stat(LOW, 3)]);
        varying.stats[0].max = 20;
        varying.stats[1].max = 8;
        let mut rng = SeededRng::new(5);
        for _ in 0..100 {
            let rolled = Mod::roll(&varying, &mut rng);
            let value = |id: &str| rolled.stats.iter().find(|s| s.id == id).unwrap().value;
            let expected = format!(
                "Adds {} to {} Physical Damage to Attacks",
                value(LOW),
                value(HIGH)
            );
            assert_eq!(rolled.display_text(), expected);
        }
    }

    #[test]
    #[should_panic(expected = "missing stat")]
    fn placeholder_without_a_stat_is_a_programming_error() {
        text_of(&definition(vec![stat(LOW, 3)]));
    }

    #[test]
    fn new_item_is_normal_with_rolled_implicits() {
        let base = BaseItem {
            id: "invented/base".to_owned(),
            name: "Invented Base".to_owned(),
            item_class: "Inventeds".to_owned(),
            required_level: 1,
            tags: Vec::new(),
            implicits: vec![definition(vec![stat(LOW, 3), stat(HIGH, 12)])],
        };
        let item = Item::new(&base, &mut SeededRng::new(1));
        assert_eq!(item.rarity, Rarity::Normal);
        assert_eq!(item.base, base);
        assert_eq!(item.implicits.len(), 1);
    }

    fn range_text_of(stats: Vec<StatDefinition>) -> String {
        definition(stats).range_text()
    }

    #[test]
    fn range_text_shows_a_range_in_brackets_and_a_fixed_value_alone() {
        let ranged = vec![
            StatDefinition {
                id: LOW.to_owned(),
                min: 3,
                max: 4,
            },
            stat(HIGH, 4),
        ];
        assert_eq!(
            range_text_of(ranged),
            "Adds (3-4) to 4 Physical Damage to Attacks"
        );
    }

    #[test]
    fn range_text_finds_stats_by_identifier_and_keeps_line_breaks() {
        let mut multi = definition(vec![
            stat(HIGH, 12),
            StatDefinition {
                id: LOW.to_owned(),
                min: 1,
                max: 2,
            },
        ]);
        multi.text.push_str(
            "
Second line",
        );
        assert_eq!(
            multi.range_text(),
            "Adds (1-2) to 12 Physical Damage to Attacks
Second line"
        );
    }

    #[test]
    fn range_text_converts_a_fixed_value_like_a_rolled_one() {
        let mut converted = definition(vec![stat(LOW, 135), stat(HIGH, -15)]);
        converted.conversions.insert(
            LOW.to_owned(),
            Conversion {
                divisor: 60,
                precision: Some(1),
            },
        );
        converted.conversions.insert(
            HIGH.to_owned(),
            Conversion {
                divisor: -1,
                precision: None,
            },
        );
        assert_eq!(
            converted.range_text(),
            "Adds 2.2 to 15 Physical Damage to Attacks"
        );
    }

    #[test]
    fn range_text_leaves_a_text_without_placeholders_unchanged() {
        let mut plain = definition(vec![stat(LOW, 1)]);
        plain.text = "No placeholders".to_owned();
        plain.placeholders.clear();
        assert_eq!(plain.range_text(), "No placeholders");
    }

    #[test]
    #[should_panic(expected = "not defined by poe-crafting-rules")]
    fn range_text_does_not_guess_the_range_of_a_converted_stat() {
        let mut converted = definition(vec![
            stat(LOW, 1),
            StatDefinition {
                id: HIGH.to_owned(),
                min: 60,
                max: 120,
            },
        ]);
        converted.conversions.insert(
            HIGH.to_owned(),
            Conversion {
                divisor: 60,
                precision: Some(1),
            },
        );
        converted.range_text();
    }

    #[test]
    #[should_panic(expected = "not defined by poe-crafting-rules")]
    fn range_text_does_not_guess_the_range_of_negative_values() {
        range_text_of(vec![
            stat(LOW, 1),
            StatDefinition {
                id: HIGH.to_owned(),
                min: -5,
                max: -3,
            },
        ]);
    }

    #[test]
    #[should_panic(expected = "missing stat")]
    fn range_text_of_a_placeholder_without_a_stat_is_a_programming_error() {
        range_text_of(vec![stat(LOW, 3)]);
    }

    fn explicit(name: &str, affix_type: AffixType) -> ExplicitMod {
        ExplicitMod {
            name: name.to_owned(),
            affix_type,
            modifier: Mod::roll(
                &definition(vec![stat(LOW, 3), stat(HIGH, 12)]),
                &mut SeededRng::new(1),
            ),
        }
    }

    fn item_with(prefix: Option<&str>, suffix: Option<&str>) -> Item {
        let base = BaseItem {
            id: "invented/base".to_owned(),
            name: "Invented Base".to_owned(),
            item_class: "Inventeds".to_owned(),
            required_level: 1,
            tags: Vec::new(),
            implicits: Vec::new(),
        };
        let mut item = Item::new(&base, &mut SeededRng::new(1));
        item.prefixes = prefix
            .map(|name| explicit(name, AffixType::Prefix))
            .into_iter()
            .collect();
        item.suffixes = suffix
            .map(|name| explicit(name, AffixType::Suffix))
            .into_iter()
            .collect();
        item
    }

    #[test]
    fn new_item_is_normal_at_the_default_item_level_without_prefixes_and_suffixes() {
        let item = item_with(None, None);
        assert_eq!(item.rarity, Rarity::Normal);
        assert_eq!(item.item_level, DEFAULT_ITEM_LEVEL);
        assert!(item.prefixes.is_empty() && item.suffixes.is_empty());
    }

    #[test]
    fn affix_limits_follow_the_rarity() {
        let limits = |rarity| {
            AffixLimits::of(rarity).map(|limits| (limits.mods, limits.prefixes, limits.suffixes))
        };
        assert_eq!(limits(Rarity::Normal), Some((0, 0, 0)));
        assert_eq!(limits(Rarity::Magic), Some((2, 1, 1)));
        assert_eq!(limits(Rarity::Rare), Some((6, 3, 3)));
        assert_eq!(limits(Rarity::Unique), None);
    }

    #[test]
    fn name_puts_the_base_name_between_prefix_and_suffix() {
        assert_eq!(
            item_with(Some("Prefix"), Some("of Suffix")).name(),
            "Prefix Invented Base of Suffix"
        );
    }

    #[test]
    fn name_leaves_out_a_missing_prefix_or_suffix() {
        assert_eq!(item_with(None, None).name(), "Invented Base");
        assert_eq!(
            item_with(Some("Prefix"), None).name(),
            "Prefix Invented Base"
        );
        assert_eq!(
            item_with(None, Some("of Suffix")).name(),
            "Invented Base of Suffix"
        );
    }

    #[test]
    fn text_converts_only_the_stats_with_a_conversion() {
        let mut converted = definition(vec![stat(LOW, 135), stat(HIGH, 135)]);
        converted.conversions.insert(
            HIGH.to_owned(),
            Conversion {
                divisor: 60,
                precision: Some(1),
            },
        );
        assert_eq!(
            text_of(&converted),
            "Adds 135 to 2.2 Physical Damage to Attacks"
        );
    }

    #[test]
    fn a_rolled_mod_keeps_the_level_and_the_conversions_of_its_definition() {
        let mut converted = definition(vec![stat(LOW, 1), stat(HIGH, 2)]);
        converted.level = 44;
        converted.conversions.insert(
            LOW.to_owned(),
            Conversion {
                divisor: 100,
                precision: None,
            },
        );
        let rolled = Mod::roll(&converted, &mut SeededRng::new(1));
        assert_eq!(rolled.level, 44);
        assert_eq!(rolled.conversions, converted.conversions);
    }

    /// A magic item of a base with the given own requirement; the implicit and the prefix
    /// have the given levels.
    fn leveled_item(base_requirement: u32, implicit: Option<u32>, prefix: Option<u32>) -> Item {
        let at_level = |level: u32| {
            let mut level_definition = definition(vec![stat(LOW, 3), stat(HIGH, 12)]);
            level_definition.level = level;
            level_definition
        };
        let base = BaseItem {
            id: "invented/base".to_owned(),
            name: "Invented Base".to_owned(),
            item_class: "Inventeds".to_owned(),
            required_level: base_requirement,
            tags: Vec::new(),
            implicits: implicit.map(at_level).into_iter().collect(),
        };
        let mut rng = SeededRng::new(1);
        let mut item = Item::new(&base, &mut rng);
        item.prefixes = prefix
            .map(|level| ExplicitMod {
                name: "Prefix".to_owned(),
                affix_type: AffixType::Prefix,
                modifier: Mod::roll(&at_level(level), &mut rng),
            })
            .into_iter()
            .collect();
        item
    }

    #[test]
    fn requirement_counts_every_prefix_and_suffix() {
        let mut item = leveled_item(0, None, Some(10));
        for (level, affix_type) in [(20, AffixType::Prefix), (60, AffixType::Suffix)] {
            let mut rolled = explicit("Another", affix_type);
            rolled.modifier.level = level;
            match affix_type {
                AffixType::Prefix => item.prefixes.push(rolled),
                AffixType::Suffix => item.suffixes.push(rolled),
            }
        }
        assert_eq!(item.required_level(), Some(48));
    }

    #[test]
    fn requirement_is_the_higher_of_the_base_and_80_percent_of_the_highest_mod_level() {
        assert_eq!(
            leveled_item(50, Some(1), Some(74)).required_level(),
            Some(59)
        );
        assert_eq!(
            leveled_item(50, Some(1), Some(35)).required_level(),
            Some(50)
        );
        assert_eq!(leveled_item(0, Some(32), None).required_level(), Some(25));
        assert_eq!(
            leveled_item(0, Some(2), Some(44)).required_level(),
            Some(35)
        );
    }

    #[test]
    fn requirement_rounds_down_and_is_hidden_at_1_or_lower() {
        assert_eq!(leveled_item(0, None, Some(51)).required_level(), Some(40));
        assert_eq!(leveled_item(0, Some(3), None).required_level(), Some(2));
        assert_eq!(leveled_item(0, Some(2), None).required_level(), None);
        assert_eq!(leveled_item(1, None, None).required_level(), None);
        assert_eq!(leveled_item(0, None, None).required_level(), None);
    }
}
