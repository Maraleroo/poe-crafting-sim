# 0009: Random rolls with a caller-supplied seeded RNG

- Status: proposed
- Date: 2026-10-20
- Work item: AB#52

## Context
From AB#52 on, every stat rolls a value within its range when its mod is created. Crafting
outcomes will be random, and acceptance tests need identical results on every run
(`writing-acceptance-tests`: seeded RNG with a fixed seed). The developer rules already
require that randomness in `crafting-core` comes from an RNG passed in by the caller.
The `rand` crate needs `getrandom` for its default generators, which does not build for
`wasm32-unknown-unknown` without extra features and compiler flags.

## Decision
- `crafting-core` defines a small trait `Rng` with one method, `next_u64(&mut self) -> u64`.
  Every function that rolls takes `&mut impl Rng`.
- `crafting-core` provides one implementation, `SeededRng::new(seed: u64)`, a SplitMix64
  generator. The same seed always gives the same sequence on every platform.
- Rolling an integer in `min..=max` is one function in `crafting-core`, unit-tested for
  the bounds and for `min == max`, without modulo bias.
- No global or time-based randomness inside `crafting-core`.
- The web page creates its `SeededRng` with a fixed seed for now: every range in the data
  of AB#52 is fixed (1–1, 4–4). The first story whose rolls must differ between visits
  decides where the web page takes its seed from.

## Alternatives considered
- `rand` with `default-features = false` and `SmallRng`: well tested, but a new
  dependency tree and feature juggling for `wasm32-unknown-unknown`, for a few lines of
  code.
- `rand_core` traits only: standard interface, but still a dependency, and the generator
  would have to come from yet another crate.

## Consequences
- No new dependency for randomness.
- SplitMix64 is fast and statistically good enough for a simulator; it is not
  cryptographically secure, which is not needed.
- If a later story needs `rand` features (distributions, weighted choice), a new ADR can
  replace `Rng` with `rand_core::RngCore`; the call sites only pass the generator through.
