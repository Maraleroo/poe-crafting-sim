# 0015: RePoE as the source of the rings' game data

- Status: proposed
- Date: 2026-10-25
- Work item: AB#162

## Context
Until now the PO typed game data from poedb.tw into `data/` by hand (ADR 0008): the Iron Ring
and six of its mods. The next stories need complete mod pools, first for rings, with the Iron
Ring and the Bone Ring as test cases. The PO decided that correctness of the data matters
most and that the source only matters because of its licence. RePoE
(https://github.com/repoe-fork/repoe, export at https://repoe-fork.github.io/) is the main
candidate: its code is MIT, its data is subject only to the terms of Grinding Gear Games.
poedb.tw (CC BY-NC-SA 3.0) is used only to cross-check, and nothing from it goes into the
repository. Craft of Exile is not used. The PO downloads the raw files; agents download
nothing.

The spike AB#162 checked the RePoE export for Path of Exile 1 that the PO supplied (version
3.29.3.3 according to https://repoe-fork.github.io/poe1.html):

- Files: `base_items.json` (base items, their tags and implicits), `mods.json` (mods with
  name, generation type, required level, groups, tags, stats with ranges and spawn weights by
  tag), `mods_by_base.json` (the pools per item class and tag set, already resolved to
  weights), `stat_translations.json` (display texts per stat) and `item_classes.json` (the
  item class name, `Rings`). `tags.json` is not needed. The export is the one for Path of
  Exile 1: the identifiers are the ones of the game (`Metadata/Items/Rings/Ring1`), the item
  classes are those of PoE 1, and the pools match poedb.tw/us and the PO's measurement in
  PoE 1.
- Game version: no file contains a version, patch or league. The version is known only from
  the export page, as stated by the PO.
- Mod pools: the prefix and suffix pools of the Iron Ring (35 groups, 207 tiers) and the Bone
  Ring (40 groups, 230 tiers) match poedb.tw in name, affix type, level, weight and value
  ranges for every tier. Recomputing the pools from the spawn weights in `mods.json` gives the
  same result as `mods_by_base.json`. The only deviation is the line order of the three
  two-line mods `of Shining`, `of Light` and `of Radiance`: the `text` field in `mods.json`
  lists Light Radius first, poedb and the game list Global Accuracy Rating first. The order
  of the entries in `stat_translations.json` gives the game's order.
- Weights: with the rules for item level and the Orbs of Transmutation and Alteration, the
  Bone Ring weights from RePoE fit the PO's 3000 measured items (item level 83). No mod above
  level 83 rolled; the tier numbers and value ranges in all item texts match RePoE; the
  frequencies of the groups and tiers show no significant deviation (figures in the
  appendix).
- Further differences to the current `data/` files: RePoE writes stat identifiers with
  underscores (`base_maximum_life`), poedb with spaces; mod tags are lower case; there is no
  "Global" flag per stat; `requirements` of the rings is empty, and the Iron Ring's
  `drop_level` is 2, the value of the current `required_level`. Stat ranges are raw values;
  for some mods the display converts them (`per_minute_to_per_second` for life regeneration,
  `divide_by_one_hundred` for leech). The game adds reminder texts that RePoE keeps as
  `reminder_text` in `stat_translations.json`.

## Decision
- RePoE's export for Path of Exile 1 is the source of the game data for base items and mods.
  poedb.tw stays a manual cross-check; nothing from it is copied into the repository.
- The PO supplies `base_items.json`, `mods.json`, `mods_by_base.json`,
  `stat_translations.json` and `item_classes.json` of one export, together with the version
  shown on the export page and the download date. The raw files stay outside the repository.
- A converter turns the raw files into the files under `data/`. It is a binary crate in the
  workspace (`data-import`), uses `serde_json` like `crafting-core`, is run by hand with the
  directory of the raw files as argument, and is not part of the website. Its output is
  deterministic, so the same raw files give byte-identical files. It converts only the bases
  a story names (first: the rings) and fails with a message on any value it cannot convert,
  instead of guessing.
- Conversion into the format of ADR 0008:
  - `base_items.json`: one entry per base from RePoE's `base_items.json`, with `item_class`
    from `item_classes.json`, implicits from `mods.json`, and a new field `mod_pool`: the
    identifiers of its prefixes and suffixes from `mods_by_base.json`. The project does not
    implement the spawn-tag mechanism; the pool comes resolved from the source.
  - `mods.json`: every mod that is in a pool, with a new field `id` (the RePoE mod
    identifier), `name`, `affix_type` (generation type), `family` (the mod's only group),
    `level` (required level), `weight`, `tags` and `stats` with the raw ranges. The weight of
    a ring mod is the same in every ring pool; if a mod had different weights in two pools,
    the converter fails and a new decision is needed.
  - `text` and `placeholders` come from the English entry of `stat_translations.json` whose
    condition holds for the whole range; the converter fails if the range needs two
    entries. Each positional `{0}` of the source becomes a placeholder named after the stat
    identifier it stands for, so the text refers to its stats by identifier (ADR 0008). A
    `+#` format keeps its `+`. Lines are ordered by the position of their entry in
    `stat_translations.json`; the `text` field of `mods.json` is not used.
  - Identifiers and values are taken unchanged from RePoE (stat identifiers with
    underscores, tags in lower case).
- Provenance: `data/README.md` names, per file, the source (RePoE export for PoE 1, the
  export page URL), the game version from the export page, the download date, the SHA-256
  of each raw file and the command that ran the converter. A data change is a converter run
  with new raw files and an updated provenance section in the same commit.

## Alternatives considered
- poedb.tw as the source: the pools are equally correct, but its licence (CC BY-NC-SA 3.0)
  is not wanted for the repository, and poedb has no export.
- Computing the pools from the spawn weights in `mods.json`: gives the same pools for both
  rings, but it needs a written game rule for spawn tags, which `poe-crafting-rules` does not
  have yet. `mods_by_base.json` keeps that mechanism in the source. It becomes the better
  choice when currencies that change tags (for example fossils) are planned.
- Copying the RePoE files into `data/` and parsing them at build time: no converter, but
  about 75 MB of data the website does not need, and the format of ADR 0008 would change
  everywhere at once.
- Using RePoE's `text` field: simpler, but it has values instead of placeholders and the
  wrong line order for two-line mods.
- A Python script as converter: Python is not part of the toolchain the project checks
  with `cargo fmt`, `cargo clippy` and `cargo test`.

## Consequences
- The import story changes data that existing acceptance tests read: stat identifiers
  (`attack minimum added physical damage` becomes `attack_minimum_added_physical_damage`),
  placeholder names in texts (`{minimum}`), the `global` flag that RePoE does not have, tag
  case, and the fact that `data/mods.json` is the Iron Ring's whole pool. Its plan has to
  name these tests and get the PO's confirmation for each replaced criterion.
- Mods whose display converts the raw value (life regeneration, leech) need a rule for how
  the value is shown and rounded before they can be shown correctly. Reminder texts are not
  imported until a story needs them, for example pasting items from the game.
- The game version of the data is only as reliable as the export page, because the files do
  not carry it.
- The section "Mod pools and weights" of `poe-crafting-rules` still names poedb.tw as the
  data source; if this ADR is accepted, the PO updates it.
- The website does not change through this decision; it keeps embedding the data at build
  time (ADR 0008) and loads nothing from other servers.

## Appendix: Bone Ring weights against the PO's measurement
Data: the PO's 3000 Bone Rings of item level 83 (1000 Orbs of Transmutation, 2000 Orbs of
Alteration) and the RePoE weights. All 6 tiers above level 83 (level 84 and 85) are
excluded; 224 tiers in 40 mod groups remain, and none of the excluded tiers rolled. Expected
counts are 3000 x (0.5 x w / W_all + 0.5 x w / W_type), with w the weight of the tier, W_all
the sum of the weights of all 224 tiers and W_type the sum for its affix type. The table sums
observed and expected counts of the tiers of each group.

| Mod group | Type | Tiers | Observed | Expected |
|---|---|---|---|---|
| IncreasedLife | prefix | 8 | 244 | 260.2 |
| IncreasedMana | prefix | 12 | 388 | 390.2 |
| EnergyShield | prefix | 11 | 353 | 357.7 |
| PhysicalDamage | prefix | 6 | 195 | 195.1 |
| FireDamage | prefix | 9 | 117 | 105.7 |
| ColdDamage | prefix | 9 | 102 | 105.7 |
| LightningDamage | prefix | 9 | 110 | 105.7 |
| EvasionRating | prefix | 6 | 196 | 195.1 |
| IncreasedWeaponElementalDamagePercent | prefix | 5 | 40 | 48.8 |
| ItemFoundRarityIncreasePrefix | prefix | 3 | 93 | 97.6 |
| MinionDamage | prefix | 5 | 126 | 97.6 |
| Strength | suffix | 9 | 150 | 167.4 |
| Dexterity | suffix | 9 | 192 | 167.4 |
| Intelligence | suffix | 9 | 169 | 167.4 |
| AllAttributes | suffix | 4 | 39 | 29.8 |
| LifeLeechPermyriad | suffix | 1 | 13 | 18.6 |
| ManaLeechPermyriad | suffix | 1 | 16 | 18.6 |
| ItemFoundRarityIncrease | suffix | 4 | 75 | 74.4 |
| IncreasedCastSpeed | suffix | 3 | 53 | 44.6 |
| IncreasedAttackSpeed | suffix | 1 | 10 | 9.3 |
| IncreasedAccuracy | suffix | 5 | 82 | 85.6 |
| LifeRegeneration | suffix | 7 | 107 | 130.2 |
| ManaRegeneration | suffix | 6 | 117 | 111.6 |
| FireResistance | suffix | 7 | 130 | 130.2 |
| ColdResistance | suffix | 7 | 133 | 130.2 |
| LightningResistance | suffix | 7 | 126 | 130.2 |
| ChaosResistance | suffix | 6 | 41 | 27.9 |
| AllResistances | suffix | 5 | 93 | 93.0 |
| LifeGainPerTarget | suffix | 1 | 21 | 18.6 |
| FireDamagePercentage | suffix | 4 | 43 | 37.2 |
| ColdDamagePercentage | suffix | 4 | 33 | 37.2 |
| LightningDamagePercentage | suffix | 4 | 36 | 37.2 |
| LifeGainedFromEnemyDeath | suffix | 6 | 57 | 55.8 |
| ManaGainedFromEnemyDeath | suffix | 6 | 50 | 55.8 |
| LightRadiusAndAccuracyPercent | suffix | 3 | 59 | 55.8 |
| DamageTakenGainedAsLife | suffix | 4 | 36 | 37.2 |
| MinionAttackSpeedAndCastSpeed | suffix | 4 | 133 | 148.8 |
| MinionLife | suffix | 4 | 158 | 148.8 |
| MinionMovementSpeed | suffix | 5 | 206 | 186.0 |
| MinionElementalResistance | suffix | 5 | 176 | 186.0 |
| Prefixes in total | | 83 | 1964 | 1959.3 |
| Suffixes in total | | 141 | 2554 | 2540.8 |

Tests (`compare_weights.py` of the spike, not part of the repository): chi-square goodness-of-fit
test, the sum of (observed - expected)^2 / expected with degrees of freedom = bins - 1. For
the test, the expected counts of each affix type are scaled to the observed total of that
type; tiers with an expected count below 5 are pooled into one bin.

| Test | Result |
|---|---|
| Groups, prefixes (11 groups) | p = 0.24 |
| Groups, suffixes (29 groups) | p = 0.36 |
| Tiers, prefixes | p = 0.53 |
| Tiers, suffixes | p = 0.67 |

Share of suffixes among the items with one mod: 69.9 % observed, 69.4 % expected.

The largest deviations are not significant:
- MinionDamage: 126 observed against 97.8 expected (scaled as in the test), z = 2.85. The
  chance that at least one of 40 groups deviates this much is about p = 0.17.
- "Discharging" (level 64, weight 100): 0 observed against 3.25 expected, p of about 0.04 for
  this tier alone. With 224 tiers, about 0.9 tiers are expected to have no hit at this
  expectation.
