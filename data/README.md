# Game data

Game data files, embedded into `crafting-core` at build time with `include_str!`
(see `docs/adr/0008-game-data-files.md`). Nothing is read from disk or the network at runtime.
Values are stored as the PO supplies them, including values no function uses yet.

## base_items.json

Source: poedb.tw, supplied by the PO with work item AB#52.

```
{ "base_items": [ BaseItem, ... ] }
```

BaseItem:
- `id`: item identifier, e.g. `Metadata/Items/Rings/Ring1`
- `name`: item name (English original)
- `item_class`: item class (English original)
- `required_level`: level requirement, a data value, not computed from rules
- `tags`: tags of the base
- `implicits`: list of mods the base item always has

Mod (an implicit):
- `family`: mod family
- `tags`: mod tags
- `stats`: one or more stats `{ "id", "min", "max", "global" }`. `id` is the stat identifier as
  shown by poedb; `min`/`max` is the range the stat rolls in; `global` is the "Global" flag.
- `text`: display text with named placeholders, e.g. `Adds {minimum} to {maximum} ...`
- `placeholders`: maps each placeholder name in `text` to the identifier of a stat of the same
  mod. The text is rendered through the identifier, never through the order of the stats.

## mods.json

Source: poedb.tw, supplied by the PO with work item AB#62 (one tier per mod; the weight is the
spawn weight for rings). Until the spawn tags are imported, the whole file is the mod pool of
the Iron Ring.

```
{ "mods": [ ExplicitMod, ... ] }
```

ExplicitMod has the fields of a Mod (see above) and:
- `name`: name of the mod, used in the item name (`Hale` before the base name,
  `of the Pupil` after it)
- `affix_type`: `prefix` or `suffix`
- `level`: level of the mod, a data value
- `weight`: spawn weight, greater than zero

The stats of a mod are listed in the order poedb shows them. The order has no meaning; the text
is rendered through the placeholders. A text with several lines separates them with `\n`.

## currencies.json

Source: names from the story texts of AB#62 and AB#114 (ADR 0011); descriptions and failure
messages from the story texts of AB#113 and AB#114 (ADR 0012).

```
{ "currencies": [ { "id": "...", "name": "...", "description": "...", "messages": { ... } }, ... ] }
```

- `id`: project identifier in lower snake case, not a game value; unique
- `name`: English name of the currency (game text, shown unchanged)
- `description`: the currency's English game description (game text, shown unchanged)
- `messages`: maps a reason code to the game's failure message for this currency (game text,
  shown unchanged). The keys are the codes of `CraftError` (`item_not_normal`,
  `item_not_magic`, `no_valid_mod`); a currency has a message for every reason it can fail
  with, and for no other.
