//! The data files that the converter writes (format of `data/README.md`).

use std::collections::BTreeMap;

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct OutStat {
    pub id: String,
    pub min: i32,
    pub max: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OutConversion {
    pub divisor: i32,
    pub precision: Option<u32>,
}

/// A mod definition, as for an implicit.
#[derive(Serialize)]
pub struct OutModDefinition {
    pub family: String,
    pub level: u32,
    pub tags: Vec<String>,
    pub stats: Vec<OutStat>,
    pub text: String,
    pub placeholders: BTreeMap<String, String>,
    pub conversions: BTreeMap<String, OutConversion>,
}

/// A prefix or suffix of a pool.
#[derive(Serialize)]
pub struct OutMod {
    pub id: String,
    pub name: String,
    pub affix_type: String,
    pub weight: u32,
    #[serde(flatten)]
    pub definition: OutModDefinition,
}

/// A value for each side of an item.
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct OutSides<T> {
    pub prefixes: T,
    pub suffixes: T,
}

#[derive(Serialize)]
pub struct OutBase {
    pub id: String,
    pub name: String,
    pub item_class: String,
    pub required_level: u32,
    pub tags: Vec<String>,
    pub implicits: Vec<OutModDefinition>,
    /// The tag of the damage type named by the implicit line "Cannot roll Modifiers of
    /// Non-<damage type> Damage Types" (ADR 0020); left out for a base without that line.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub damage_type_lock: Option<String>,
    /// How the lines "<N> Prefix/Suffix Modifier(s) allowed" in the implicits change the affix
    /// limits (ADR 0022); left out for a base without such a line.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub affix_limit_changes: Option<OutSides<i32>>,
    /// The percent of the lines "<X>% increased Prefix/Suffix Modifier magnitudes" in the
    /// implicits (ADR 0022); left out for a base without such a line.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub magnitude_increases: Option<OutSides<u32>>,
    pub mod_pool: Vec<String>,
}

#[derive(Serialize)]
pub struct OutBaseItems {
    pub base_items: Vec<OutBase>,
}

#[derive(Serialize)]
pub struct OutMods {
    pub mods: Vec<OutMod>,
}

/// The text of a data file: indented JSON with a final line break.
pub fn to_file<T: Serialize>(file: &T) -> Result<String, String> {
    let mut text = serde_json::to_string_pretty(file).map_err(|error| error.to_string())?;
    text.push('\n');
    Ok(text)
}
