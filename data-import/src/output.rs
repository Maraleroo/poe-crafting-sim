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
    pub divisor: u32,
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

#[derive(Serialize)]
pub struct OutBase {
    pub id: String,
    pub name: String,
    pub item_class: String,
    pub required_level: u32,
    pub tags: Vec<String>,
    pub implicits: Vec<OutModDefinition>,
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
