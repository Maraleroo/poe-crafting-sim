//! Items, mods and stats, as defined by the game data and as rolled for an item.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::base_effects::effects_of;
use crate::conversion::Conversion;
use crate::data::mod_pool;
use crate::rng::{roll_in_range, Rng};
use crate::stat_order::position_of;

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

/// The tags that the advanced view never shows (rule "Advanced mod descriptions" in
/// `poe-crafting-rules`, ADR 0024). This is the only place of the list.
const HIDDEN_TAGS: [&str; 5] = [
    "elemental_damage",
    "physical_damage",
    "chaos_damage",
    "flat_life_regen",
    "resource",
];

/// `word` with an uppercase first letter.
fn capitalised(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
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
    /// by identifier, never by their position. The text shows the rolled values unscaled; the
    /// normal view of a prefix or suffix is `Item::explicit_text`.
    ///
    /// # Panics
    /// If a placeholder names a stat the mod does not have. That is an error in the data.
    pub fn display_text(&self) -> String {
        self.text_scaled_by(0)
    }

    /// Like `display_text`, but each stat value is first increased by `percent` percent:
    /// multiplied by (100 + percent), divided by 100 and rounded down (integer arithmetic).
    /// With 0 the value is unchanged.
    fn text_scaled_by(&self, percent: u32) -> String {
        self.fill(&self.text, |stat| scaled(stat.value, percent))
    }

    /// `text` with every placeholder in it replaced by the value that `value_of` gives for the
    /// stat it names, converted where the mod has a conversion for the stat.
    fn fill(&self, text: &str, value_of: impl Fn(&Stat) -> i32) -> String {
        placeholder_names(text)
            .into_iter()
            .fold(text.to_owned(), |text, name| {
                let stat_id = self.placeholders.get(name).unwrap_or_else(|| {
                    panic!("the mod text uses the unmapped placeholder {name:?}")
                });
                let value = value_of(self.stat(name, stat_id));
                text.replace(&format!("{{{name}}}"), &self.shown(stat_id, value))
            })
    }

    /// The raw value of the stat `stat_id` as the game shows it: converted where the mod has a
    /// conversion for the stat.
    fn shown(&self, stat_id: &str, raw: i32) -> String {
        match self.conversions.get(stat_id) {
            Some(conversion) => conversion.show(raw),
            None => raw.to_string(),
        }
    }

    /// The lines of the mod as the extended item text (Ctrl+Alt+C) shows them, with the rolled
    /// values unscaled (rules "In an extended item text ..." and "Values with decimals ..." in
    /// `poe-crafting-rules`, ADR 0024). A line is a list of pieces that, put together, give the
    /// line: the text with each value converted as `display_text` does, and after the value of a
    /// stat with a range (minimum less than maximum) a `TextPart::Range` `(minimum-maximum)`
    /// with both bounds converted the same way. A fixed value has no range.
    ///
    /// # Panics
    /// If a stat with a range has a negative minimum or a negative divisor: the rules do not
    /// define the range of a negative or negated value. Or if a placeholder names a stat the mod
    /// does not have. That is an error in the data.
    pub fn extended_lines(&self) -> Vec<Vec<TextPart>> {
        self.text
            .split('\n')
            .map(|line| self.extended_line(line))
            .collect()
    }

    fn extended_line(&self, line: &str) -> Vec<TextPart> {
        let mut parts = Vec::new();
        let mut text = String::new();
        let mut rest = line;
        while let Some(start) = rest.find('{') {
            let end = start
                + rest[start..]
                    .find('}')
                    .unwrap_or_else(|| panic!("unclosed placeholder in the mod text {line:?}"));
            text.push_str(&rest[..start]);
            let name = &rest[start + 1..end];
            let stat_id = self
                .placeholders
                .get(name)
                .unwrap_or_else(|| panic!("the mod text uses the unmapped placeholder {name:?}"));
            let stat = self.stat(name, stat_id);
            text.push_str(&self.shown(stat_id, stat.value));
            if stat.min < stat.max {
                let negated = self
                    .conversions
                    .get(stat_id)
                    .is_some_and(|conversion| conversion.divisor < 0);
                assert!(
                    stat.min >= 0 && !negated,
                    "the range of stat {stat_id:?} is not defined by poe-crafting-rules: \
                     ranges of negative or negated values"
                );
                parts.push(TextPart::Text(std::mem::take(&mut text)));
                parts.push(TextPart::Range(format!(
                    "({}-{})",
                    self.shown(stat_id, stat.min),
                    self.shown(stat_id, stat.max)
                )));
            }
            rest = &rest[end + 1..];
        }
        text.push_str(rest);
        if !text.is_empty() {
            parts.push(TextPart::Text(text));
        }
        parts
    }

    /// The tags of the mod's header (rule "Advanced mod descriptions" in `poe-crafting-rules`):
    /// the tags of the game data in their order, without the hidden tags, each word (split at
    /// `_`) with an uppercase first letter, separated by ", ". Empty if no tag is shown.
    pub fn tag_text(&self) -> String {
        self.tags
            .iter()
            .filter(|tag| !HIDDEN_TAGS.contains(&tag.as_str()))
            .map(|tag| {
                tag.split('_')
                    .map(capitalised)
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// The stat with the identifier that the placeholder `name` names.
    fn stat(&self, name: &str, stat_id: &str) -> &Stat {
        self.stats
            .iter()
            .find(|stat| stat.id == stat_id)
            .unwrap_or_else(|| panic!("placeholder {name:?} names the missing stat {stat_id:?}"))
    }
}

/// The names of the placeholders in `text`, in their order.
///
/// # Panics
/// If a placeholder is not closed. That is an error in the data.
pub(crate) fn placeholder_names(text: &str) -> Vec<&str> {
    let mut names = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find('{') {
        let end = rest[start..]
            .find('}')
            .unwrap_or_else(|| panic!("unclosed placeholder in the mod text {text:?}"))
            + start;
        names.push(&rest[start + 1..end]);
        rest = &rest[end + 1..];
    }
    names
}

/// `value` increased by `percent` percent and rounded down (ADR 0022). A result outside the
/// range of `i32` is cut to it; the values of the game data are far from that.
fn scaled(value: i32, percent: u32) -> i32 {
    let scaled = (i64::from(value) * (100 + i64::from(percent))).div_euclid(100);
    scaled.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

/// A prefix or suffix of an item: its name from the game data and the rolled mod.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplicitMod {
    pub name: String,
    pub affix_type: AffixType,
    pub modifier: Mod,
}

/// What a block of the advanced view shows: an implicit, a prefix or a suffix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModKind {
    Implicit,
    Prefix,
    Suffix,
}

/// A piece of a line in the extended item text. The pieces of a line, put together, are the
/// line as the extended item text shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextPart {
    /// Text of the line and the value, e.g. `+15`.
    Text(String),
    /// The range directly after a value, with its brackets, e.g. `(12-16)`.
    Range(String),
}

/// One mod in the advanced view (ADR 0024).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ModBlock {
    pub kind: ModKind,
    /// Name of a prefix or suffix; none for an implicit.
    pub name: Option<String>,
    /// Tier of a prefix or suffix; none for an implicit.
    pub tier: Option<u32>,
    /// The shown tags, separated by ", "; empty if the mod shows none.
    pub tags: String,
    /// One entry per line of the mod, unscaled values.
    pub lines: Vec<Vec<TextPart>>,
    /// X of "— X% Increased" if the base increases the values of the mod's side; none for an
    /// implicit and for a side without an increase.
    pub magnitude_increase: Option<u32>,
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

    /// These limits with the limit of each side changed by the given number (negative values
    /// reduce it), never below 0. The total limit stays.
    fn changed_by(self, prefixes: i32, suffixes: i32) -> AffixLimits {
        let changed = |limit: usize, change: i32| {
            let limit = i64::try_from(limit).unwrap_or(i64::MAX) + i64::from(change);
            usize::try_from(limit.max(0)).unwrap_or(usize::MAX)
        };
        AffixLimits {
            mods: self.mods,
            prefixes: changed(self.prefixes, prefixes),
            suffixes: changed(self.suffixes, suffixes),
        }
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

    /// The affix limits of the item: the limits of its rarity (`AffixLimits::of`) with the limit
    /// of each side changed by the lines "+N Prefix Modifiers allowed" and "-N Suffix Modifiers
    /// allowed" of its base, never below 0 (rule "Affix limits" in `poe-crafting-rules`, ADR
    /// 0022). The total limit does not change. `None` for a unique item. This is where the
    /// changes are applied; `AffixLimits::of` stays the only place of the limits themselves.
    pub fn affix_limits(&self) -> Option<AffixLimits> {
        let changes = effects_of(&self.base.id).affix_limit_changes;
        AffixLimits::of(self.rarity)
            .map(|limits| limits.changed_by(changes.prefixes, changes.suffixes))
    }

    /// The text of a prefix or suffix of the item in the normal view: each stat value is
    /// increased by the percent of the lines "X% increased Prefix/Suffix Modifier magnitudes"
    /// of the base for the mod's side (multiplied by (100 + X), divided by 100, rounded down),
    /// then converted and inserted as `Mod::display_text` does. The mod keeps its unscaled
    /// values. Without such a line the text equals `explicit.modifier.display_text()`.
    ///
    /// # Panics
    /// If a placeholder of the mod names a stat the mod does not have. That is an error in the
    /// data.
    pub fn explicit_text(&self, explicit: &ExplicitMod) -> String {
        let percent = effects_of(&self.base.id)
            .magnitude_increases
            .of(explicit.affix_type);
        explicit.modifier.text_scaled_by(percent)
    }

    /// The explicit lines of the item in the normal view (rule "In the normal view, ..." in
    /// `poe-crafting-rules`, ADR 0023), one entry per line:
    /// 1. every line of every prefix and suffix, with its stat values increased as
    ///    `explicit_text` does;
    /// 2. lines of different mods that show the same stats become one line, with the sum of
    ///    the increased values for each stat;
    /// 3. the lines are sorted by the position of their stats in the game's stat descriptions
    ///    (the order of the data, prefixes before suffixes, decides between equal positions).
    ///
    /// Implicit mods are not part of it. `prefixes` and `suffixes` keep the order the mods
    /// were added in; only this view sorts and merges. `explicit_text` is the text of one mod
    /// alone.
    ///
    /// # Panics
    /// If a line shows no stat or a stat without a position, or a placeholder names a stat the
    /// mod does not have. That only happens for a mod that is not from the game data.
    pub fn explicit_texts(&self) -> Vec<String> {
        struct Line<'a> {
            text: &'a str,
            owner: &'a Mod,
            /// The increased value of each stat the line shows.
            values: BTreeMap<&'a str, i32>,
        }
        let mut lines: Vec<Line> = Vec::new();
        for explicit in self.prefixes.iter().chain(&self.suffixes) {
            let percent = effects_of(&self.base.id)
                .magnitude_increases
                .of(explicit.affix_type);
            let owner = &explicit.modifier;
            for text in owner.text.split('\n') {
                let values: BTreeMap<&str, i32> = placeholder_names(text)
                    .into_iter()
                    .map(|name| {
                        let stat_id = owner.placeholders.get(name).unwrap_or_else(|| {
                            panic!("the mod text uses the unmapped placeholder {name:?}")
                        });
                        let stat = owner.stat(name, stat_id);
                        (stat.id.as_str(), scaled(stat.value, percent))
                    })
                    .collect();
                assert!(
                    !values.is_empty(),
                    "the line {text:?} shows no stat, so it has no place in the order"
                );
                match lines
                    .iter_mut()
                    .find(|line| line.values.keys().eq(values.keys()))
                {
                    Some(line) => {
                        for (stat_id, value) in values {
                            *line.values.get_mut(stat_id).expect("the same stats") += value;
                        }
                    }
                    None => lines.push(Line {
                        text,
                        owner,
                        values,
                    }),
                }
            }
        }
        // The stats of a line share one position (checked when the data is read)
        lines.sort_by_cached_key(|line| {
            line.values
                .keys()
                .next()
                .map(|stat_id| position_of(stat_id))
        });
        lines
            .into_iter()
            .map(|line| {
                line.owner
                    .fill(line.text, |stat| line.values[stat.id.as_str()])
            })
            .collect()
    }

    /// The mods of the item in the advanced view (rule "Advanced mod descriptions" in
    /// `poe-crafting-rules`, ADR 0024), one block per mod, nothing merged: the implicits in the
    /// order of the base, then the prefixes, then the suffixes, each in the order they were
    /// added. The lines show the unscaled values; `magnitude_increase` holds the increase of
    /// the mod's side. The base's pool is read once per call, and only if the item has a prefix
    /// or a suffix.
    ///
    /// # Panics
    /// As `Mod::extended_lines`.
    pub fn advanced_mods(&self) -> Vec<ModBlock> {
        let pool = if self.prefixes.is_empty() && self.suffixes.is_empty() {
            Vec::new()
        } else {
            mod_pool(&self.base.id).unwrap_or_default()
        };
        let implicits = self.implicits.iter().map(|implicit| ModBlock {
            kind: ModKind::Implicit,
            name: None,
            tier: None,
            tags: implicit.tag_text(),
            lines: implicit.extended_lines(),
            magnitude_increase: None,
        });
        let explicits = self.prefixes.iter().chain(&self.suffixes).map(|explicit| {
            let increase = effects_of(&self.base.id)
                .magnitude_increases
                .of(explicit.affix_type);
            ModBlock {
                kind: match explicit.affix_type {
                    AffixType::Prefix => ModKind::Prefix,
                    AffixType::Suffix => ModKind::Suffix,
                },
                name: Some(explicit.name.clone()),
                tier: Some(tier_in(&pool, explicit)),
                tags: explicit.modifier.tag_text(),
                lines: explicit.modifier.extended_lines(),
                magnitude_increase: (increase > 0).then_some(increase),
            }
        });
        implicits.chain(explicits).collect()
    }

    /// The tier of a prefix or suffix (rule "Advanced mod descriptions" in
    /// `poe-crafting-rules`, ADR 0024): 1 plus the number of mods in the base's pool on the
    /// same side, of the same family and with a higher level. The pool is the one of the game
    /// data, filtered neither by the item level nor by the damage type lock. A base without a
    /// pool counts as an empty pool.
    pub fn tier(&self, explicit: &ExplicitMod) -> u32 {
        tier_in(&mod_pool(&self.base.id).unwrap_or_default(), explicit)
    }

    /// The name of the item: the names of all prefixes, the base name and the names of all
    /// suffixes, each list in the order its mods were added, separated by spaces (rule "Item
    /// basics" in `poe-crafting-rules`). A side without mods leaves its part out. Names of
    /// rare items are not defined by a rule yet; the formula is the same for them.
    pub fn name(&self) -> String {
        self.prefixes
            .iter()
            .map(|prefix| prefix.name.as_str())
            .chain([self.base.name.as_str()])
            .chain(self.suffixes.iter().map(|suffix| suffix.name.as_str()))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// 1 plus the number of mods in `pool` on the same side as `explicit`, of the same family and
/// with a higher level.
fn tier_in(pool: &[ExplicitModDefinition], explicit: &ExplicitMod) -> u32 {
    let higher = pool
        .iter()
        .filter(|definition| {
            definition.affix_type == explicit.affix_type
                && definition.definition.family == explicit.modifier.family
                && definition.definition.level > explicit.modifier.level
        })
        .count();
    1 + u32::try_from(higher).unwrap_or(u32::MAX - 1)
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

    #[test]
    fn name_has_all_prefixes_and_suffixes_in_the_order_they_were_added() {
        let mut item = item_with(Some("First"), Some("of One"));
        item.prefixes.push(explicit("Second", AffixType::Prefix));
        item.suffixes.push(explicit("of Two", AffixType::Suffix));
        assert_eq!(item.name(), "First Second Invented Base of One of Two");
    }

    #[test]
    fn limits_changed_by_a_number_never_fall_below_zero_and_keep_the_total() {
        let magic = AffixLimits::of(Rarity::Magic).unwrap();
        let changed = magic.changed_by(3, -3);
        assert_eq!(
            (changed.mods, changed.prefixes, changed.suffixes),
            (2, 4, 0)
        );
        let changed = magic.changed_by(-5, 1);
        assert_eq!(
            (changed.mods, changed.prefixes, changed.suffixes),
            (2, 0, 2)
        );
        let normal = AffixLimits::of(Rarity::Normal).unwrap();
        let changed = normal.changed_by(-1, 2);
        assert_eq!(
            (changed.mods, changed.prefixes, changed.suffixes),
            (0, 0, 2)
        );
        assert_eq!(magic.changed_by(0, 0), magic);
    }

    #[test]
    fn a_base_without_changes_has_the_limits_of_its_rarity() {
        for rarity in [Rarity::Normal, Rarity::Magic, Rarity::Rare, Rarity::Unique] {
            let mut item = item_with(None, None);
            item.rarity = rarity;
            assert_eq!(item.affix_limits(), AffixLimits::of(rarity));
        }
    }

    /// A base of the game data with the given identifier, as a rare item with a prefix and a
    /// suffix whose only stat has the value 11 or 100.
    fn rare_item_of(base_id: &str, value: i32) -> Item {
        let base = crate::data::base_item(base_id).expect("the base is in the game data");
        let mut item = Item::new(&base, &mut SeededRng::new(1));
        item.rarity = Rarity::Rare;
        for affix_type in [AffixType::Prefix, AffixType::Suffix] {
            let mut rolled = explicit("Invented", affix_type);
            rolled.modifier.text = "{value}".to_owned();
            rolled.modifier.placeholders = BTreeMap::from([("value".to_owned(), HIGH.to_owned())]);
            rolled
                .modifier
                .stats
                .iter_mut()
                .for_each(|stat| stat.value = value);
            match affix_type {
                AffixType::Prefix => item.prefixes.push(rolled),
                AffixType::Suffix => item.suffixes.push(rolled),
            }
        }
        item
    }

    #[test]
    fn explicit_text_increases_the_values_of_the_side_that_the_base_increases() {
        // The base increases suffixes by 25 % and has +1 suffix and -1 prefix
        let item = rare_item_of("Metadata/Items/Rings/RingE1", 11);
        assert_eq!(item.explicit_text(&item.prefixes[0]), "11");
        assert_eq!(item.explicit_text(&item.suffixes[0]), "13");
        // The rolled value stays unscaled
        assert_eq!(item.suffixes[0].modifier.display_text(), "11");
        let item = rare_item_of("Metadata/Items/Rings/RingE4", 57);
        assert_eq!(item.explicit_text(&item.prefixes[0]), "85");
        assert_eq!(item.explicit_text(&item.suffixes[0]), "57");
    }

    #[test]
    fn explicit_text_without_an_increase_equals_the_display_text() {
        let item = rare_item_of("Metadata/Items/Rings/RingE3", 11);
        for explicit in item.prefixes.iter().chain(&item.suffixes) {
            assert_eq!(
                item.explicit_text(explicit),
                explicit.modifier.display_text()
            );
        }
    }

    #[test]
    fn scaling_uses_integer_arithmetic_and_rounds_down() {
        assert_eq!(scaled(11, 25), 13);
        assert_eq!(scaled(6, 25), 7);
        assert_eq!(scaled(47, 25), 58);
        assert_eq!(scaled(57, 50), 85);
        assert_eq!(scaled(31, 50), 46);
        assert_eq!(scaled(7, 0), 7);
        assert_eq!(scaled(-7, 0), -7);
        assert_eq!(scaled(0, 100), 0);
        assert_eq!(scaled(i32::MAX, 100), i32::MAX);
    }

    #[test]
    fn scaling_happens_before_the_conversion() {
        let mut converted = definition(vec![stat(LOW, 1), stat(HIGH, 30)]);
        converted.text = "{high}".to_owned();
        converted.placeholders = BTreeMap::from([("high".to_owned(), HIGH.to_owned())]);
        converted.conversions.insert(
            HIGH.to_owned(),
            Conversion {
                divisor: 100,
                precision: None,
            },
        );
        let rolled = Mod::roll(&converted, &mut SeededRng::new(1));
        assert_eq!(rolled.display_text(), "0.3");
        assert_eq!(rolled.text_scaled_by(25), "0.37");
    }

    /// The base of the first ring of the game data and its pool.
    fn ring_and_pool() -> (BaseItem, Vec<ExplicitModDefinition>) {
        let base = crate::slot::Slot::Ring
            .base_items()
            .into_iter()
            .next()
            .expect("the game data has a ring");
        let pool = crate::data::mod_pool(&base.id).expect("the base is in the game data");
        (base, pool)
    }

    /// A rare item of `base` with the given mods of its pool, added in the given order.
    fn rare_item_with(base: &BaseItem, mods: &[&ExplicitModDefinition]) -> Item {
        let mut rng = SeededRng::new(3);
        let mut item = Item::new(base, &mut rng);
        item.rarity = Rarity::Rare;
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

    /// The mods of the pool with one stat and one placeholder, the first of each stat and side.
    fn single_stat_mods(pool: &[ExplicitModDefinition]) -> Vec<&ExplicitModDefinition> {
        let mut seen = Vec::new();
        pool.iter()
            .filter(|definition| {
                definition.definition.stats.len() == 1
                    && definition.definition.text.matches('{').count() == 1
            })
            .filter(|definition| {
                let key = (
                    definition.definition.stats[0].id.clone(),
                    definition.affix_type,
                );
                let new = !seen.contains(&key);
                seen.push(key);
                new
            })
            .collect()
    }

    #[test]
    fn an_item_without_mods_has_no_explicit_lines() {
        let (base, _) = ring_and_pool();
        assert!(rare_item_with(&base, &[]).explicit_texts().is_empty());
    }

    #[test]
    fn explicit_lines_follow_the_stat_positions_whatever_the_order_added() {
        let (base, pool) = ring_and_pool();
        let single = single_stat_mods(&pool);
        let mut checked = 0;
        for first in &single {
            for second in &single {
                let position = |definition: &ExplicitModDefinition| {
                    position_of(&definition.definition.stats[0].id)
                };
                if position(first) >= position(second) || first.affix_type == second.affix_type {
                    continue;
                }
                for order in [[*first, *second], [*second, *first]] {
                    let item = rare_item_with(&base, &order);
                    let text = |definition: &ExplicitModDefinition| {
                        let side = match definition.affix_type {
                            AffixType::Prefix => &item.prefixes,
                            AffixType::Suffix => &item.suffixes,
                        };
                        item.explicit_text(&side[0])
                    };
                    assert_eq!(item.explicit_texts(), [text(first), text(second)]);
                    checked += 1;
                }
            }
        }
        assert!(checked > 0);
    }

    #[test]
    fn lines_of_mods_that_show_the_same_stat_are_one_line_with_the_sum() {
        let (base, pool) = ring_and_pool();
        let single = single_stat_mods(&pool);
        let both = single.iter().find_map(|prefix| {
            single
                .iter()
                .find(|suffix| {
                    prefix.affix_type == AffixType::Prefix
                        && suffix.affix_type == AffixType::Suffix
                        && prefix.definition.stats[0].id == suffix.definition.stats[0].id
                })
                .map(|suffix| [*prefix, *suffix])
        });
        let both = both.expect("a stat that a prefix and a suffix of the pool show");
        let mut item = rare_item_with(&base, &both);
        item.prefixes[0].modifier.stats[0].value = 20;
        item.suffixes[0].modifier.stats[0].value = 8;
        let mut summed = item.prefixes[0].modifier.clone();
        summed.stats[0].value = 28;
        assert_eq!(item.explicit_texts(), [summed.display_text()]);
        // The mods themselves keep their values and their order
        assert_eq!(item.prefixes[0].modifier.stats[0].value, 20);
        assert_eq!(item.suffixes[0].modifier.stats[0].value, 8);
    }

    #[test]
    fn extended_lines_put_the_range_directly_after_a_value_with_a_range() {
        let ranged = definition(vec![
            StatDefinition {
                id: LOW.to_owned(),
                min: 3,
                max: 3,
            },
            StatDefinition {
                id: HIGH.to_owned(),
                min: 10,
                max: 14,
            },
        ]);
        let mut rolled = Mod::roll(&ranged, &mut SeededRng::new(1));
        rolled.stats[1].value = 12;
        assert_eq!(
            rolled.extended_lines(),
            [vec![
                TextPart::Text("Adds 3 to 12".to_owned()),
                TextPart::Range("(10-14)".to_owned()),
                TextPart::Text(" Physical Damage to Attacks".to_owned()),
            ]]
        );
    }

    #[test]
    fn extended_lines_convert_the_value_and_both_bounds() {
        let mut converted = definition(vec![
            StatDefinition {
                id: LOW.to_owned(),
                min: 120,
                max: 240,
            },
            stat(HIGH, 210),
        ]);
        converted.text = "Regenerate {low} Life per second\n{high} Life".to_owned();
        for id in [LOW, HIGH] {
            converted.conversions.insert(
                id.to_owned(),
                Conversion {
                    divisor: 60,
                    precision: Some(1),
                },
            );
        }
        let mut rolled = Mod::roll(&converted, &mut SeededRng::new(1));
        rolled.stats[0].value = 210;
        assert_eq!(
            rolled.extended_lines(),
            [
                vec![
                    TextPart::Text("Regenerate 3.5".to_owned()),
                    TextPart::Range("(2-4)".to_owned()),
                    TextPart::Text(" Life per second".to_owned()),
                ],
                vec![TextPart::Text("3.5 Life".to_owned())],
            ]
        );
    }

    #[test]
    #[should_panic(expected = "not defined by poe-crafting-rules")]
    fn extended_lines_do_not_guess_the_range_of_a_negated_value() {
        let mut negated = definition(vec![
            stat(LOW, 1),
            StatDefinition {
                id: HIGH.to_owned(),
                min: 5,
                max: 8,
            },
        ]);
        negated.conversions.insert(
            HIGH.to_owned(),
            Conversion {
                divisor: -1,
                precision: None,
            },
        );
        Mod::roll(&negated, &mut SeededRng::new(1)).extended_lines();
    }

    #[test]
    #[should_panic(expected = "not defined by poe-crafting-rules")]
    fn extended_lines_do_not_guess_the_range_of_a_negative_value() {
        let negative = definition(vec![
            stat(LOW, 1),
            StatDefinition {
                id: HIGH.to_owned(),
                min: -5,
                max: -3,
            },
        ]);
        Mod::roll(&negative, &mut SeededRng::new(1)).extended_lines();
    }

    #[test]
    fn no_mod_of_the_data_has_a_range_of_a_negative_or_negated_value() {
        let implicits = crate::slot::Slot::Ring
            .base_items()
            .into_iter()
            .flat_map(|base| base.implicits);
        let explicits = crate::data::explicit_mods()
            .into_iter()
            .map(|explicit| explicit.definition);
        for definition in implicits.chain(explicits) {
            // Panics on a range that the rules do not define
            Mod::roll(&definition, &mut SeededRng::new(1)).extended_lines();
        }
    }

    #[test]
    fn tags_are_capitalised_per_word_and_the_hidden_ones_are_left_out() {
        let mut tagged = Mod::roll(&definition(vec![stat(LOW, 1)]), &mut SeededRng::new(1));
        tagged.tags = ["resource", "test_new_tag", "life", "chaos_damage"]
            .map(str::to_owned)
            .to_vec();
        assert_eq!(tagged.tag_text(), "Test New Tag, Life");
        tagged.tags = HIDDEN_TAGS.map(str::to_owned).to_vec();
        assert_eq!(tagged.tag_text(), "");
    }

    #[test]
    fn no_family_of_a_locked_base_mixes_blocked_and_unblocked_mods() {
        // Otherwise the rule would leave open which pool counts for the tier
        for base in crate::slot::Slot::Ring.base_items() {
            let pool = crate::data::mod_pool(&base.id).expect("every base has a pool");
            let unblocked = crate::damage_lock::unblocked_mods(&base.id, &pool);
            for definition in &pool {
                let same_family = |other: &&ExplicitModDefinition| {
                    other.affix_type == definition.affix_type
                        && other.definition.family == definition.definition.family
                };
                let all = pool.iter().filter(same_family).count();
                let free = unblocked.iter().filter(same_family).count();
                assert!(
                    free == 0 || free == all,
                    "{}: the family {} mixes blocked and unblocked mods",
                    base.name,
                    definition.definition.family
                );
            }
        }
    }

    #[test]
    fn the_tier_counts_mods_of_the_same_side_and_family_with_a_higher_level() {
        let (base, pool) = ring_and_pool();
        let lowest = pool
            .iter()
            .min_by_key(|definition| definition.definition.level)
            .expect("the pool has mods");
        let item = rare_item_with(&base, &[lowest]);
        let explicit = item.prefixes.iter().chain(&item.suffixes).next().unwrap();
        let higher = pool
            .iter()
            .filter(|other| {
                other.affix_type == lowest.affix_type
                    && other.definition.family == lowest.definition.family
                    && other.definition.level > lowest.definition.level
            })
            .count();
        assert_eq!(item.tier(explicit), 1 + higher as u32);
        // The item level does not matter
        let mut low = item.clone();
        low.item_level = 1;
        assert_eq!(low.tier(explicit), item.tier(explicit));
    }

    #[test]
    fn a_base_without_a_pool_counts_as_an_empty_pool() {
        let item = item_with(Some("Prefix"), Some("of Suffix"));
        assert_eq!(item.tier(&item.prefixes[0]), 1);
        let tiers: Vec<Option<u32>> = item.advanced_mods().iter().map(|b| b.tier).collect();
        assert_eq!(tiers, [Some(1), Some(1)]);
    }

    #[test]
    fn advanced_mods_put_implicits_first_then_prefixes_then_suffixes() {
        let mut item = leveled_item(0, Some(1), Some(10));
        item.suffixes.push(explicit("of One", AffixType::Suffix));
        item.prefixes.push(explicit("Second", AffixType::Prefix));
        let kinds: Vec<(ModKind, Option<String>)> = item
            .advanced_mods()
            .into_iter()
            .map(|block| (block.kind, block.name))
            .collect();
        assert_eq!(
            kinds,
            [
                (ModKind::Implicit, None),
                (ModKind::Prefix, Some("Prefix".to_owned())),
                (ModKind::Prefix, Some("Second".to_owned())),
                (ModKind::Suffix, Some("of One".to_owned())),
            ]
        );
    }
}
