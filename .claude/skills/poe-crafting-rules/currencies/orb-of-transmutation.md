**Orb of Transmutation**
- Applies to: normal items.
- Effect: the item becomes magic and gets new mods.
  1. The item gets 1 or 2 mods, each with a 50% chance.
  2. One mod: it is chosen from one pool of all valid prefixes and all valid suffixes together. The chance of a mod is its weight divided by the sum of the weights of all mods in this pool. There is no separate choice between prefix and suffix.
  3. Two mods: one prefix is chosen from the valid prefixes and one suffix from the valid suffixes. Within each pool, the chance of a mod is its weight divided by the sum of the weights in that pool. If one type has no valid mod, that mod is left out; no second mod of the other type is chosen instead.
  4. If there is no valid mod at all, applying the orb fails with an error message and the item stays unchanged.
- Evidence: measured by the PO on 2026-10-21 with 1000 Orbs of Transmutation on a Bone Ring (item level 83): 1 mod 49.5 %, 2 mods 50.5 %; with one mod, 68.5 % suffixes against 69.4 % expected from the weights. A 50:50 choice between prefix and suffix is ruled out (p ≈ 10⁻¹⁷). Not documented by GGG. The empty-pool case in rule 3 is the PO's assumption and not measured.
