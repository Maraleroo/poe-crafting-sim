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

/// A uniformly distributed integer in `0..bound`, without modulo bias.
///
/// # Panics
/// If `bound` is zero.
pub fn roll_below(rng: &mut impl Rng, bound: u64) -> u64 {
    assert!(bound > 0, "empty range 0..0");
    // Values below 2^64 mod bound would make the low results more likely; draw again.
    let threshold = bound.wrapping_neg() % bound;
    loop {
        let draw = rng.next_u64();
        if draw >= threshold {
            return draw % bound;
        }
    }
}

/// A uniformly distributed integer in `min..=max`, without modulo bias.
///
/// # Panics
/// If `min > max`.
pub fn roll_in_range(rng: &mut impl Rng, min: i32, max: i32) -> i32 {
    assert!(min <= max, "empty range {min}..={max}");
    let span = (i64::from(max) - i64::from(min) + 1) as u64;
    (i64::from(min) + roll_below(rng, span) as i64) as i32
}

/// The index of one of the `weights`, chosen with a probability proportional to its weight.
///
/// # Panics
/// If `weights` is empty or contains a zero. Both are programming errors: the game data is
/// checked when it is parsed.
pub fn choose_weighted(rng: &mut impl Rng, weights: &[u32]) -> usize {
    assert!(!weights.is_empty(), "no weights to choose from");
    assert!(weights.iter().all(|&weight| weight > 0), "zero weight");
    let total: u64 = weights.iter().map(|&weight| u64::from(weight)).sum();
    let mut rest = roll_below(rng, total);
    for (index, &weight) in weights.iter().enumerate() {
        let weight = u64::from(weight);
        if rest < weight {
            return index;
        }
        rest -= weight;
    }
    unreachable!("the roll is below the sum of the weights")
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

    #[test]
    fn roll_below_stays_below_the_bound_and_reaches_every_value() {
        let mut rng = SeededRng::new(1);
        let rolls: Vec<u64> = (0..500).map(|_| roll_below(&mut rng, 3)).collect();
        assert!(rolls.iter().all(|&value| value < 3));
        assert!((0..3).all(|value| rolls.contains(&value)));
    }

    #[test]
    #[should_panic(expected = "empty range")]
    fn roll_below_zero_is_a_programming_error() {
        roll_below(&mut SeededRng::new(1), 0);
    }

    #[test]
    fn weighted_choice_follows_the_weights() {
        let weights = [1, 3, 6];
        let mut rng = SeededRng::new(1);
        let mut counts = [0u32; 3];
        for _ in 0..100_000 {
            counts[choose_weighted(&mut rng, &weights)] += 1;
        }
        for (count, expected) in counts.iter().zip([10_000.0, 30_000.0, 60_000.0]) {
            // Five standard deviations of a binomial with p up to 0.6 stay below 800.
            assert!((f64::from(*count) - expected).abs() < 800.0, "{counts:?}");
        }
    }

    #[test]
    fn weighted_choice_of_a_single_weight_returns_it() {
        assert_eq!(choose_weighted(&mut SeededRng::new(1), &[7]), 0);
    }

    #[test]
    fn weighted_choice_handles_weights_whose_sum_exceeds_u32() {
        let mut rng = SeededRng::new(1);
        assert!(choose_weighted(&mut rng, &[u32::MAX, u32::MAX]) < 2);
    }

    #[test]
    #[should_panic(expected = "no weights")]
    fn weighted_choice_without_weights_is_a_programming_error() {
        choose_weighted(&mut SeededRng::new(1), &[]);
    }

    #[test]
    #[should_panic(expected = "zero weight")]
    fn weighted_choice_with_a_zero_weight_is_a_programming_error() {
        choose_weighted(&mut SeededRng::new(1), &[1, 0]);
    }
}
