# 0019: The chosen item level is state of the workbench

- Status: accepted
- Date: 2026-10-28
- Work item: AB#187

## Context
AB#187 adds the field "Item level" (1 to 100, default 100). After a change, the item becomes a
new normal item of the chosen base, and the Orbs of Transmutation and Alteration roll only mods
whose level is at most the chosen item level (`poe-crafting-rules`, "Item level"). The PO
decided that invalid input is not applied, that choosing another base sets the item level back
to 100 and that "Reset" keeps it.

ADR 0013 made `valid_mods(mods, item_level)` the only implementation of the rule. The page held
the item level as a constant, and ADR 0013 planned to move it to `Item` in its own decision once
the user can set it. `Item::new(base, rng)` is used by the protected acceptance tests, and the
currencies take the valid mods as their pool and do not filter it again.

## Decision
- The chosen item level is a field of `Workbench` in `web`, not of `Item`. `crafting-core` does
  not change.
- The workbench builds its pool as `valid_mods(&mod_pool(base), item_level)` whenever the base
  or the item level changes. A new item level replaces the item with a new normal item of the
  base, like "Reset". Choosing a base sets the item level to the default of 100; "Reset" keeps
  the item level and the pool.
- The allowed values 1 to 100 and the reading of the field's text are rules of the page (PO
  decision on AB#187), not game rules. They live in `web` as a pure function that compiles
  natively and has unit tests (ADR 0010). Text that is not a whole number from 1 to 100 is not
  applied; the field shows the last valid value again.
- The item level moves to `Item` when items come from the game (pasted item texts), because
  then the item itself carries it. That story decides it. This replaces the plan in ADR 0013 to
  move it as soon as the user can set it.

## Alternatives considered
- A field `item_level` on `Item`: matches the game, but `Item::new(base, rng)` cannot take a
  level without changing protected tests, so `crafting-core` would have to own the UI default
  of 100. The currencies would either filter by it again, duplicating `valid_mods`, or ignore
  it, so the field would have no effect in `crafting-core`.
- The range 1 to 100 as constants in `crafting-core`: the range is the PO's decision for the
  field, not a rule in `poe-crafting-rules`, and nothing in `crafting-core` uses it.

## Consequences
- The story adds no interface to `crafting-core`. The rule itself stays covered by the
  acceptance tests of AB#114 and AB#186; how the page uses the chosen level is covered by unit
  tests in `web` and checked in the local preview.
- The item does not know its item level. Showing it on the item, or using it in a later
  currency, goes through the workbench until the item level moves to `Item`.
