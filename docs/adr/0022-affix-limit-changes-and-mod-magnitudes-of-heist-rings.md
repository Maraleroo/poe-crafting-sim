# 0022: Affix limit changes and mod magnitudes of the Heist rings

- Status: accepted
- Date: 2026-10-31
- Work item: AB#189

## Context
AB#189 adds the six Heist rings (`RingE1` to `RingE6` in RePoE: Cogwork, Geodesic, Composite,
Manifold, Ratcheting and Helical Ring), which ADR 0017 left out. The PO confirmed that this
replaces criterion 2 of AB#185 for these six rings. RePoE gives all six the Iron Ring's pool of
207 mods with the same weights. Each has one implicit (level 30) with five stats:
`local_maximum_prefixes_allowed_+`, `local_maximum_suffixes_allowed_+`,
`local_implicit_mod_cannot_be_changed` (format `ignore`), `local_prefix_mod_effect_+%` and
`local_suffix_mod_effect_+%`. On every ring at least one of the two magnitude stats is 0.

The PO's rules in `poe-crafting-rules`:
- "Affix limits": a line "+N Prefix Modifiers allowed" or "-N Suffix Modifiers allowed" (also
  singular) changes the limit of that side by N, never below 0; the total limit stays.
- Magnitudes: where a line "X% increased Prefix Modifier magnitudes" (or Suffix, or Explicit)
  applies, each affected stat value of the game data is multiplied by (1 + X/100), rounded
  down to a whole number, and then converted for display as usual. The normal view shows this
  value; the extended text (Alt view, Ctrl+Alt+C) shows the unscaled value and range.
- The name of a magic item has all its prefixes, the base name and all its suffixes, each side
  in the order its mods were added.

The converter cannot convert these implicits today, for three reasons:
- The translations of the two "allowed" stats have a singular for -1 to 1 and an
  unconditional plural. For a value of 1 both hold, and the converter requires exactly one.
- Both use the format `+#`, also for negative values, which the converter rejects.
- A magnitude stat of 0 has no translation whose condition holds (the conditions are "at
  least 1" and "at most -1"), which the converter also rejects. Criterion 2 says that a
  magnitude of 0 shows no line.

Earlier decisions that limit the design: ADR 0017 (public types keep their fields, because the
protected tests build them as struct literals), ADR 0020 (the converter recognizes the line of
a PO rule in the translated text and writes a field on the base; no table of stat identifiers
in code; `crafting-core` never interprets game texts) and ADR 0021 (`AffixLimits` is the only
place of the limits, and AB#189 adds the changes from the lines there).

## Decision
- The converter converts 43 bases: the 37 of ADR 0020 and `RingE1` to `RingE6`, listed in
  `BASES` in `data-import/src/main.rs`.
- Three translation rules in the converter, none of which changes the output for the 37 bases:
  - Within one entry of `stat_translations.json`, the first translation whose conditions hold
    for the whole range is used, as in the game data's order. Across entries, exactly one entry
    must still have such a translation.
  - The format `+#` puts a `+` before the placeholder if the whole range is at least 0, and
    none if the whole range is below 0; the value then shows its own minus sign
    (`-1 Prefix Modifier allowed`). A range across 0 still stops the converter.
  - A stat whose range is exactly 0 and for which no translation holds gets no line. It stays
    in `stats`, without a placeholder and a conversion, like a stat with the format `ignore`.
    A stat with any other value and no translation still stops the converter.
- The converter recognizes the lines of the PO's rules in the translated implicit text of a
  base and writes two optional fields on its entry in `data/base_items.json`:
  - `affix_limit_changes: { "prefixes": N, "suffixes": N }` from the lines
    `<N> Prefix Modifier(s) allowed` and `<N> Suffix Modifier(s) allowed` (singular or plural),
    a side without a line 0.
  - `magnitude_increases: { "prefixes": X, "suffixes": X }` (percent) from the lines
    `<X>% increased Prefix Modifier magnitudes` and `<X>% increased Suffix Modifier
    magnitudes`, a side without a line 0.
  A base without such lines has neither field, so the entries of the other bases stay byte for
  byte the same. The value comes from the stat that the line's placeholder names and must be
  fixed (minimum equal to maximum). The converter stops if such a stat has a range, if a side
  has two lines of the same kind, or if a line ends like one of these lines but does not fit
  them, for example "Explicit Modifier magnitudes" or "reduced ... magnitudes": the rule names
  "Explicit", but no base in the data has it, and how it combines with a side's own line is not
  in `poe-crafting-rules`.
- `crafting-core` reads both fields once with a private deserialization type, like
  `damage_type_lock` (ADR 0020). `BaseItem` gets no new field. Loading the data panics if a
  field is malformed.
- `Item::affix_limits(&self) -> Option<AffixLimits>`: `AffixLimits::of(rarity)` with each
  side's limit changed by the base's `affix_limit_changes`, never below 0; the total limit is
  unchanged; `None` for a unique item. The change is applied next to `AffixLimits`, which stays
  the only place of the limits. `valid_pool` uses it instead of `AffixLimits::of`.
  `AffixLimits::of(rarity)` stays as it is.
- An item keeps the rolled values of its mods unscaled. `Item::explicit_text(&self, explicit:
  &ExplicitMod) -> String` is the text of a prefix or suffix in the normal view: each stat
  value is multiplied by (100 + X) and divided by 100, rounded down (integer arithmetic, no
  floating point), where X is the base's increase for the mod's side; then it is converted and
  inserted as by `Mod::display_text`. With X = 0 it equals `display_text`. `Mod::display_text`
  stays the unscaled text and is still used for implicits. The page shows prefixes and
  suffixes through `explicit_text`.
- `Item::name()` joins the names of all prefixes, the base name and the names of all
  suffixes, each list in its order. The formula holds for every rarity; names of rare items
  are still not defined by a rule.

## Alternatives considered
- `crafting-core` reads the stat identifiers of the implicits (`local_maximum_prefixes_allowed_+`
  and the others): no new data fields, and the values would come from the item's implicits.
  Rejected for the reason of ADR 0020: it puts stat identifiers into code and does not follow
  the PO's rules, which are about the lines.
- Scaling the values when a mod is rolled: no work at display time, but the item would lose
  the unscaled value and range that the extended text shows, and the scaled value would be
  wrong if the mod ever moved to another base.
- Scaling on the page: the rule would live in the UI, against the architecture.
- Accepting "Explicit Modifier magnitudes" now: no base has the line, and its combination with
  a side's own line would be a guess.
- Requiring exactly one matching translation and special-casing the two "allowed" stats:
  the game data's order already decides, and a special case would name stats in code.

## Consequences
- Both rules have unit tests with invented data. The converter's new rules have unit tests
  with invented raw files; its existing stop cases stay.
- The converter depends on the English wording of the lines in `stat_translations.json`. If a
  new RePoE export words them differently, the converter stops or finds no line. Then the
  limits or magnitudes are missing, and the tests of AB#189 fail.
- The limit changes and magnitudes belong to the base, like the damage type lock. If a currency
  can change implicits later, they have to come from the item's mods; the line "Implicit
  Modifiers Cannot Be Changed" is relevant for that story and has no effect today.
- Rounding down is exact for the values in the pools of the six rings, which are all at least
  0. For negative values the rule's "rounded down" is applied as written, but it is not backed
  by evidence.
- The simulator has no extended view of an item yet. When it gets one, it shows the unscaled
  value from `Mod` and marks the mod as the rule says.
