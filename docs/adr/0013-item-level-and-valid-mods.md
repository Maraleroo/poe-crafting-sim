# 0013: Item level decides the valid mods before a currency is applied

- Status: accepted
- Date: 2026-10-24
- Work item: AB#114

## Context
AB#114 adds the Orb of Alteration. With it, the PO wrote the rule for item level in
`poe-crafting-rules`: a mod can roll on an item only if the item level is at least the mod's
level; the default in the UI is 100. The rule applies to every currency that rolls mods,
including the existing Orb of Transmutation.

The currencies already take "the valid mods" as a pool parameter
(`apply_orb_of_transmutation(item, pool, rng)`); the acceptance tests of AB#62 and AB#113
call them with that signature. Until now, the page passed all mods of `data/mods.json`.
The highest mod level in the data is 16, so with item level 100 no mod was wrongly allowed,
but the rule was not written down in the code.

`Item` has no item level, and the page cannot change it yet.

## Decision
- `crafting-core` gets `valid_mods(mods: &[ExplicitModDefinition], item_level: u32) ->
  Vec<ExplicitModDefinition>`: the mods whose `level` is at most `item_level`, in their
  order. It is the only place where the item-level rule is implemented.
- Currencies keep their `pool` parameter and treat it as the valid mods. They do not filter
  it again. An empty pool is the reason code `NoValidMod`, as before.
- The item level is not a field of `Item` yet. The page holds it: `web` has one constant for
  the UI default of 100 and builds the pool with `valid_mods(&explicit_mods(), <constant>)`.
- When a story lets the user set the item level, or when items come from the game, the item
  level moves to `Item` in its own decision.

## Alternatives considered
- A field `item_level` on `Item`, set by `Item::new`: the item level belongs to the item in
  the game. But `Item::new(base, rng)` is used by the acceptance tests and cannot take a
  level, so `crafting-core` would have to own the UI default of 100. Postponed until the
  level can be set.
- Filtering inside every `apply_*` function: each currency would repeat the rule and would
  need the item level, which the item does not have.
- No change, because no mod in the data is above level 100: the rule would only hold by
  accident of the data and could not be tested.

## Consequences
- The item-level rule has its own unit tests, independent of any currency.
- The Orb of Transmutation and the Orb of Alteration get the same pool, so both follow the
  rule with no change to their signatures.
- Callers that pass their own pool, such as tests, are responsible for passing valid mods.
