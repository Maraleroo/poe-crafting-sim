//! The parts of the RePoE raw files that the converter reads.
//!
//! The raw files are large (`mods.json` has 34 MB), so the maps keep every entry as unparsed
//! JSON and only the entries that are needed are parsed.

use std::collections::BTreeMap;

use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::value::RawValue;

/// A JSON object whose values are not parsed yet.
pub type RawMap<'a> = BTreeMap<String, &'a RawValue>;

/// Parses the text of a raw file as a JSON object whose values stay unparsed.
pub fn parse_map<'a>(file: &str, text: &'a str) -> Result<RawMap<'a>, String> {
    serde_json::from_str(text).map_err(|error| format!("{file} is not valid: {error}"))
}

/// Parses one unparsed value.
pub fn parse_value<T: DeserializeOwned>(what: &str, value: &RawValue) -> Result<T, String> {
    serde_json::from_str(value.get()).map_err(|error| format!("{what} is not valid: {error}"))
}

/// An entry of `base_items.json`.
#[derive(Deserialize)]
pub struct RawBase {
    pub name: String,
    pub item_class: String,
    pub tags: Vec<String>,
    pub implicits: Vec<String>,
    pub requirements: Option<RawRequirements>,
}

#[derive(Deserialize)]
pub struct RawRequirements {
    pub level: Option<u32>,
}

/// An entry of `item_classes.json`.
#[derive(Deserialize)]
pub struct RawItemClass {
    pub name: String,
}

/// An entry of `mods.json`.
#[derive(Deserialize)]
pub struct RawMod {
    pub generation_type: String,
    pub groups: Vec<String>,
    pub implicit_tags: Vec<String>,
    pub name: String,
    pub required_level: u32,
    pub stats: Vec<RawStat>,
}

#[derive(Deserialize)]
pub struct RawStat {
    pub id: String,
    pub min: i32,
    pub max: i32,
}

/// An entry of one item class in `mods_by_base.json`: the pool of a set of bases. Other
/// entries (for example `essences`) have neither field.
#[derive(Deserialize)]
pub struct RawPool<'a> {
    pub bases: Option<Vec<String>>,
    #[serde(borrow)]
    pub mods: Option<RawMap<'a>>,
}

/// An entry of `stat_translations.json`; only its stat identifiers.
#[derive(Deserialize)]
pub struct RawTranslationIds {
    pub ids: Vec<String>,
}

/// An entry of `stat_translations.json`.
#[derive(Deserialize)]
pub struct RawTranslationEntry {
    pub ids: Vec<String>,
    #[serde(rename = "English")]
    pub english: Option<Vec<RawTranslation>>,
}

/// One English translation of an entry.
#[derive(Deserialize)]
pub struct RawTranslation {
    pub condition: Vec<RawCondition>,
    pub format: Vec<String>,
    pub index_handlers: Vec<Vec<String>>,
    pub string: String,
}

/// The condition for one stat of an entry; a missing bound is open.
#[derive(Deserialize)]
pub struct RawCondition {
    pub min: Option<i64>,
    pub max: Option<i64>,
    pub negated: Option<bool>,
}

/// An entry of `stat_value_handlers.json`.
#[derive(Deserialize)]
pub struct RawHandler {
    #[serde(rename = "type")]
    pub kind: String,
    pub addend: Option<f64>,
    pub divisor: Option<f64>,
    pub multiplier: Option<f64>,
    pub precision: Option<u32>,
    pub fixed: Option<bool>,
}
