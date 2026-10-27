# 0018: Item slots and implicit texts with value ranges

- Status: accepted
- Date: 2026-10-27
- Work item: AB#186

## Context
AB#186 lets the player choose the base on the workbench: a search field with suggestions
(item classes first, then bases with their class) and the equipment slots arranged as in the
game's inventory (Weapon, Helmet, Off-hand, Body Armour, Amulet, Ring, Belt, Gloves, Boots).
A slot without data shows "Not available yet"; in this story only Ring has data. The PO decided
on AB#186 that the slot names are UI texts, while the class suggestion shows the class name
from the game data ("Rings") and opens the Ring slot. So the page has to know which item
classes belong to which slot, without writing a game text such as "Rings" into code
(ADR 0008, ADR 0011).

Bases with the same name (three Two-Stone Rings, three Shadowed Rings) are told apart by their
implicit text. The PO decided that suggestions and the list show every base's implicit text
with its value ranges, the way the game shows ranges (Ctrl+Alt+C), for example
"+(12–16)% to Fire and Lightning Resistances". `Mod::display_text` only renders rolled values,
and the item of a base does not exist yet when the list is shown.

## Decision
- Slots are a model of `crafting-core`, like currencies (ADR 0011): an enum `Slot` with one
  variant per slot and `Slot::ALL` in the order of the list above. `Slot::id()` is a project
  identifier in lower snake case (`weapon`, `helmet`, `off_hand`, `body_armour`, `amulet`,
  `ring`, `belt`, `gloves`, `boots`). `crafting-core` knows no slot names; the page takes them
  from `web/i18n/` and decides the layout.
- Which item classes belong to a slot is data: a new hand-written file `data/item_slots.json`,
  embedded and validated like the other data files (ADR 0008):
  `{ "slots": [ { "id": "ring", "item_classes": ["Rings"] } ] }`. A slot without an entry has
  no item classes. `Slot::item_classes()` returns the entry's classes, `Slot::base_items()`
  the bases of `base_items()` whose `item_class` is one of them, in the order of
  `data/base_items.json`. Loading the data panics on an unknown or repeated slot id, on an item
  class that is listed for two slots or that no base has, and on a base whose item class
  belongs to no slot. `data/README.md` documents the file.
- A slot is available when `base_items()` of the slot is not empty. The page needs no list of
  available slots of its own.
- `ModDefinition::range_text()` returns the display text of a mod definition with every
  placeholder replaced by the stat's range, as `poe-crafting-rules` defines it. Each end of a
  range is converted with the stat's `Conversion`, as for rolled values (ADR 0016, ADR 0017).
  A text without placeholders stays as it is; a text with several lines keeps its line breaks.
- The search, the suggestions, the path and the open or closed state of the picker are plain
  Rust in `web` that is tested without a browser (ADR 0010). Matching and ordering are UI
  behaviour, not crafting logic, so they stay out of `crafting-core`.
- The page still starts with the Iron Ring. Choosing a base replaces the workbench's base, its
  pool (`valid_mods` of `mod_pool` of the base at the page's item level) and its item, which
  becomes a new normal item of the base, like Reset.

## Alternatives considered
- Slot assignment by the base tag `ring` in code: no data file, but the slot of an item follows
  its item class in the game, and tags such as `weapon` do not separate main hand and off-hand.
- Slot assignment by the item class name in `web` code: simplest, but puts the game text
  "Rings" into code, against ADR 0011.
- An item class identifier (RePoE's `Ring`) in `data/base_items.json`: a stable reference, but
  needs a converter run with the raw files, which are not in the repository, and changes the
  generated data that earlier acceptance tests check.
- Range texts built in `web`: no change to `crafting-core`, but the formatting of game values
  and their conversion belong to `crafting-core` (ADR 0016).
- Search in `crafting-core`, so that acceptance tests can call it: it would move UI behaviour
  into the crafting logic.

## Consequences
- A new item class (amulets, for example) needs an entry in `data/item_slots.json` in the story
  that imports it; otherwise loading the data panics.
- The search and the slot layout are not covered by `cargo test` in `crafting-core`; they are
  checked by unit tests in `web` and by the PO in the local preview (ADR 0004).
- Ranges of negated or divided stats do not occur in the ring implicits. How the game orders
  the two ends of a negated range is left to the story that first needs it.
