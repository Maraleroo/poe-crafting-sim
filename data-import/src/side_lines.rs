//! The implicit lines of the PO's rules on affix limits and modifier magnitudes (ADR 0022,
//! `poe-crafting-rules`, "Affix limits" and the paragraph on "X% increased ... Modifier
//! magnitudes").

use crate::output::{OutModDefinition, OutSides};

/// The sides of an item, in the order of `OutSides`.
const SIDES: [&str; 2] = ["Prefix", "Suffix"];

/// The two kinds of lines, with the endings that identify them.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// "<N> Prefix Modifier(s) allowed" and "<N> Suffix Modifier(s) allowed".
    Limit,
    /// "<X>% increased Prefix Modifier magnitudes" and the same for suffixes.
    Increase,
}

/// What the lines of the implicits of one base say, per side.
#[derive(Debug, PartialEq, Eq)]
pub struct SideLines {
    /// The change of the affix limits; `None` if no implicit has such a line.
    pub limit_changes: Option<OutSides<i32>>,
    /// The increase of the modifier magnitudes in percent; `None` if no implicit has such a line.
    pub magnitude_increases: Option<OutSides<u32>>,
}

/// Reads the lines of the implicits that change the affix limits or increase the modifier
/// magnitudes. A side without a line gets 0. The value comes from the stat that the placeholder
/// of the line names.
///
/// # Errors
/// If a line ends like one of these lines but does not fit it (for example "Explicit Modifier
/// magnitudes" or "reduced ... magnitudes"), if the stat of a line is missing or has a range
/// instead of a fixed value, or if a side has two lines of the same kind.
pub fn side_lines(base_id: &str, implicits: &[OutModDefinition]) -> Result<SideLines, String> {
    let mut limits: [Option<i32>; 2] = [None; 2];
    let mut increases: [Option<i32>; 2] = [None; 2];
    for implicit in implicits {
        for line in implicit.text.split('\n') {
            let Some((kind, side, name)) = parse_line(base_id, line)? else {
                continue;
            };
            let value = fixed_value(base_id, implicit, line, name)?;
            let slot = match kind {
                Kind::Limit => &mut limits[side],
                Kind::Increase => &mut increases[side],
            };
            if slot.replace(value).is_some() {
                return Err(format!(
                    "the base item {base_id} has more than one line of the kind of {line:?} for \
                     the side {}",
                    SIDES[side]
                ));
            }
        }
    }
    let magnitude_increases = if increases.iter().any(Option::is_some) {
        let percent = |side: usize| {
            let value = increases[side].unwrap_or(0);
            u32::try_from(value).map_err(|_| {
                format!(
                    "the base item {base_id}: the increase of the {} magnitudes is {value}, but \
                     must not be below 0",
                    SIDES[side]
                )
            })
        };
        Some(OutSides {
            prefixes: percent(0)?,
            suffixes: percent(1)?,
        })
    } else {
        None
    };
    let limit_changes = limits.iter().any(Option::is_some).then(|| OutSides {
        prefixes: limits[0].unwrap_or(0),
        suffixes: limits[1].unwrap_or(0),
    });
    Ok(SideLines {
        limit_changes,
        magnitude_increases,
    })
}

/// The kind, the side (index in `SIDES`) and the placeholder name of a line, `None` for a line
/// that does not end like a line of either kind.
fn parse_line<'a>(base_id: &str, line: &'a str) -> Result<Option<(Kind, usize, &'a str)>, String> {
    let (kind, head) = if let Some(head) = line
        .strip_suffix(" Modifier allowed")
        .or_else(|| line.strip_suffix(" Modifiers allowed"))
    {
        (Kind::Limit, head)
    } else if let Some(head) = line.strip_suffix(" Modifier magnitudes") {
        (Kind::Increase, head)
    } else {
        return Ok(None);
    };
    let fitting = SIDES.iter().enumerate().find_map(|(side, word)| {
        let value = head.strip_suffix(&format!(" {word}"))?;
        let value = match kind {
            Kind::Limit => value.strip_prefix('+').unwrap_or(value),
            Kind::Increase => value.strip_suffix("% increased")?,
        };
        let name = value.strip_prefix('{')?.strip_suffix('}')?;
        (!name.contains(['{', '}'])).then_some((side, name))
    });
    match fitting {
        Some((side, name)) => Ok(Some((kind, side, name))),
        None => Err(format!(
            "the base item {base_id}: the line {line:?} ends like a line of the affix limits or \
             modifier magnitudes but does not fit one"
        )),
    }
}

