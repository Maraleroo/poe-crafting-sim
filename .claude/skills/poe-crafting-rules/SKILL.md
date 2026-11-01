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

Affix limits. A magic item has at most 2 mods and at most 1 prefix and 1 suffix. A rare item has at most 6 mods and at most 3 prefixes and 3 suffixes. A mod line "+N Prefix Modifiers allowed" or "-N Suffix Modifiers allowed" (also singular) changes the limit of that side by N; a side's limit is never below 0. The total limit does not change. Both limits hold at all times. Implicit mods count neither toward these limits nor for the family rule in Adding a random mod. Evidence: 200 Orbs of Alteration on a Composite Ring ("+3 Prefix Modifiers allowed", "-3 Suffix Modifiers allowed"), measured by the PO on 2026-10-30: never a suffix, 1 prefix 98 times, 2 prefixes 102 times, never more than 2 mods. Evidence for implicits: a magic Sapphire Ring, copied by the PO on 2026-10-31, has the implicit "+26(20-30)% to Cold Resistance" and the suffix "of the Polar Bear" "+37(36-41)% to Cold Resistance". A magic item's name is the names of its prefixes, the base name and the names of its suffixes, each side in the order its mods were added (a fractured mod first), separated by spaces; a side without mods leaves its part out. Evidence: "Chilled Fleet Composite Ring", "Sapphire Polished Composite Ring" (Sapphire before an Orb of Augmentation, Polished added by it), "Burning Stout Composite Ring", "Cogwork Ring of the Philosopher of Haast" (fractured suffix first).

The level requirement of an item is the higher of two values: the level requirement of the base item itself, and 80 % of the highest level of all its mods (implicit and explicit), rounded down. A base item without its own level requirement (for example rings) counts as 0; its implicit mods still count as mods. The drop level of a base is not a requirement. An item whose level requirement is 1 or lower shows no level requirement.
- Evidence: 3000 magic Bone Rings of item level 83 measured by the PO (poe_orb_test): the rule gives the shown requirement for all 2656 items whose mods could be identified, with no contradiction. A normal Bone Ring (implicit level 32) shows "Level: 25" in the game; poedb.tw shows the drop level 30 instead. Two rare Blasting Blades (base requirement 50): with a highest mod level of 74 the game shows "Level: 59", with a highest mod level of 35 it shows "Level: 50". A normal Iron Ring (implicit level 2) shows no requirement; an Iron Ring of the Gorilla (mod level 44) shows "Level: 35".

Where the game converts a stat value for display, it computes the converted value as a 32-bit floating-point number and rounds it to the nearest value at the precision given in the game data; a value exactly halfway rounds to the even digit. Trailing zeros are dropped (1.0 is shown as 1). Because of the 32-bit computation, most halves are not exact: 123/60 = 2.05 is shown as 2 and 129/60 = 2.15 as 2.2, while 135/60 = 2.25 is exact and shown as 2.2.
- Evidence: 3500 Blessed Orbs on a Coral Amulet (implicit raw 120-240 life per minute, shown per second), measured by the PO on 2026-10-25 in three runs of 1000, 500 and 2000 (poe_orb_test, blessed_coral*.jsonl). Rounding without the 32-bit computation does not fit (last run alone: p = 0.00001; all runs: p < 0.000000001), nor do rounding down or up. The 32-bit computation fits (all runs: p = 0.98). That exact halves (2.25, 3.25) round to the even digit rather than up is the better fit over all runs (about 750 times more likely), but it rests on only two raw values, and the first run pointed slightly the other way. The shown ranges equal RePoE's raw ranges divided by 60.

Where the game data gives no precision for a conversion, the converted value is shown exactly, without trailing zeros (for example a raw leech value of 21 divided by 100 is shown as 0.21).
- Evidence: all 100 leech lines in the PO's Bone Ring item texts (poe_orb_test) show values with at most two decimals that equal RePoE's raw values divided by 100, with trailing zeros dropped (0.2, 0.21 … 0.4).

