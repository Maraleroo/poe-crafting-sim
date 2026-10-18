---
name: poe-crafting-rules
description: Path of Exile 1 crafting mechanics as specified by the PO, covering rarities, item level, mod pools, weights and currency behaviour. Use whenever code, tests or reviews touch game mechanics.
---

# Path of Exile 1 crafting rules

These rules are written by the PO and are the only source of truth for game mechanics
in this project. If a rule you need is missing or ambiguous, stop and report it.
Never fill gaps from memory.

## Item basics
TODO (PO): rarities and how many prefixes and suffixes each rarity allows.

## Item level
TODO (PO): how item level restricts which mods can roll. Default in the UI: 100.

## Mod pools and weights
TODO (PO): which mods belong to a base, how weights turn into probabilities,
which mod groups exclude each other. Data source: poedb.tw, imported into `data/`.

## Currencies
One file per currency, linked here, e.g. `currencies/orb-of-transmutation.md`:
- Applies to: <which items>
- Effect: <exact behaviour>
- Not applicable when: <condition>, reason shown to the user: "<text>"

## Sources
- https://www.poewiki.net/
- https://poedb.tw/
