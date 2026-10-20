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
