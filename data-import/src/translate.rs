//! Display texts and value conversions of mods, from `stat_translations.json` and
//! `stat_value_handlers.json` (ADR 0015, ADR 0016).

use std::collections::btree_map::Entry;
use std::collections::BTreeMap;

use serde_json::value::RawValue;

use crate::output::{OutConversion, OutStat};
use crate::raw::{
    parse_map, parse_value, RawCondition, RawHandler, RawMap, RawTranslation, RawTranslationEntry,
    RawTranslationIds,
};

/// The text of a mod with the conversions of its stats.
pub struct Display {
    pub text: String,
    pub placeholders: BTreeMap<String, String>,
    pub conversions: BTreeMap<String, OutConversion>,
}

/// `stat_translations.json` and `stat_value_handlers.json`.
pub struct Translator<'a> {
    entries: Vec<&'a RawValue>,
    /// Positions of the entries that name a stat, by stat identifier.
    by_stat: BTreeMap<String, Vec<usize>>,
    handlers: RawMap<'a>,
}

impl<'a> Translator<'a> {
    pub fn new(translations: &'a str, handlers: &'a str) -> Result<Self, String> {
        let entries: Vec<&RawValue> = serde_json::from_str(translations)
            .map_err(|error| format!("stat_translations.json is not valid: {error}"))?;
        let mut by_stat: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (position, entry) in entries.iter().enumerate() {
            let ids: RawTranslationIds =
                parse_value(&format!("stat_translations.json entry {position}"), entry)?;
            for id in ids.ids {
                by_stat.entry(id).or_default().push(position);
            }
        }
        Ok(Self {
            entries,
            by_stat,
            handlers: parse_map("stat_value_handlers.json", handlers)?,
        })
    }

    /// The text and the conversions of a mod with the given stats. `owner` names the mod or
    /// base item for the messages.
    pub fn display(&self, owner: &str, stats: &[OutStat]) -> Result<Display, String> {
        for (position, stat) in stats.iter().enumerate() {
            if stat.id.contains(['{', '}']) {
                return Err(format!(
                    "{owner}: the stat id {:?} contains a brace",
                    stat.id
                ));
            }
            if stats[..position].iter().any(|other| other.id == stat.id) {
                return Err(format!("{owner}: the stat {:?} occurs twice", stat.id));
            }
        }
        // The entries that name a stat of the mod, parsed once, by position
        let mut entries: BTreeMap<usize, RawTranslationEntry> = BTreeMap::new();
        // The translation that applies, by position of its entry
        let mut chosen: BTreeMap<usize, usize> = BTreeMap::new();
        for stat in stats {
            let positions = self.by_stat.get(&stat.id).ok_or_else(|| {
                format!(
                    "{owner}: the stat {:?} has no entry in stat_translations.json",
                    stat.id
                )
            })?;
            let mut matches = Vec::new();
            for &position in positions {
                let entry = match entries.entry(position) {
                    Entry::Occupied(entry) => entry.into_mut(),
                    Entry::Vacant(slot) => slot.insert(parse_value(
                        &format!("stat_translations.json entry {position}"),
                        self.entries[position],
                    )?),
                };
                for (index, translation) in entry.english.iter().flatten().enumerate() {
                    if applies(owner, &entry.ids, translation, stats)? {
                        matches.push((position, index));
                    }
                }
            }
            let [(position, index)] = matches.as_slice() else {
                return Err(format!(
                    "{owner}: {} English translations of the stat {:?} hold for its range {} \
                     to {}, but exactly one must",
                    matches.len(),
                    stat.id,
                    stat.min,
                    stat.max
                ));
            };
            chosen.insert(*position, *index);
        }
        let mut lines = Vec::new();
        let mut placeholders = BTreeMap::new();
        let mut conversions = BTreeMap::new();
        for (position, index) in chosen {
            let entry = &entries[&position];
            let translation = &entry.english.as_deref().unwrap_or_default()[index];
            lines.push(self.line(
                owner,
                entry,
                translation,
                stats,
                &mut placeholders,
                &mut conversions,
            )?);
        }
        Ok(Display {
            text: lines.join("\n"),
            placeholders,
            conversions,
        })
    }

