//! Randomness for rolls (ADR 0009).
//!
//! `crafting-core` has no global or time-based randomness: every function that rolls takes
//! an [`Rng`] from its caller, so tests can use a fixed seed.

/// A source of random numbers.
pub trait Rng {
    /// The next 64 random bits.
    fn next_u64(&mut self) -> u64;
}

/// SplitMix64 generator. The same seed gives the same sequence on every platform.
#[derive(Debug, Clone)]
pub struct SeededRng {
    state: u64,
}

impl SeededRng {
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }
}

impl Rng for SeededRng {
    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

/// A uniformly distributed integer in `min..=max`, without modulo bias.
///
/// # Panics
/// If `min > max`.
pub fn roll_in_range(rng: &mut impl Rng, min: i32, max: i32) -> i32 {
    assert!(min <= max, "empty range {min}..={max}");
    let span = (i64::from(max) - i64::from(min) + 1) as u64;
    // Values below 2^64 mod span would make the low results more likely; draw again.
    let threshold = span.wrapping_neg() % span;
    loop {
        let draw = rng.next_u64();
        if draw >= threshold {
            let offset = (draw % span) as i64;
            return (i64::from(min) + offset) as i32;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitmix64_matches_the_reference_sequence_for_seed_zero() {
        let mut rng = SeededRng::new(0);
        assert_eq!(rng.next_u64(), 0xE220_A839_7B1D_CDAF);
        assert_eq!(rng.next_u64(), 0x6E78_9E6A_A1B9_65F4);
    }

    #[test]
    fn same_seed_gives_same_sequence() {
        let mut a = SeededRng::new(7);
        let mut b = SeededRng::new(7);
        assert!((0..10).all(|_| a.next_u64() == b.next_u64()));
    }

    #[test]
    fn roll_stays_within_bounds_and_reaches_both_ends() {
        let mut rng = SeededRng::new(1);
        let rolls: Vec<i32> = (0..500).map(|_| roll_in_range(&mut rng, -2, 3)).collect();
        assert!(rolls.iter().all(|value| (-2..=3).contains(value)));
        assert!(rolls.contains(&-2) && rolls.contains(&3));
    }

    #[test]
    fn roll_of_a_single_value_range_returns_it() {
        let mut rng = SeededRng::new(1);
        assert_eq!(roll_in_range(&mut rng, 4, 4), 4);
    }

    #[test]
    fn roll_handles_the_full_i32_range() {
        let mut rng = SeededRng::new(1);
        let value = roll_in_range(&mut rng, i32::MIN, i32::MAX);
        assert!((i32::MIN..=i32::MAX).contains(&value));
    }

    #[test]
    #[should_panic(expected = "empty range")]
    fn roll_of_an_empty_range_is_a_programming_error() {
        roll_in_range(&mut SeededRng::new(1), 5, 4);
    }
}
