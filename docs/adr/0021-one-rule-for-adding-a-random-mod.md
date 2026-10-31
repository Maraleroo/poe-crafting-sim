# 0021: One rule for adding a random mod, built from the item

- Status: accepted
- Date: 2026-10-31
- Work item: AB#248

## Context
AB#248 replaces rules 1 to 4 of the Orb of Transmutation and the Orb of Alteration with two
general rules in `poe-crafting-rules`: "Affix limits" in "Item basics" and "Adding a random
mod". To build the valid pool, the rule needs the base's pool, the item level, the blocks
(Breach rings), the room left under both affix limits and the families already on the item.
Implicit mods do not count toward the limits or for the family rule. Criterion 4 says that
crafting-core has exactly one implementation of the rule, that both orbs use it, and that the
page does not build the pool itself. Criterion 5 says that an item can hold as many prefixes
and suffixes as the limits allow.

Until now, the caller built the pool: `valid_mods` (ADR 0013) and `unblocked_mods`
(ADR 0020) in `web/src/workbench.rs`, and the orbs took "the valid mods" and did not filter
them again. The item level was state of the workbench, not of the item (ADR 0019). `Item` had
the fields `prefix` and `suffix`, so it could hold only one of each.

The PO decided on AB#248:
- The tests that check the old orb rules and the empty-pool failure are replaced. The tests
  that use `item.prefix` and `item.suffix` are adapted to the new model. The name checks in
  `web/src/` for `valid_mods` and `unblocked_mods` are replaced by criterion 4.
- The limits come from the rarity only. Reading the line "+N Prefix Modifiers allowed" or
  "-N Suffix Modifiers allowed" from the data comes with AB#189.
- These orbs no longer fail with `NoValidMod`. Their `no_valid_mod` messages are removed, and
  the Exalted Orb story brings the message back.

The protected tests still call `apply_orb_of_transmutation(item, pool, rng)`,
`apply_orb_of_alteration(item, pool, rng)`, `Item::new(base, rng)`, `valid_mods` and
`unblocked_mods`, also with invented pools and pools that are already filtered.

Blocks by tag occur more than once in the game: the Breach rings today, and later crafted mods
such as "Cannot roll Attack Modifiers" and "Cannot roll Caster Modifiers".

## Decision
- `Item` holds `prefixes: Vec<ExplicitMod>` and `suffixes: Vec<ExplicitMod>` in the order they
  were added, and `item_level: u32`. `Item::new(base, rng)` gives a normal item with item
  level `DEFAULT_ITEM_LEVEL`, which is 100, the default of the rule "Item level". The page's
  item level field uses this constant as its default. The workbench sets the chosen level on
  the item. This replaces the decision of ADR 0019 that the item level is state of the
  workbench. The page's rules from ADR 0019 (range 1 to 100, reading the field, reset and
  choice of a base) stay.
- `AffixLimits { mods, prefixes, suffixes }` with `AffixLimits::of(rarity)`: normal 0/0/0,
  magic 2/1/1, rare 6/3/3, `None` for unique items, which have no rule. It is the only place
  of the limits. AB#189 adds the changes from the item's lines in the same place.
- A block is a set of blocked tags that the item collects from its sources. A mod is blocked
  if one of its tags is in the set. Today the only source is the base's damage type lock,
  which blocks the four other damage types (ADR 0020). A later story adds a crafted mod as a
  source of tags, through its rule in `poe-crafting-rules`. `unblocked_mods` stays public and
  uses the same set. The lock data is read once, not on every call, because the pool is now
  built for every added mod.
- `valid_pool(item, base_pool)` is the only implementation of step 1 of the rule. It keeps
  the mods of `base_pool` in their order that pass four filters, applied in this order: item
  level (through `valid_mods`), blocks, room on the mod's side under both limits (counting
  only prefixes and suffixes), and no prefix or suffix of the same family on the item.
  `add_random_mod(item, base_pool, rng) -> bool` builds it, chooses one mod by weight, rolls
  it and adds it to its side. It returns false and changes nothing if the pool is empty.
- The orbs keep their signatures. Their `pool` is now the base's pool, not the valid mods.
  They follow the PO's rule in both currency files: choose a target number of 1 or 2 mods,
  each with a 50 % chance. The Orb of Transmutation first makes the item magic, and the Orb of
  Alteration first removes all prefixes and suffixes. Then they call `add_random_mod` until
  the item has that many prefixes and suffixes or no mod is added. Their only errors are
  `ItemNotNormal` and `ItemNotMagic`. `CraftError::NoValidMod` and the two `no_valid_mod`
  entries in `data/currencies.json` are removed.
- The page passes the base's pool from `mod_pool` and calls neither `valid_mods` nor
  `unblocked_mods`.

## Alternatives considered
- The orbs read the pool themselves with `mod_pool(&item.base.id)` and lose the parameter.
  This matches the game better, but it changes every call of the orbs in the protected tests,
  and invented pools or bases would no longer be possible.
- The item level as a parameter of the orbs, with no field on `Item`: this also changes every
  call, and the item level, which belongs to the item in the game, would stay outside it.
- Filtering by item level and blocks on the page, and by room and family in the orbs: the
  rule would be split in two places, and an orb could roll a mod above the item level. This
  contradicts criterion 4.
- One test per kind of block (closures or a trait per source) instead of a set of tags: this
  is more general, but neither the Breach rule nor the planned crafted blocks need more than
  tags. If a later rule is not about tags, that story revisits this decision.
- Keeping `NoValidMod` for the Exalted Orb: no currency could return it, and the data would
  give these orbs a message that criterion 3 says they never show.
- Keeping the names `valid_mods` and `unblocked_mods` in `web/src/`, in comments or unit
  tests, so that the old name checks pass: the checks would pass although the page no longer
  builds the pool.

## Consequences
- Each step of the rule has one place: `valid_mods` for the item level, the blocked tags for
  the Breach lock, `AffixLimits` for the limits, and `valid_pool` for the whole step 1. Each
  new currency that adds mods calls `add_random_mod`.
- The consequence of ADR 0013 and ADR 0020 that callers build the pool no longer holds. A
  pool that is already filtered gives the same result, because the orbs filter it again.
- The random sequence of the orbs changes. Equal seeds still give equal results, but no
  longer the same results as before.
- On a magic item with at most 1 prefix and 1 suffix, the chances are the same as before. No
  family in the ring data has both prefixes and suffixes, so the family filter does not change
  them for the 37 rings (criterion 6).
- `Item::name()` follows the rule for magic items. Names of rare items are not defined yet.
- If a currency can change implicits later (for example corruption), the lock and its blocked
  tags have to come from the item's mods instead of the base. The set of blocked tags is the
  place for that change.