    /// The text of one translation with its placeholders named after the stat identifiers;
    /// the placeholders go into `placeholders` and the conversions of the stats into
    /// `conversions`. A stat with the format `ignore` gets neither (ADR 0017).
    fn line(
        &self,
        owner: &str,
        entry: &RawTranslationEntry,
        translation: &RawTranslation,
        stats: &[OutStat],
        placeholders: &mut BTreeMap<String, String>,
        conversions: &mut BTreeMap<String, OutConversion>,
    ) -> Result<String, String> {
        let mut shown = Vec::new();
        for (index, id) in entry.ids.iter().enumerate() {
            let (min, max) = range_of(stats, id)
                .ok_or_else(|| format!("{owner}: the stat {id:?} is missing"))?;
            shown.push(match translation.format[index].as_str() {
                "#" => format!("{{{id}}}"),
                "+#" if min >= 0 => format!("+{{{id}}}"),
                "+#" => {
                    return Err(format!(
                        "{owner}: the stat {id:?} has the format \"+#\" but can be negative \
                         (range {min} to {max})"
                    ))
                }
                "ignore" if !translation.string.contains(&format!("{{{index}}}")) => {
                    shown.push(String::new());
                    continue;
                }
                "ignore" => {
                    return Err(format!(
                        "{owner}: the stat {id:?} has the format \"ignore\" but the text \
                         {:?} shows its value",
                        translation.string
                    ))
                }
                other => {
                    return Err(format!(
                        "{owner}: the stat {id:?} has the unsupported format {other:?}"
                    ))
                }
            });
            placeholders.insert(id.clone(), id.clone());
            let handlers = &translation.index_handlers[index];
            if let Some(conversion) = self.conversion(owner, id, handlers)? {
                conversions.insert(id.clone(), conversion);
            }
        }
        substitute(owner, &translation.string, &shown)
    }

    /// The conversion that the handlers of one stat stand for, if there is one.
    fn conversion(
        &self,
        owner: &str,
        stat: &str,
        names: &[String],
    ) -> Result<Option<OutConversion>, String> {
        let name = match names {
            [] => return Ok(None),
            [name] => name,
            _ => {
                return Err(format!(
                    "{owner}: the stat {stat:?} has more than one value handler: {names:?}"
                ))
            }
        };
        let fail = |why: &str| format!("{owner}: the handler {name:?} of the stat {stat:?} {why}");
        let raw = self
            .handlers
            .get(name)
            .ok_or_else(|| fail("is not in stat_value_handlers.json"))?;
        let handler: RawHandler = parse_value(&format!("the handler {name:?}"), raw)?;
        match handler.kind.as_str() {
            "noop" => return Ok(None),
            "int" => {}
            _ => return Err(fail("has a type the converter does not know")),
        }
        if handler.addend.is_some() || handler.fixed == Some(true) {
            return Err(fail("has an addend or a fixed value"));
        }
        let divisor = match (handler.multiplier, handler.divisor) {
            // RePoE's `negate`: the shown value has the reversed sign (ADR 0017)
            (Some(-1.0), None) => -1,
            (Some(_), _) => {
                return Err(fail(
                    "has a multiplier other than -1, or a multiplier together with a divisor",
                ))
            }
            (None, None) => return Err(fail("has no divisor")),
            (None, Some(divisor)) => {
                if divisor.fract() != 0.0 || divisor < 1.0 || divisor > f64::from(i32::MAX) {
                    return Err(fail("has a divisor that is not a positive whole number"));
                }
                divisor as i32
            }
        };
        if handler.precision.is_none() && !has_finite_decimal(divisor.unsigned_abs()) {
            return Err(fail(
                "has no precision and a divisor whose quotient has no exact decimal form",
            ));
        }
        Ok(Some(OutConversion {
            divisor,
            precision: handler.precision,
        }))
    }
}

/// The range of the stat with the given identifier.
fn range_of(stats: &[OutStat], id: &str) -> Option<(i32, i32)> {
    stats
        .iter()
        .find(|stat| stat.id == id)
        .map(|stat| (stat.min, stat.max))
}

