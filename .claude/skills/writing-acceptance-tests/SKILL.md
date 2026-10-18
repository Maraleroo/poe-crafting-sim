---
name: writing-acceptance-tests
description: How to turn acceptance criteria into Rust acceptance tests that fail before implementation, including statistical tests for random crafting results. Use when writing or checking acceptance tests.
---

# Acceptance tests

## Location
- `crafting-core/tests/acceptance.rs` declares one module per story: `mod story_<id>;`
- The tests live in `crafting-core/tests/acceptance/story_<id>.rs`.

## Structure
- One test per acceptance criterion, named `ac<n>_<what_it_checks>`.
- Quote the original (German) criterion in a comment above the test.
- Body follows Given / When / Then, marked with comments.
- Use only the public interface of `crafting-core` described in the tech lead's plan.
  If it does not exist yet, the test failing to compile is the expected red state.
- Test behaviour, never implementation details.

## Randomness
- Always use a seeded RNG with a fixed seed, so every run is identical.
- For probabilities, run N trials and compare the observed share with the expected
  share from the mod weights. Tolerance: 5 standard deviations, sqrt(p * (1 - p) / N).
  Choose N so the test finishes in a few seconds.

## Before committing
- Run the tests and confirm each one fails because the feature is missing.
- Commit with `test(core): add acceptance tests for AB#<id>`.
