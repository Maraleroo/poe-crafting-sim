//! Converts the RePoE export for Path of Exile 1 into the game data files of the simulator
//! (ADR 0015, ADR 0016).
//!
//! The library function takes the contents of the six raw files and returns the contents of
//! `data/base_items.json` and `data/mods.json`. It stops with a message instead of guessing
//! whenever a value is missing or cannot be converted. The output is deterministic.

mod output;
mod raw;
mod translate;

use std::collections::{BTreeMap, BTreeSet};

use output::{to_file, OutBase, OutBaseItems, OutMod, OutModDefinition, OutMods, OutStat};
use raw::{parse_map, parse_value, RawBase, RawItemClass, RawMap, RawMod, RawPool};
use translate::Translator;

/// The contents of the six raw files of one RePoE export.
#[derive(Debug, Clone)]
pub struct RawFiles {
    pub base_items: String,
    pub mods: String,
    pub mods_by_base: String,
    pub stat_translations: String,
    pub stat_value_handlers: String,
    pub item_classes: String,
}

/// The contents of the data files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Converted {
    pub base_items_json: String,
    pub mods_json: String,
}

/// The categories of `mods_by_base.json` that make up the pool of a base (ADR 0016).
const POOL_CATEGORIES: [&str; 2] = ["prefix", "suffix"];

/// Converts the bases with the given identifiers, their implicits and their pools.
///
/// # Errors
/// A message that names the value that cannot be converted.
pub fn convert(raw: &RawFiles, base_ids: &[&str]) -> Result<Converted, String> {
    let converter = Converter {
        base_items: parse_map("base_items.json", &raw.base_items)?,
        mods: parse_map("mods.json", &raw.mods)?,
        mods_by_base: parse_map("mods_by_base.json", &raw.mods_by_base)?,
        item_classes: parse_map("item_classes.json", &raw.item_classes)?,
        translator: Translator::new(&raw.stat_translations, &raw.stat_value_handlers)?,
    };
    let base_ids: BTreeSet<&str> = base_ids.iter().copied().collect();
    let mut bases = Vec::new();
    let mut pooled: BTreeMap<String, u32> = BTreeMap::new();
    for id in base_ids {
        let (base, weights) = converter.base(id)?;
        for (mod_id, weight) in weights {
            match pooled.insert(mod_id.clone(), weight) {
                Some(other) if other != weight => {
                    return Err(format!(
                        "the mod {mod_id} has the weights {other} and {weight} in different pools"
                    ))
                }
                _ => {}
            }
        }
        bases.push(base);
    }
    let mut mods = pooled
        .iter()
        .map(|(id, weight)| converter.explicit(id, *weight))
        .collect::<Result<Vec<_>, _>>()?;
    check_unique_names(&mods)?;
    mods.sort_by(|a, b| sort_key(a).cmp(&sort_key(b)));
    let order: BTreeMap<&str, usize> = mods
        .iter()
        .enumerate()
        .map(|(position, entry)| (entry.id.as_str(), position))
        .collect();
    for base in &mut bases {
        base.mod_pool.sort_by_key(|id| order[id.as_str()]);
    }
    Ok(Converted {
        base_items_json: to_file(&OutBaseItems { base_items: bases })?,
        mods_json: to_file(&OutMods { mods })?,
    })
}

/// Prefixes before suffixes, then by family, level and identifier.
fn sort_key(entry: &OutMod) -> (bool, &str, u32, &str) {
    (
        entry.affix_type != "prefix",
        &entry.definition.family,
        entry.definition.level,
        &entry.id,
    )
}

fn check_unique_names(mods: &[OutMod]) -> Result<(), String> {
    let mut seen: BTreeMap<&str, &str> = BTreeMap::new();
    for entry in mods {
        if let Some(other) = seen.insert(&entry.name, &entry.id) {
            return Err(format!(
                "the mods {other} and {} have the same name {:?}",
                entry.id, entry.name
            ));
        }
    }
    Ok(())
}

/// The parsed raw files that the conversion reads.
struct Converter<'a> {
    base_items: RawMap<'a>,
    mods: RawMap<'a>,
    mods_by_base: RawMap<'a>,
    item_classes: RawMap<'a>,
    translator: Translator<'a>,
}

