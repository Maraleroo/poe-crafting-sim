//! The item level field of the page: the allowed values and the reading of its text.
//!
//! The range and the reading are rules of the page (PO decision on AB#187, ADR 0019), not game
//! rules. The game rule itself, which mods roll at which item level, is part of the rule for
//! adding a random mod in `crafting-core`.

use std::ops::RangeInclusive;

use crafting_core::DEFAULT_ITEM_LEVEL;

/// The item level of a new workbench and after the choice of a base item
/// (`poe-crafting-rules`, "Item level"): the item level of a new item in `crafting-core`.
pub const DEFAULT: u32 = DEFAULT_ITEM_LEVEL;

/// The item levels the field accepts.
pub const ALLOWED: RangeInclusive<u32> = 1..=100;

/// The item level that the text of the field stands for, or `None` if the text is not a whole
/// number from 1 to 100. Only digits count, so a sign, a decimal point, an exponent, spaces and
/// an empty text are invalid (a browser gives an empty text for what it cannot read as a
/// number). Leading zeros are allowed.
pub fn parse(text: &str) -> Option<u32> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse().ok().filter(|level| ALLOWED.contains(level))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_is_an_allowed_item_level() {
        assert!(ALLOWED.contains(&DEFAULT));
    }

    #[test]
    fn the_default_is_the_item_level_of_a_new_item() {
        assert_eq!(DEFAULT, DEFAULT_ITEM_LEVEL);
    }

    #[test]
    fn whole_numbers_from_1_to_100_are_read() {
        for level in ALLOWED {
            assert_eq!(parse(&level.to_string()), Some(level));
        }
    }

    #[test]
    fn numbers_outside_the_range_are_invalid() {
        for text in ["0", "101", "1000", "99999999999999999999"] {
            assert_eq!(parse(text), None, "{text:?}");
        }
    }

    #[test]
    fn text_that_is_not_a_whole_number_is_invalid() {
        for text in [
            "", " ", "abc", "-5", "+5", "1e2", "50.0", "50,5", "5.5", ".", " 50", "50 ", "5 0",
            "５０",
        ] {
            assert_eq!(parse(text), None, "{text:?}");
        }
    }

    #[test]
    fn leading_zeros_are_read_as_the_number() {
        assert_eq!(parse("050"), Some(50));
        assert_eq!(parse("0000000000000000000000001"), Some(1));
        assert_eq!(parse("00"), None);
    }
}
