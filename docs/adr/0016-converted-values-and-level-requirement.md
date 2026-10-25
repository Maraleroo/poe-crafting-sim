# 0016: Converted stat values and the level requirement

- Status: accepted
- Date: 2026-10-25
- Work item: AB#167

## Context
AB#167 replaces the Iron Ring's mini pool with its full pool from RePoE (ADR 0015) and shows
item values as in the game. Two things are new for the data model:

- Some stats are stored in another unit than shown. In the Iron Ring's pool, life
  regeneration is stored per minute and shown per second (`per_minute_to_per_second`:
  divisor 60, precision 1), and leech is stored in permyriad and shown in percent
  (`divide_by_one_hundred`: divisor 100, no precision). RePoE names these handlers per stat
  in the `index_handlers` of a `stat_translations.json` entry and defines them in a sixth raw
  file, `stat_value_handlers.json`, which ADR 0015 does not list. `poe-crafting-rules`
  defines how the game computes and rounds the converted value.
- The level requirement is no longer a data value. `poe-crafting-rules` computes it from the
  base item's own requirement and the levels of all mods of the item, implicit and explicit.
  The value `required_level` of the old data (2 for the Iron Ring) was the drop level. The
  implicit mods had no level in the data, and a rolled mod did not keep its level.

The analysis of the export 3.29.3.3 for the Iron Ring: `mods_by_base.json` has 573 mods for
the tag set `ring,default`, but only its categories `prefix` (81) and `suffix` (126) are the
pool that the Orbs of Transmutation and Alteration roll from (207 mods, as in ADR 0015). The
other categories are influence, delve, corruption and scourge mods. The 207 names are unique.
Every stat of the 207 mods and of the implicit has exactly one English translation entry
whose condition holds for its whole range; only the two handlers above occur.

## Decision
- The converter reads `stat_value_handlers.json` as a sixth raw file. `data/README.md` lists
  its SHA-256 like those of the other five.
- The pool of a base is the union of the categories `prefix` and `suffix` of the
  `mods_by_base.json` entry whose `bases` contains the base. All other categories are left
  out until a story needs them.
- A mod definition (`ModDefinition`) has a `level` and `conversions`, for implicit and
  explicit mods alike. `level` moves from `ExplicitModDefinition` into `ModDefinition`.
  `conversions` maps a stat identifier to its display conversion: `divisor` (a whole number)
  and `precision` (number of decimals, or none). The converter writes the parameters of the
  handler, not its name, so `crafting-core` knows no handler names. It fails on a handler
  with a multiplier, an addend or a fixed value, on a divisor that is not a whole number, and
  on any handler it does not know.
- A rolled mod keeps the level and the conversions of its definition. `Mod::display_text()`
  shows a converted value as `poe-crafting-rules` defines it and every other value as rolled.
  With a precision, the value is computed in 32-bit floating point, rounded to the precision
  (half to even) and shown without trailing zeros. Without a precision, the value is shown
  exactly, without trailing zeros (leech: raw 21 divided by 100 is shown as 0.21). A conversion
  without a precision needs a divisor whose quotient has a finite decimal form (only the prime
  factors 2 and 5, at most 9 decimals); the converter and the loading of the data reject any
  other.
- The base item's `required_level` is the requirement of the base itself: RePoE's
  `requirements.level`, 0 if the base has none (rings). The item computes its displayed
  requirement in `crafting-core` by the rule and reports none when it is 1 or lower; `web`
  only shows it.
- The Global flag is dropped: RePoE has no such value (ADR 0015).
- The converter is a library with a binary (`data-import`). The library function converts the
  contents of the raw files to the contents of the data files and returns an error message
  instead of guessing; the binary reads and writes the files. `crafting-core` uses the
  library as a dev-dependency only, so the acceptance tests can check the converter's
  behaviour with small invented raw files. The website does not depend on it.

## Alternatives considered
- Keeping `level` on `ExplicitModDefinition` and storing the implicits' levels and the rolled
  mods' levels elsewhere (a list beside the implicits, a level set after rolling): no change
  to the test data of earlier stories, but two places for the same value and rolled mods
  whose level is unknown.
- One table from stat identifier to conversion for all mods, as RePoE's own structure: no
  field per mod, but `display_text` would need the embedded data, and two translation
  entries of one stat could convert differently.
- Storing the handler name and implementing the handlers in `crafting-core`: shorter data,
  but each new handler needs code, and the conversion would not come from the game data.
- Computing the pool from the spawn weights (ADR 0015) or taking all 573 mods: the first
  needs a rule for spawn tags; the second would let the orbs roll influence and corrupted
  mods.

## Consequences
- Earlier acceptance tests that build mod definitions or read the level of an explicit mod
  change with the model; the PO confirms them per story (AB#167).
- Handlers that the next pools need (for example `negate`) are added to the converter and
  to the conversion when a story needs them; until then the converter stops with a message.
- The rule for a conversion without a precision is in `poe-crafting-rules` (the value is shown
  exactly, without trailing zeros). It says nothing about a quotient without a finite decimal
  form, so such a conversion is rejected until a story needs it and the PO defines it.
- `data/mods.json` grows from 6 to 207 mods; the website embeds it as before (ADR 0008) and
  loads nothing at runtime.