Where a mod line "X% increased Prefix Modifier magnitudes", "X% increased Suffix Modifier magnitudes" or "X% increased Explicit Modifier magnitudes" applies, each affected stat value of the game data is multiplied by (1 + X/100) and rounded down to a whole number; the result is then converted for display as usual. The normal view shows this value. The extended text (Alt view, Ctrl+Alt+C) shows the unscaled value and range and marks the mod with "— X% Increased". Evidence: Cogwork Ring ("25% increased Suffix Modifier magnitudes"), values read by the PO on 2026-10-31 in the normal and the Alt view. Whole numbers: 11 values, including 6 → 7 (7.5), 10 → 12 (12.5), 22 → 27 (27.5), 47 → 58 (58.75). Leech (raw value / 100): 0.30 → 0.37, 0.31 → 0.38, 0.34 → 0.42, 0.35 → 0.43, 0.38 → 0.47, 0.39 → 0.48. Two life regeneration values (29.8 → 37.2, 25.2 → 31.5) fit, but their raw values are not unique. Measured only at +25 %; +50 % and +100 % are assumed to follow the same rule.

In an extended item text (Ctrl+Alt+C), a value with a range (minimum less than maximum) is followed directly by `(minimum-maximum)`: in round brackets, with a normal hyphen, without spaces. If a mod has several values, this applies to each value separately. A fixed value (minimum equal to maximum) is shown as the value only, without brackets.
- Evidence: extended copies (Ctrl+Alt+C) from Path of Exile 1, provided by the PO on AB#186. Two-Stone Ring: `+15(12-16)% to Fire and Lightning Resistances`. Steel Ring: `Adds 3(3-4) to 10(10-14) Physical Damage to Attacks`. Iron Ring: `Adds 1 to 4 Physical Damage to Attacks`.
- PO decision for AB#186: suggestions and lists of bases have no rolled value yet. There the value before the brackets is left out and the range stays: `+(12-16)% to Fire and Lightning Resistances`, `Adds (3-4) to (10-14) Physical Damage to Attacks`. Fixed values stay as above: `Adds 1 to 4 Physical Damage to Attacks`.
- Not evidenced and not to be guessed: ranges of negated values (reduced). They do not occur among the ring implicits; all negated values there are fixed.
- Values with decimals follow the same format: the value and both bounds of the range are converted and rounded as described above, without trailing zeros. Evidence: the PO's recordings (poe_orb_test): "Regenerate 3.5(2-4) Life per second", "Regenerate 10.1(8.1-16) Life per second", "0.38(0.2-0.4)% of Physical Attack Damage Leeched as Life".

In the normal view, the explicit mod lines of an item are sorted by the order of their stats in the game's stat descriptions (RePoE stat_translations). Lines of different mods with the same stat are shown as one line with their values added up; a fractured mod is not merged and keeps its own line. Implicit mods are shown above the explicit mods and are never merged with them. Evidence: normal view and Alt view of seven items read by the PO on 2026-10-31 (Sapphire Polished Composite Ring, Burning Stout Composite Ring, Cogwork Ring, rare Steel Ring, rare Sapphire Ring, rare Wyrmscale Boots, Sanguine Sapphire Ring of Haast): all 23 explicit lines follow the stat order. Wyrmscale Boots: "+92 to maximum Life" (Rotund) and "+18 to maximum Life" (Flea's) are shown as "+110 to maximum Life". Sanguine Sapphire Ring of Haast: the implicit "+26% to Cold Resistance" and the suffix "+48% to Cold Resistance" are shown as two lines. Fractured mods: the PO's knowledge of the game. The lines of fractured mods come first, sorted by stat order among themselves; then the other explicit lines follow, sorted and merged as above. Evidence: rare Wyvernscale Boots "Dusk Spark", screenshots of the PO on 2026-11-01 with a fractured suffix and with a fractured prefix.

