---
name: poe-crafting-rules
description: Path of Exile 1 crafting mechanics as specified by the PO, covering rarities, item level, mod pools, weights and currency behaviour. Use whenever code, tests or reviews touch game mechanics.
---

# Path of Exile 1 crafting rules

These rules are written by the PO and are the only source of truth for game mechanics
in this project. If a rule you need is missing or ambiguous, stop and report it.
Never fill gaps from memory.

## Item basics
Rarities: Normal, Magic, Rare, Unique. A normal item has no prefixes or suffixes. It only has the implicit mods of its base, which can be none (for example a Cobalt Jewel). A mod has one or more stats; each stat has a range (min–max) and rolls a value within that range when the mod is created. A mod's display text refers to its stats by identifier, never by their order.

A magic item has at most one prefix and one suffix. Its name is "<prefix name> <base name> <suffix name>"; a missing prefix or suffix leaves its part out.

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

Currency rules:
- [Orb of Transmutation](currencies/orb-of-transmutation.md)

## Sources
- https://www.poewiki.net/
- https://poedb.tw/