/// The fixed value of the stat that the placeholder `name` of `implicit` names.
fn fixed_value(
    base_id: &str,
    implicit: &OutModDefinition,
    line: &str,
    name: &str,
) -> Result<i32, String> {
    let stat_id = implicit.placeholders.get(name).ok_or_else(|| {
        format!("the base item {base_id}: the placeholder {name:?} of the line {line:?} is unknown")
    })?;
    let stat = implicit
        .stats
        .iter()
        .find(|stat| stat.id == *stat_id)
        .ok_or_else(|| {
            format!("the base item {base_id}: the stat {stat_id:?} of the line {line:?} is missing")
        })?;
    if stat.min == stat.max {
        Ok(stat.min)
    } else {
        Err(format!(
            "the base item {base_id}: the stat {stat_id:?} of the line {line:?} has the range {} \
             to {}, but must have a fixed value",
            stat.min, stat.max
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::output::OutStat;

    /// An implicit whose placeholders are named after the stats, with the given fixed values or
    /// ranges.
    fn implicit(text: &str, stats: &[(&str, i32, i32)]) -> OutModDefinition {
        OutModDefinition {
            family: "Invented".to_owned(),
            level: 1,
            tags: Vec::new(),
            stats: stats
                .iter()
                .map(|(id, min, max)| OutStat {
                    id: (*id).to_owned(),
                    min: *min,
                    max: *max,
                })
                .collect(),
            text: text.to_owned(),
            placeholders: stats
                .iter()
                .map(|(id, _, _)| ((*id).to_owned(), (*id).to_owned()))
                .collect::<BTreeMap<_, _>>(),
            conversions: BTreeMap::new(),
        }
    }

    fn read(text: &str, stats: &[(&str, i32, i32)]) -> Result<SideLines, String> {
        side_lines("invented", &[implicit(text, stats)])
    }

    #[test]
    fn a_base_without_such_lines_has_neither_value() {
        let lines = read(
            "Has 1 Socket\n{a}% increased Global Defences",
            &[("a", 5, 7)],
        )
        .unwrap();
        assert_eq!(lines.limit_changes, None);
        assert_eq!(lines.magnitude_increases, None);
    }

    #[test]
    fn limit_lines_give_the_change_per_side_with_a_missing_side_0() {
        let stats = [("p", 1, 1), ("s", -2, -2)];
        let both = read(
            "+{p} Prefix Modifier allowed\n{s} Suffix Modifiers allowed",
            &stats,
        )
        .unwrap();
        assert_eq!(
            both.limit_changes,
            Some(OutSides {
                prefixes: 1,
                suffixes: -2
            })
        );
        let one = read("+{p} Prefix Modifiers allowed", &stats).unwrap();
        assert_eq!(
            one.limit_changes,
            Some(OutSides {
                prefixes: 1,
                suffixes: 0
            })
        );
        assert_eq!(one.magnitude_increases, None);
    }

    #[test]
    fn increase_lines_give_the_percent_per_side_with_a_missing_side_0() {
        let stats = [("p", 25, 25), ("s", 50, 50)];
        let both = read(
            "{p}% increased Prefix Modifier magnitudes\n{s}% increased Suffix Modifier magnitudes",
            &stats,
        )
        .unwrap();
        assert_eq!(
            both.magnitude_increases,
            Some(OutSides {
                prefixes: 25,
                suffixes: 50
            })
        );
        let one = read("{s}% increased Suffix Modifier magnitudes", &stats).unwrap();
        assert_eq!(
            one.magnitude_increases,
            Some(OutSides {
                prefixes: 0,
                suffixes: 50
            })
        );
        assert_eq!(one.limit_changes, None);
    }

    #[test]
    fn the_lines_are_found_in_every_implicit_and_among_other_lines() {
        let first = implicit(
            "Implicit Modifiers Cannot Be Changed\n+{p} Prefix Modifier allowed",
            &[("p", 1, 1)],
        );
        let second = implicit(
            "{s}% increased Suffix Modifier magnitudes",
            &[("s", 25, 25)],
        );
        let lines = side_lines("invented", &[first, second]).unwrap();
        assert_eq!(lines.limit_changes.unwrap().prefixes, 1);
        assert_eq!(lines.magnitude_increases.unwrap().suffixes, 25);
    }

    #[test]
    fn a_range_instead_of_a_fixed_value_is_rejected() {
        for text in [
            "+{a} Prefix Modifiers allowed",
            "{a}% increased Suffix Modifier magnitudes",
        ] {
            let message = read(text, &[("a", 1, 3)]).unwrap_err();
            assert!(
                message.contains("invented") && message.contains("fixed value"),
                "{message}"
            );
        }
    }

    #[test]
    fn two_lines_of_the_same_kind_for_a_side_are_rejected() {
        for text in [
            "+{a} Prefix Modifier allowed\n+{a} Prefix Modifiers allowed",
            "{a}% increased Suffix Modifier magnitudes\n{a}% increased Suffix Modifier magnitudes",
        ] {
            let message = read(text, &[("a", 1, 1)]).unwrap_err();
            assert!(message.contains("more than one"), "{message}");
        }
        let in_two_implicits = side_lines(
            "invented",
            &[
                implicit("+{a} Prefix Modifier allowed", &[("a", 1, 1)]),
                implicit("+{a} Prefix Modifier allowed", &[("a", 1, 1)]),
            ],
        );
        assert!(in_two_implicits.is_err());
    }

    #[test]
    fn a_line_that_ends_like_one_but_does_not_fit_is_rejected() {
        for text in [
            "{a}% increased Explicit Modifier magnitudes",
            "{a}% reduced Prefix Modifier magnitudes",
            "+{a} Explicit Modifiers allowed",
            "+{a}% Prefix Modifiers allowed",
            "5 Prefix Modifiers allowed",
            "{a}% Prefix Modifier magnitudes",
        ] {
            let message = read(text, &[("a", 1, 1)])
                .err()
                .unwrap_or_else(|| panic!("{text} was accepted"));
            assert!(
                message.contains("invented") && message.contains(text),
                "{message}"
            );
        }
    }

    #[test]
    fn a_line_with_an_unknown_placeholder_is_rejected() {
        let message = read("+{x} Prefix Modifier allowed", &[("a", 1, 1)]).unwrap_err();
        assert!(message.contains("unknown"), "{message}");
    }
}
