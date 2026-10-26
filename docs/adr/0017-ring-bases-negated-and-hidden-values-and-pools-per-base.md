# 0017: Ring bases: negated and hidden values, and a mod pool per base

- Status: accepted
- Date: 2026-10-26
- Work item: AB#185

## Context
AB#185 imports every ring base of the RePoE export 3.29.3.3 that the PO selected: item class
Ring, release state `released`, without the four Royale variants (`RingRoyale1` to
`RingRoyale4`), the six Breach rings and the six Heist rings. That gives 31 bases. Golden
Hoop (unique only) and Jet Ring (unreleased) stay out (PO decision on AB#197). Two-Stone Ring
and Shadowed Ring each occur three times with the same name and different implicits.

The analysis of the raw files found three things that ADR 0015 and ADR 0016 do not cover:

- Pools differ by base. 28 bases have exactly the Iron Ring's 207 mods; Bone Ring and Gloam
  Ring have 23 minion mods more, the Unset Ring 3 mods for socketed gems. Together the data
  holds 233 mods; names are unique and every mod has the same weight in every pool. The
  website builds the Iron Ring's pool from `explicit_mods()`, which offers all mods of
  `data/mods.json`, so the Iron Ring would roll the other bases' mods.
- Five implicits (Dusk, Gloam, Penumbra, Tenebrous Ring on the left line, Nameless Ring on
  both lines) show their value through the handler `negate` (`multiplier` -1, no divisor, no
  precision): a raw -15 is shown as "15% reduced". ADR 0016 makes the converter fail on any
  multiplier, and `Conversion` has only an unsigned divisor. RePoE itself encodes
  `divide_by_one_hundred_and_negate` as a divisor of -100.
- The implicits of the Breach Ring ("Properties are doubled while in a Breach") and the Unset
  Ring ("Has 1 Socket") use the format `ignore`: the text does not show the stat's value. The
  converter rejects every format other than `#` and `+#`.

## Decision
- The converter converts the 31 bases. They are listed in `BASES` in
  `data-import/src/main.rs`, as before; the converter does not select bases itself.
- A mod pool per base: `crafting-core` offers `mod_pool(base_id) -> Option<Vec<ExplicitModDefinition>>`,
  the mods listed in the base's `mod_pool` in `data/base_items.json`, in that order, and
  `None` for an unknown base. It reads `mod_pool` and the mods' `id` with private
  deserialization types; the public types `BaseItem` and `ExplicitModDefinition` keep their
  fields. Loading the data panics if a pool names a mod that `data/mods.json` does not have
  or if two mods have the same `id`. `explicit_mods()` keeps offering every mod of the file.
  The website builds its pool from `mod_pool` of its start base (the Iron Ring) with
  `valid_mods`.
- Negation is a signed divisor: `Conversion::divisor` becomes an `i32` that is never zero. The
  converter turns a handler with `multiplier` -1 and no divisor, addend or fixed value into a
  divisor of -1 with the handler's precision. Every other multiplier, and a multiplier
  together with a divisor, still stops the converter, as does a negative divisor in the raw
  handler, until a story needs it. `display_text` divides by the divisor as before, so the
  rounding rules of `poe-crafting-rules` apply unchanged; without a precision, -1 has an
  exact decimal form. The data file writes `"divisor": -1`.
- The format `ignore` is accepted only when the translation text does not contain the
  placeholder of that stat. The stat stays in `stats` with its raw range, but gets no
  placeholder and no conversion. If the text shows the value, the converter stops as before.

## Alternatives considered
- Keeping `explicit_mods()` as the Iron Ring's pool by splitting the data: `data/mods.json`
  with the mods all bases share (today exactly the Iron Ring's 207), the base-specific mods in
  a second file. No change to earlier tests, but the Iron Ring keeps its pool only because its
  pool happens to be the shared set; it fails with the first base that lacks a shared mod
  (Breach or Heist rings, amulets).
- Fields `mod_pool` on `BaseItem` and `id` on `ExplicitModDefinition`: the pool travels with
  the base, but every struct literal of these types in tests and code changes, for a value
  only the data loading needs.
- A field `negate: bool` in `Conversion`: explicit, but a second parameter for what RePoE
  expresses with the divisor's sign, and more changed struct literals.
- A table of negated stats read by `display_text`: rejected for the same reason as the global
  conversion table in ADR 0016 (`display_text` would need the embedded data).

## Consequences
- ADR 0016's rule "the converter fails on a handler with a multiplier" now has the exception
  `multiplier` -1. Its acceptance test case uses another unregulated handler
  (`30%_of_value`); the PO confirmed this on AB#185.
- Earlier acceptance tests that took `explicit_mods()` as the Iron Ring's pool or checked
  that the website calls `explicit_mods`, and the two that build a `Conversion` from an
  unsigned divisor, are adapted in AB#185's test commit (PO decision on AB#197). Tests that
  only use all mods as some pool stay as they are.
- `data/base_items.json` grows to about 200 KB, because every base lists its pool; the website
  embeds it as before and loads nothing at runtime.
- The website still shows only the Iron Ring; choosing another base needs a story of its
  own, which can use `mod_pool` for that base.
