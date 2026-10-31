//! The order of the stats in the game's normal view (ADR 0023): the position of the entry of
//! the stat descriptions (RePoE `stat_translations.json`) that shows each stat, written into
//! `mods.json` by the converter as `stat_order`. Like the base effects (ADR 0022), it is read
//! once with a private type; no public struct has a field for it.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::Deserialize;

use crate::data::MODS_JSON;
use crate::model::placeholder_names;

/// The part of an entry of `mods.json` that shows its lines.
#[derive(Deserialize)]
struct LineEntry {
    text: String,
    placeholders: BTreeMap<String, String>,
}

#[derive(Deserialize)]
struct OrderFile {
    mods: Vec<LineEntry>,
    stat_order: BTreeMap<String, usize>,
}

/// The position of the stat's entry in the game's stat descriptions. Stats of one entry share
/// the position; a smaller position comes first in the normal view.
///
/// # Panics
/// If the stat has no position. That only happens for a mod that is not from the game data, or
/// if the embedded data is invalid (a programming error, caught by the unit tests).
pub(crate) fn position_of(stat_id: &str) -> usize {
    static ORDER: OnceLock<BTreeMap<String, usize>> = OnceLock::new();
    *ORDER
        .get_or_init(|| parse_stat_order(MODS_JSON))
        .get(stat_id)
        .unwrap_or_else(|| panic!("the stat {stat_id:?} has no position in the stat order"))
}

/// Reads `stat_order` of `mods.json` and checks it against the mods: every stat that a
/// placeholder of a mod names has a position, and the stats of one line have the same one.
///
/// # Panics
/// If the data cannot be read or fails one of these checks.
pub(crate) fn parse_stat_order(json: &str) -> BTreeMap<String, usize> {
    let file: OrderFile = serde_json::from_str(json)
        .unwrap_or_else(|error| panic!("invalid stat order data: {error}"));
    for entry in &file.mods {
        for line in entry.text.split('\n') {
            let mut positions = Vec::new();
            for name in placeholder_names(line) {
                let stat_id = entry.placeholders.get(name).unwrap_or_else(|| {
                    panic!("the mod text uses the unmapped placeholder {name:?}")
                });
                let position = file
                    .stat_order
                    .get(stat_id)
                    .unwrap_or_else(|| panic!("the stat {stat_id:?} has no position"));
                positions.push(*position);
            }
            assert!(
                positions.windows(2).all(|pair| pair[0] == pair[1]),
                "the line {line:?} shows stats with different positions"
            );
        }
    }
    file.stat_order
}

#[cfg(test)]
mod tests {
    use super::*;

    const MODS: &str = r#"{"mods": [
        {"text": "+{a} to A\n{b} to {c}", "placeholders": {"a": "sa", "b": "sb", "c": "sc"}}
    ], "stat_order": {"sa": 7, "sb": 3, "sc": 3}}"#;

    #[test]
    fn positions_are_read_by_stat() {
        let order = parse_stat_order(MODS);
        assert_eq!(order["sa"], 7);
        assert_eq!(order["sb"], order["sc"]);
    }

    #[test]
    #[should_panic(expected = "has no position")]
    fn a_stat_without_a_position_is_rejected() {
        parse_stat_order(&MODS.replace(r#", "sc": 3"#, ""));
    }

    #[test]
    #[should_panic(expected = "different positions")]
    fn a_line_with_stats_of_different_positions_is_rejected() {
        parse_stat_order(&MODS.replace(r#""sc": 3"#, r#""sc": 4"#));
    }

    #[test]
    #[should_panic(expected = "invalid stat order data")]
    fn a_file_without_stat_order_is_rejected() {
        parse_stat_order(r#"{"mods": []}"#);
    }

    #[test]
    fn the_embedded_stat_order_is_valid() {
        assert!(!parse_stat_order(MODS_JSON).is_empty());
    }

    /// How the values of a stat with a conversion are added up is not in `poe-crafting-rules`
    /// (ADR 0023). A stat that mods of several families show must therefore have no conversion
    /// and the same line in each of them; a new export with another case fails here, and the
    /// question goes to the PO.
    #[test]
    fn stats_shown_by_several_families_have_no_conversion_and_the_same_line() {
        // For each stat: the family, the line and whether the mod converts the stat
        let mut shown: BTreeMap<String, Vec<(String, String, bool)>> = BTreeMap::new();
        for explicit in crate::data::explicit_mods() {
            let definition = &explicit.definition;
            for line in definition.text.split('\n') {
                for name in placeholder_names(line) {
                    let stat_id = &definition.placeholders[name];
                    shown.entry(stat_id.clone()).or_default().push((
                        definition.family.clone(),
                        line.to_owned(),
                        definition.conversions.contains_key(stat_id),
                    ));
                }
            }
        }
        let mut several = 0;
        for (stat_id, uses) in &shown {
            if uses.iter().any(|(family, _, _)| *family != uses[0].0) {
                several += 1;
                assert!(
                    uses.iter().all(|(_, line, _)| *line == uses[0].1),
                    "{stat_id:?} is shown by several families with different lines"
                );
                assert!(
                    uses.iter().all(|(_, _, converted)| !converted),
                    "{stat_id:?} is shown by several families and has a conversion"
                );
            }
        }
        assert!(several > 0, "the rarity stat is shown by two families");
    }

    #[test]
    #[should_panic(expected = "has no position in the stat order")]
    fn the_position_of_an_unknown_stat_is_a_programming_error() {
        position_of("invented stat");
    }
}