## Advanced mod descriptions
The advanced view (Alt in the game) shows each mod as its own block; nothing is merged. Implicit mods come first, then prefixes, then suffixes. Within each side, a fractured mod comes first; the other mods follow in the order they were added. A block shows the mod's lines with each value and its range as in the extended item text, its tags, and for a prefix or suffix its name and tier.

A mod's tier is 1 plus the number of mods in the base's pool on the same side and of the same family with a higher level. The item level does not matter.

The header of a mod shows its tags from the game data in their order, each word capitalised and with spaces instead of underscores, separated by commas. The tags elemental_damage, physical_damage, chaos_damage, flat_life_regen and resource are never shown. An implicit mod without tags shows no tags.
- Evidence: the PO's Ctrl+Alt+C recordings (poe_orb_test, Bone Ring and Composite Ring): tiers of all 297 different mods and tags of all 11,425 mod headers fit. Copies by the PO on 2026-11-01: Sapphire Ring of the Flatworm ("of the Flatworm" Tier 5, tags "Life"; data: flat_life_regen, resource, life), rare Unset Ring (Tiers 3, 8, 2, 4; "Gem", "Attribute", "Damage, Elemental, Lightning", "Drop"), Iolite Ring (implicit tags "Damage, Chaos"; data: chaos_damage, damage, chaos). Order and fractured mods: rare Wyvernscale Boots "Dusk Spark", screenshots of the PO on 2026-11-01 before and after a Fracturing Orb (a fractured suffix and a fractured prefix).

## Item level
A mod can roll on an item only if the item level is at least the mod's level. Example: a mod with level 60 needs an item level of 60 or higher. Default in the UI: 100.
- Evidence: the PO's knowledge of the game. Consistent with the PO's measurement on 2026-10-21 (3000 Orbs of Transmutation and Alteration on a Bone Ring of item level 83): mods with level 82 rolled 63 times, mods with level 84 or 85 never.

## Mod pools and weights
The mods that can roll on a base and their weights come from the game data (RePoE, ADR 0015), already resolved per base.

## Adding a random mod
Adding a random mod. 1. Build the valid pool: every mod of the base's pool whose level is at most the item level, that is not blocked (Breach rings), whose side still has room under both affix limits, and whose family is not already on the item. 2. If the pool is empty, no mod is added. 3. Otherwise choose one mod from the pool; its chance is its weight divided by the sum of the weights in the pool. Every currency that adds a random mod uses this rule; what happens when no mod is added is part of each currency's rule. Evidence: the PO's knowledge of the game (an essence mod and the normal mod of the same family never appear together); on the Composite Ring, 31 different prefix pairs and never two of the same family.

## Breach rings
PO rule: There are exactly five damage types: Physical, Lightning, Cold, Fire, Chaos. The line "Cannot roll Modifiers of Non-<damage type> Damage Types" blocks every mod that has at least one tag of one of the four other damage types; a blocked mod can never roll on that ring. The mod's tags in the game data decide; there, the damage types are exactly the tags `physical`, `lightning`, `cold`, `fire` and `chaos`. Other tags such as `elemental`, `physical_damage` or `elemental_damage` are not damage types.
- Source: AB#188, criterion 3.

## Currencies
One file per currency, linked here, e.g. `currencies/orb-of-transmutation.md`:
- Applies to: <which items>
- Effect: <exact behaviour>
- Not applicable when: <condition>, reason shown to the user: "<text>"

Currency rules:
- [Orb of Transmutation](currencies/orb-of-transmutation.md)
- [Orb of Alteration](currencies/orb-of-alteration.md)
- [Orb of Augmentation](currencies/orb-of-augmentation.md)
- [Regal Orb](currencies/regal-orb.md)
- [Exalted Orb](currencies/exalted-orb.md)

## Sources
- https://www.poewiki.net/
- https://poedb.tw/
