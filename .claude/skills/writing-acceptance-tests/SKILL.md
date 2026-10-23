---
name: writing-acceptance-tests
description: How to turn acceptance criteria into Rust acceptance tests that fail before implementation, including statistical tests for random crafting results. Use when writing or checking acceptance tests.
---

# Acceptance tests

## Location
- `crafting-core/tests/acceptance.rs` declares one module per story, with an explicit path:
  ```rust
  #[path = "acceptance/story_<id>.rs"]
  mod story_<id>;
  ```
  The file is the crate root of the test target, so a plain `mod story_<id>;` would look
  for `tests/story_<id>.rs` and not compile.
- The tests live in `crafting-core/tests/acceptance/story_<id>.rs`.

## Structure
- One test per acceptance criterion, named `ac<n>_<what_it_checks>`.
- Quote the original (German) criterion in a comment above the test.
- Body follows Given / When / Then, marked with comments.
- Use only the public interface of `crafting-core` described in the tech lead's plan.
  If it does not exist yet, the test failing to compile is the expected red state.
- Test behaviour, never implementation details.

## CI and process stories
Stories that change workflows, skills, agents or other team files get only lean
text-level checks (ADR 0004): a file exists, contains or does not contain a given string,
a line with a given text exists. No parsers for YAML, Markdown or JSON, no structural
models of the file.
Tests check formats and contracts, never concrete versions, tags or SHAs: for example
"every action is pinned to a 40-character SHA with a `# v` comment", not "checkout is
pinned to `3d3c42e…`"; "no job runs on `ubuntu-latest`", not "every job runs on
`ubuntu-24.04`". Otherwise every Dependabot or
toolchain update breaks a protected test. Behaviour that needs a real run is verified after the merge by the
verification task from the tech lead's plan, not by `cargo test`.

## Randomness
- Always use a seeded RNG with a fixed seed, so every run is identical.
- For probabilities, run N trials and compare the observed share with the expected
  share from the mod weights. Tolerance: 5 standard deviations, sqrt(p * (1 - p) / N).
  Choose N so the test finishes in a few seconds.
- Choose N so that the most likely wrong implementation fails clearly.
  If N differs from the PO's example, explain why in the test or in the PR.

## Before committing
- Run the tests and confirm each one fails because the feature is missing.
- Commit with `test(core): add acceptance tests for AB#<id>`.
