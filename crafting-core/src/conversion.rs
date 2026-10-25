//! The conversion the game applies to a stat value for display (ADR 0016).

use serde::Deserialize;

/// How the game converts the value of a stat for display: the raw value divided by
/// `divisor`. How the result is rounded and shown is defined in `poe-crafting-rules`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Conversion {
    /// Divisor, greater than zero.
    pub divisor: u32,
    /// Number of decimals the game rounds to, or none if the exact value is shown.
    pub precision: Option<u32>,
}

/// The highest precision, so that a value scaled by a power of ten fits into 64 bits.
const MAX_DECIMALS: u32 = 9;

impl Conversion {
    /// Checks that the value can be shown as `poe-crafting-rules` defines it.
    pub(crate) fn check(&self) -> Result<(), String> {
        if self.divisor == 0 {
            return Err("the divisor is zero".to_owned());
        }
        match self.precision {
            Some(precision) if precision > MAX_DECIMALS => {
                Err(format!("the precision {precision} is above {MAX_DECIMALS}"))
            }
            None if exact_scale(self.divisor).is_none() => Err(format!(
                "without a precision, the divisor {} has no exact decimal form with at most \
                 {MAX_DECIMALS} decimals",
                self.divisor
            )),
            _ => Ok(()),
        }
    }

    /// The value as the game shows it. With a precision, the converted value is computed as a
    /// 32-bit floating-point number and rounded to the precision, an exact half to the even
    /// digit. Without a precision, the exact value is shown. Trailing zeros are dropped.
    ///
    /// # Panics
    /// If the conversion is invalid (see `check`). That is an error in the data.
    pub(crate) fn show(&self, raw: i32) -> String {
        if let Err(message) = self.check() {
            panic!("invalid conversion: {message}");
        }
        match self.precision {
            Some(precision) => {
                let converted = raw as f32 / self.divisor as f32;
                let scale = 10_i64.pow(precision);
                // The 32-bit value is exact in 64 bits, and so is its product with a power of
                // ten up to 10^9, so an exact half stays exact.
                let scaled = (f64::from(converted) * scale as f64).round_ties_even() as i64;
                decimal(scaled, scale)
            }
            None => {
                let scale = exact_scale(self.divisor).expect("checked above");
                decimal(i64::from(raw) * (scale / i64::from(self.divisor)), scale)
            }
        }
    }
}

/// The smallest power of ten up to 10^9 that is a multiple of `divisor`.
fn exact_scale(divisor: u32) -> Option<i64> {
    let mut scale = 1_i64;
    for _ in 0..=MAX_DECIMALS {
        if scale % i64::from(divisor) == 0 {
            return Some(scale);
        }
        scale *= 10;
    }
    None
}

/// `scaled / scale` as a decimal number without trailing zeros; `scale` is a power of ten.
fn decimal(scaled: i64, scale: i64) -> String {
    let sign = if scaled < 0 { "-" } else { "" };
    let whole = scaled.abs() / scale;
    let digits = scale.to_string().len() - 1;
    if digits == 0 {
        return format!("{sign}{whole}");
    }
    let fraction = format!("{:0digits$}", scaled.abs() % scale);
    let fraction = fraction.trim_end_matches('0');
    if fraction.is_empty() {
        format!("{sign}{whole}")
    } else {
        format!("{sign}{whole}.{fraction}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn per_second() -> Conversion {
        Conversion {
            divisor: 60,
            precision: Some(1),
        }
    }

    fn percent() -> Conversion {
        Conversion {
            divisor: 100,
            precision: None,
        }
    }

    #[test]
    fn rounds_the_32_bit_value_to_the_precision_as_the_rule_says() {
        // The examples of the rule: 2.05 is not exact in 32 bits and gives 2, 2.15 gives 2.2,
        // the exact half 2.25 goes to the even digit.
        for (raw, shown) in [
            (60, "1"),
            (120, "2"),
            (123, "2"),
            (129, "2.2"),
            (135, "2.2"),
            (195, "3.2"),
            (3840, "64"),
        ] {
            assert_eq!(per_second().show(raw), shown, "raw value {raw}");
        }
    }

    #[test]
    fn without_a_precision_the_exact_value_is_shown_without_trailing_zeros() {
        for (raw, shown) in [(20, "0.2"), (21, "0.21"), (30, "0.3"), (100, "1"), (0, "0")] {
            assert_eq!(percent().show(raw), shown, "raw value {raw}");
        }
        let halves = Conversion {
            divisor: 8,
            precision: None,
        };
        assert_eq!(halves.show(1), "0.125");
    }

    #[test]
    fn negative_values_keep_their_sign() {
        assert_eq!(percent().show(-21), "-0.21");
        assert_eq!(per_second().show(-135), "-2.2");
        assert_eq!(per_second().show(-1), "0");
    }

    #[test]
    fn a_precision_of_zero_shows_whole_numbers() {
        let whole = Conversion {
            divisor: 10,
            precision: Some(0),
        };
        assert_eq!(whole.show(25), "2");
        assert_eq!(whole.show(26), "3");
    }

    #[test]
    fn invalid_conversions_are_found() {
        let invalid = |divisor, precision| Conversion { divisor, precision }.check().is_err();
        assert!(invalid(0, Some(1)));
        assert!(invalid(0, None));
        assert!(invalid(60, None));
        assert!(invalid(3, None));
        assert!(invalid(10, Some(10)));
        assert!(!invalid(60, Some(1)));
        assert!(!invalid(100, None));
        assert!(!invalid(1_000_000_000, None));
    }

    #[test]
    #[should_panic(expected = "invalid conversion")]
    fn showing_a_value_with_an_invalid_conversion_is_a_programming_error() {
        Conversion {
            divisor: 3,
            precision: None,
        }
        .show(1);
    }
}
