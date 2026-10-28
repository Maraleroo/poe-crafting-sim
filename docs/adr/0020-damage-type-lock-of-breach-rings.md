# 0020: The damage type lock of the Breach rings

- Status: accepted
- Date: 2026-10-29
- Work item: AB#188

## Context
AB#188 adds the six Breach rings (`RingB1` to `RingB6` in RePoE: Cryonic, Enthalpic,
Synaptic, Organic, Fugitive and Formless Ring), which ADR 0017 left out. The PO confirmed that
this replaces criterion 2 of AB#185 for these six rings.

Five of them have an implicit with a line "Cannot roll Modifiers of Non-<damage type> Damage
Types". The PO's rule in `poe-crafting-rules` ("Breach rings") says that this line blocks every
mod with a tag of one of the four other damage types, and that the damage types are exactly the
tags `physical`, `lightning`, `cold`, `fire` and `chaos`. The game data does not apply the
lock: RePoE gives all six rings the Iron Ring's pool of 207 mods. In RePoE, the line belongs to
a stat such as `breach_ring_tul_implicit` with the format `ignore`; nothing else in the data
names the damage type.

Three earlier decisions limit the design:
- AB#185 criterion 3 (and its test) requires that every ring without own mods has the Iron
  Ring's pool, in `data/base_items.json` and from `mod_pool`. AB#188 criterion 5 also says that
  the Formless Ring rolls from the same pool as the Iron Ring.
- ADR 0013: the currencies take the valid mods as their pool and do not filter it again; the
  caller builds the pool.
- ADR 0017: the public types `BaseItem`, `ModDefinition` and `ExplicitModDefinition` keep their
  fields, because the protected acceptance tests build them as struct literals.

## Decision
- The converter converts 37 bases: the 31 of ADR 0017 and `RingB1` to `RingB6`, listed in
  `BASES` in `data-import/src/main.rs`.
- The converter recognizes the line of the PO's rule in the translated text of a base's
  implicits: a line "Cannot roll Modifiers of Non-<Name> Damage Types", where `<Name>` is one of
  Physical, Lightning, Cold, Fire or Chaos. It writes the damage type's tag (the name in lower
  case) as `"damage_type_lock"` on the base's entry in `data/base_items.json`. A base without
  the line has no such field, so the entries of the other bases stay byte for byte the same.
  The converter stops if a line starts with "Cannot roll Modifiers of Non-" but does not name
  one of the five damage types, or if a base has more than one such line.
- `mod_pool` stays the pool from RePoE. The lock is applied when the valid mods are built.
- `crafting-core` gets `unblocked_mods(base_id: &str, mods: &[ExplicitModDefinition]) ->
  Vec<ExplicitModDefinition>`: the mods of `mods` that the lock of the base does not block, in
  their order. A mod is blocked if one of its `tags` is one of the four damage types other than
  the lock. A base without a lock, or an identifier that is not in the data, blocks nothing. It
  reads `damage_type_lock` with a private deserialization type, like `mod_pool`, and holds the
  five damage type tags of the PO's rule as a constant. It is the only place of the rule.
  Loading the data panics if a lock is not one of the five tags.
- The currencies do not change. The page builds its pool as
  `valid_mods(&unblocked_mods(base_id, &mod_pool(base_id)), item_level)`.

## Alternatives considered
- Removing the blocked mods from `mod_pool` in the converter: no change in `crafting-core` and
  the page, but the five locked rings would no longer have the Iron Ring's pool, which breaks
  the test of AB#185 criterion 3, and the pool in the data would no longer be RePoE's.
- Filtering inside `apply_orb_of_transmutation` and `apply_orb_of_alteration`: no caller could
  forget the lock, but every currency would repeat the rule, and ADR 0013 keeps all validity
  rules out of the currencies.
- A field for the lock on `BaseItem` or `ModDefinition`: the lock travels with the base, but
  every struct literal of these types in the protected tests would have to change.
- A table of the five lock stats (`breach_ring_tul_implicit` and so on) in the converter or in
  `crafting-core`: shorter, but it puts stat identifiers into code and does not follow the
  PO's rule, which is about the line.
- `crafting-core` reading the implicit text: `crafting-core` never interprets game texts.

## Consequences
- The rule has its own unit tests with invented mods, independent of any currency.
- Callers that build their own pool are responsible for applying `unblocked_mods`, as for
  `valid_mods` (ADR 0013). A pool taken directly from `mod_pool` can still roll blocked mods.
- The converter depends on the English wording of the line in `stat_translations.json`. If a
  new RePoE export words it differently, the converter stops instead of losing the lock
  silently.
- The lock belongs to the base. If a currency can change or remove implicits (for example
  corruption), the lock has to move to the item; that story decides it.