impl Converter<'_> {
    /// The base item with its pool, and the weight of each mod of the pool.
    fn base(&self, id: &str) -> Result<(OutBase, BTreeMap<String, u32>), String> {
        let entry = self
            .base_items
            .get(id)
            .ok_or_else(|| format!("the base item {id} is not in base_items.json"))?;
        let base: RawBase = parse_value(&format!("the base item {id}"), entry)?;
        let class_entry = self.item_classes.get(&base.item_class).ok_or_else(|| {
            format!(
                "the item class {:?} of the base item {id} is not in item_classes.json",
                base.item_class
            )
        })?;
        let class: RawItemClass = parse_value(
            &format!("the item class {:?}", base.item_class),
            class_entry,
        )?;
        let weights = self.pool(id, &class.name)?;
        let implicits = base
            .implicits
            .iter()
            .map(|mod_id| self.definition(mod_id))
            .collect::<Result<_, _>>()?;
        let out = OutBase {
            id: id.to_owned(),
            name: base.name,
            item_class: class.name,
            required_level: base.requirements.and_then(|r| r.level).unwrap_or(0),
            tags: base.tags,
            implicits,
            mod_pool: weights.keys().cloned().collect(),
        };
        Ok((out, weights))
    }

    /// The weights of the mods in the categories `prefix` and `suffix` of the one entry of
    /// the item class whose bases include `base_id`.
    fn pool(&self, base_id: &str, class_name: &str) -> Result<BTreeMap<String, u32>, String> {
        let class_entry = self.mods_by_base.get(class_name).ok_or_else(|| {
            format!("the item class {class_name:?} of the base item {base_id} is not in mods_by_base.json")
        })?;
        let class: RawMap = serde_json::from_str(class_entry.get())
            .map_err(|error| format!("mods_by_base.json {class_name:?} is not valid: {error}"))?;
        let mut found = Vec::new();
        for (tags, value) in &class {
            let pool: RawPool = serde_json::from_str(value.get()).map_err(|error| {
                format!("mods_by_base.json {class_name:?} {tags:?} is not valid: {error}")
            })?;
            if pool
                .bases
                .as_ref()
                .is_some_and(|bases| bases.iter().any(|base| base == base_id))
            {
                found.push(pool);
            }
        }
        let [pool] = found.as_slice() else {
            return Err(format!(
                "the base item {base_id} is in {} pools of {class_name:?} in mods_by_base.json, \
                 but must be in exactly one",
                found.len()
            ));
        };
        let categories = pool.mods.as_ref().ok_or_else(|| {
            format!("the pool of the base item {base_id} in mods_by_base.json has no mods")
        })?;
        let mut weights = BTreeMap::new();
        for category in POOL_CATEGORIES {
            let Some(groups) = categories.get(category) else {
                continue;
            };
            let groups: BTreeMap<String, BTreeMap<String, u32>> = parse_value(
                &format!("the category {category:?} of the pool of {base_id}"),
                groups,
            )?;
            for (mod_id, weight) in groups.into_values().flatten() {
                if weight == 0 {
                    return Err(format!(
                        "the mod {mod_id} has the weight 0 in the pool of {base_id}"
                    ));
                }
                if weights.insert(mod_id.clone(), weight).is_some() {
                    return Err(format!(
                        "the mod {mod_id} occurs twice in the pool of {base_id}"
                    ));
                }
            }
        }
        Ok(weights)
    }

    /// A prefix or suffix of a pool.
    fn explicit(&self, id: &str, weight: u32) -> Result<OutMod, String> {
        let raw = self.raw_mod(id)?;
        if raw.generation_type != "prefix" && raw.generation_type != "suffix" {
            return Err(format!(
                "the mod {id} is in a pool but its generation type is {:?}",
                raw.generation_type
            ));
        }
        let name = raw.name.clone();
        let affix_type = raw.generation_type.clone();
        Ok(OutMod {
            id: id.to_owned(),
            name,
            affix_type,
            weight,
            definition: self.convert_mod(id, raw)?,
        })
    }

    /// A mod without name, affix type and weight, for example an implicit.
    fn definition(&self, id: &str) -> Result<OutModDefinition, String> {
        self.convert_mod(id, self.raw_mod(id)?)
    }

    fn raw_mod(&self, id: &str) -> Result<RawMod, String> {
        let entry = self
            .mods
            .get(id)
            .ok_or_else(|| format!("the mod {id} is not in mods.json"))?;
        parse_value(&format!("the mod {id}"), entry)
    }

    fn convert_mod(&self, id: &str, raw: RawMod) -> Result<OutModDefinition, String> {
        let [family] = raw.groups.as_slice() else {
            return Err(format!(
                "the mod {id} has {} groups, but must have exactly one",
                raw.groups.len()
            ));
        };
        let stats: Vec<OutStat> = raw
            .stats
            .into_iter()
            .map(|stat| OutStat {
                id: stat.id,
                min: stat.min,
                max: stat.max,
            })
            .collect();
        if let Some(stat) = stats.iter().find(|stat| stat.min > stat.max) {
            return Err(format!(
                "the mod {id}: the stat {:?} has min above max",
                stat.id
            ));
        }
        let display = self.translator.display(&format!("the mod {id}"), &stats)?;
        Ok(OutModDefinition {
            family: family.clone(),
            level: raw.required_level,
            tags: raw.implicit_tags,
            stats,
            text: display.text,
            placeholders: display.placeholders,
            conversions: display.conversions,
        })
    }
}