/// Whether the translation applies to the mod: it names only stats of the mod and each
/// condition holds for the whole range of its stat.
fn applies(
    owner: &str,
    ids: &[String],
    translation: &RawTranslation,
    stats: &[OutStat],
) -> Result<bool, String> {
    if translation.condition.len() != ids.len()
        || translation.format.len() != ids.len()
        || translation.index_handlers.len() != ids.len()
    {
        return Err(format!(
            "{owner}: a translation of {ids:?} has conditions, formats or handlers that do not \
             match its stats"
        ));
    }
    for (id, condition) in ids.iter().zip(&translation.condition) {
        let Some((min, max)) = range_of(stats, id) else {
            return Ok(false);
        };
        if !holds(owner, id, condition, min, max)? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Whether the condition holds for every value from `min` to `max`.
fn holds(
    owner: &str,
    stat: &str,
    condition: &RawCondition,
    min: i32,
    max: i32,
) -> Result<bool, String> {
    if condition.negated == Some(true) {
        return Err(format!(
            "{owner}: the stat {stat:?} has a negated condition, which the converter does not \
             know"
        ));
    }
    Ok(condition.min.is_none_or(|lower| i64::from(min) >= lower)
        && condition.max.is_none_or(|upper| i64::from(max) <= upper))
}

/// Replaces every `{N}` of `string` by the Nth of `shown`.
fn substitute(owner: &str, string: &str, shown: &[String]) -> Result<String, String> {
    let bad = || {
        format!("{owner}: the translation {string:?} has a placeholder the converter does not know")
    };
    let mut out = String::new();
    let mut rest = string;
    while let Some(start) = rest.find(['{', '}']) {
        out.push_str(&rest[..start]);
        if !rest[start..].starts_with('{') {
            return Err(bad());
        }
        let end = start + rest[start..].find('}').ok_or_else(bad)?;
        let index: usize = rest[start + 1..end].parse().map_err(|_| bad())?;
        out.push_str(shown.get(index).ok_or_else(bad)?);
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

/// Whether `1 / divisor` has a finite decimal form, that is, the divisor has no prime
/// factor other than 2 and 5.
fn has_finite_decimal(mut divisor: u32) -> bool {
    for factor in [2, 5] {
        while divisor.is_multiple_of(factor) {
            divisor /= factor;
        }
    }
    divisor == 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_divisors_of_two_and_five_have_a_finite_decimal() {
        for divisor in [1, 2, 4, 5, 10, 50, 100, 125] {
            assert!(has_finite_decimal(divisor), "{divisor}");
        }
        for divisor in [3, 6, 7, 60, 15] {
            assert!(!has_finite_decimal(divisor), "{divisor}");
        }
    }

    #[test]
    fn substitute_replaces_numbered_placeholders() {
        let shown = ["+{a}".to_owned(), "{b}".to_owned()];
        assert_eq!(
            substitute("m", "{0} to {1}, {0}!", &shown).as_deref(),
            Ok("+{a} to {b}, +{a}!")
        );
    }

    const TRANSLATIONS: &str = r##"[
        {"ids": ["life"], "English": [
            {"condition": [{"min": null, "max": null, "negated": null}], "format": ["+#"],
             "index_handlers": [["tq_noop"]], "string": "{0} to Life"}]},
        {"ids": ["third"], "English": [
            {"condition": [{"min": null, "max": null, "negated": null}], "format": ["#"],
             "index_handlers": [["by_three"]], "string": "{0} thirds"}]},
        {"ids": ["reduced"], "English": [
            {"condition": [{"min": null, "max": null, "negated": null}], "format": ["#"],
             "index_handlers": [["negate"]], "string": "{0}% reduced"}]},
        {"ids": ["doubled"], "English": [
            {"condition": [{"min": null, "max": null, "negated": null}], "format": ["#"],
             "index_handlers": [["negate_and_double"]], "string": "{0}% reduced"}]},
        {"ids": ["scaled"], "English": [
            {"condition": [{"min": null, "max": null, "negated": null}], "format": ["#"],
             "index_handlers": [["thirty_percent"]], "string": "{0}% scaled"}]},
        {"ids": ["sockets"], "English": [
            {"condition": [{"min": 1, "max": 1, "negated": null}], "format": ["ignore"],
             "index_handlers": [[]], "string": "Has 1 Socket"},
            {"condition": [{"min": 2, "max": null, "negated": null}], "format": ["ignore"],
             "index_handlers": [[]], "string": "Has {0} Sockets"}]},
        {"ids": ["negated"], "English": [
            {"condition": [{"min": 1, "max": null, "negated": true}], "format": ["#"],
             "index_handlers": [[]], "string": "{0} negated"}]}
    ]"##;

    const HANDLERS: &str = r##"{
        "tq_noop": {"type": "noop"},
        "by_three": {"type": "int", "divisor": 3.0, "precision": null},
        "negate": {"type": "int", "multiplier": -1.0, "divisor": null, "precision": null},
        "negate_and_double": {"type": "int", "multiplier": -2.0, "divisor": null, "precision": null},
        "thirty_percent": {"type": "int", "multiplier": 30.0, "divisor": 100.0, "precision": null}
    }"##;

    fn stat(id: &str, min: i32, max: i32) -> OutStat {
        OutStat {
            id: id.to_owned(),
            min,
            max,
        }
    }

    fn display(stat: OutStat) -> Result<Display, String> {
        Translator::new(TRANSLATIONS, HANDLERS)?.display("the mod", &[stat])
    }

    #[test]
    fn a_noop_handler_gives_no_conversion_and_plus_keeps_its_sign() {
        let shown = display(stat("life", 3, 9)).unwrap();
        assert_eq!(shown.text, "+{life} to Life");
        assert_eq!(shown.placeholders["life"], "life");
        assert!(shown.conversions.is_empty());
    }

    #[test]
    fn a_plus_format_with_a_negative_value_is_rejected() {
        let message = display(stat("life", -3, 9)).err().unwrap();
        assert!(
            message.contains("life") && message.contains("negative"),
            "{message}"
        );
    }

    #[test]
    fn a_divisor_without_an_exact_decimal_form_and_without_precision_is_rejected() {
        let message = display(stat("third", 1, 2)).err().unwrap();
        assert!(message.contains("by_three"), "{message}");
    }

    #[test]
    fn the_handler_negate_gives_the_divisor_minus_one() {
        let shown = display(stat("reduced", -15, -15)).unwrap();
        assert_eq!(shown.text, "{reduced}% reduced");
        assert_eq!(
            shown.conversions["reduced"],
            OutConversion {
                divisor: -1,
                precision: None
            }
        );
    }

    #[test]
    fn any_other_multiplier_is_rejected() {
        for id in ["doubled", "scaled"] {
            let message = display(stat(id, 1, 2)).err().unwrap();
            assert!(
                message.contains(id) && message.contains("multiplier"),
                "{message}"
            );
        }
    }

    #[test]
    fn the_format_ignore_is_accepted_when_the_text_shows_no_value() {
        let shown = display(stat("sockets", 1, 1)).unwrap();
        assert_eq!(shown.text, "Has 1 Socket");
        assert!(shown.placeholders.is_empty());
        assert!(shown.conversions.is_empty());
    }

    #[test]
    fn the_format_ignore_is_rejected_when_the_text_shows_the_value() {
        let message = display(stat("sockets", 2, 3)).err().unwrap();
        assert!(
            message.contains("sockets") && message.contains("ignore"),
            "{message}"
        );
    }

    #[test]
    fn a_negated_condition_is_rejected() {
        assert!(display(stat("negated", 1, 2)).is_err());
    }

    #[test]
    fn a_stat_without_an_entry_is_rejected() {
        let message = display(stat("unknown", 1, 2)).err().unwrap();
        assert!(message.contains("unknown"), "{message}");
    }

    #[test]
    fn substitute_rejects_unknown_placeholders() {
        let shown = ["{a}".to_owned()];
        for string in ["{1}", "{x}", "{0", "0}", "{0:d}"] {
            assert!(substitute("m", string, &shown).is_err(), "{string}");
        }
    }
}
