# 0023: Sorted and merged mod lines in the normal view

- Status: accepted
- Date: 2026-10-31
- Work item: AB#258

## Context
AB#258 adds the PO's rule to `poe-crafting-rules` ("Item basics"): in the normal view, the
explicit mod lines of an item are sorted by the order of their stats in the game's stat
descriptions (RePoE `stat_translations`). Lines of different mods with the same stat are shown
as one line with their values added up; a fractured mod is not merged. Implicit mods are shown
above the explicit mods and are never merged with them. The PO confirmed that criterion 2 of
AB#258 replaces criterion 6 of AB#189 (prefixes, then suffixes).

Today the page shows all prefixes, then all suffixes, each in the order they were added, one
text per mod from `Item::explicit_text` (ADR 0022). The data files carry each mod's text with
its lines already in the game's order, but not the position of the stats in
`stat_translations.json`, so the order across mods is not in the data.

Facts from the RePoE export 3.29.3.3 (the raw files of `data/README.md`): the 233 mods in
`data/mods.json` show 45 stats; each line of each mod comes from exactly one entry of
`stat_translations.json`, and every line names at least one stat. Only one stat is shown by
mods of more than one family: `base_item_found_rarity_+%` (families `ItemFoundRarityIncrease`,
suffix, and `ItemFoundRarityIncreasePrefix`, prefix), with the same line text and without a
conversion. The simulator has no fractured mods.

Earlier decisions that limit the design: ADR 0017 (public types keep their fields, because the
protected tests build them as struct literals), ADR 0015 and ADR 0016 (the converter writes
the data files from the raw files; values stored as RePoE has them) and ADR 0022 (the item
keeps unscaled values; the normal view scales them by the base's increase for the mod's side).

## Decision
- The converter writes a new top-level object `stat_order` into `data/mods.json`. It maps
  every stat that a line of a mod in the file shows (every stat a placeholder names) to the
  position (0-based) of the entry of `stat_translations.json` that the converter used for it.
  The keys are sorted, so the output stays deterministic. The converter stops if one stat would
  get two different positions. The entries of `mods` do not change. `convert(raw, base_ids)`
  and `Converted` keep their signatures and fields.
- `crafting-core` reads `stat_order` once with a private deserialization type, like the base
  effects of ADR 0022. No public struct gets a new field. Loading the data panics if a stat that
  a placeholder of a mod in `data/mods.json` names has no position, or if the stats of one line
  have different positions.
- `Item::explicit_texts(&self) -> Vec<String>`: the explicit lines of the item in the normal
  view, one entry per line:
  1. Each line of each prefix and suffix, with the stats its placeholders name and their
     values increased by the base's increase for the mod's side, rounded down (ADR 0022).
  2. Lines of different mods that name the same set of stats become one line: each stat's
     value is the sum of the increased values.
  3. Each value is converted as by `Mod::display_text` (conversion of the mod) and inserted
     into the line's text.
  4. The lines are sorted by the position of their stats. After step 2 no two lines share a
     position; the sort is stable all the same (prefixes before suffixes, each in the order
     added).
  Implicit lines are not part of the result. The function panics if a line names a stat
  without a position, which only happens for a mod that is not from the game data.
- `Item::explicit_text`, `Mod::display_text`, `Item::name()` and the lists `prefixes` and
  `suffixes` (in the order the mods were added) stay as they are. Only the display sorts and
  merges; the item itself does not change.
- `web`: `ItemView.explicits` is `item.explicit_texts()`. The implicits keep their own lines
  above the explicit lines, from `Mod::display_text` as before.
- Fractured mods are not modelled. When a story adds them, their lines are left out of the
  merge in step 2, as the rule says.

## Alternatives considered
- A field with the positions of each mod's lines on the mod entries: `ModDefinition` and `Mod`
  would need a new field, which breaks the struct literals of the protected tests (ADR 0017).
- A separate data file for the stat order: one more output of the converter and one more file
  to load, with no advantage over a field in the file that already holds the mods.
- Sorting the lists `prefixes` and `suffixes` themselves: the name of a magic item uses the
  order in which the mods were added (rule in "Item basics", AB#189 criterion 5).
- Sorting and merging on the page: the rule would live in the UI, against the architecture.
- Ordering by a list of stat identifiers written in code: it would put game data into code and
  would not follow a new export.

## Consequences
- The converter run changes `data/mods.json` only by the new `stat_order` object (45 stats);
  `data/base_items.json` stays byte for byte the same. `data/README.md` describes the field.
- How to add up values of a stat with a conversion (convert the sum, or add the converted
  values) is not in `poe-crafting-rules`. No such case is in today's data. A unit test in
  `crafting-core` checks over the data that every stat shown by mods of more than one family has
  no conversion and the same line text in each of them. A new export with such a case fails
  that test, and the question goes to the PO instead of being guessed.
- The sum uses the increased values of each mod (ADR 0022: each stat value is increased and
  rounded down). In today's data this matters only on a rare Heist ring with both rarity mods,
  which the page cannot make yet.
- Unit tests in `web` can no longer show invented mods through `ItemView`, because their stats
  have no position. They take mods from the pools of `crafting-core`, chosen by their
  structure, because files under `web/` must not contain mod names, mod texts or stat
  identifiers (AB#62 criterion 5, AB#186 criterion 7).
- The rule and its order are tested in `crafting-core`; the page itself is checked in the
  local preview (ADR 0004).
