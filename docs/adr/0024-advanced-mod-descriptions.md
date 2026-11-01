# 0024: Advanced mod descriptions

- Status: accepted
- Date: 2026-11-01
- Work item: AB#282

## Context
AB#282 adds the PO's section "Advanced mod descriptions" to `poe-crafting-rules`: the advanced
view (Alt in the game) shows each mod as its own block, nothing merged; implicits first, then
prefixes, then suffixes, each side in the order the mods were added (a fractured mod first). A
block shows the mod's lines with each value and its range as in the extended item text, its
tags, and for a prefix or suffix its name and tier. The tier is 1 plus the number of mods in
the base's pool on the same side and of the same family with a higher level; the item level
does not matter. The tags are those of the game data in their order, each word capitalised,
underscores replaced by spaces, separated by commas; `elemental_damage`, `physical_damage`,
`flat_life_regen` and `resource` are never shown. The extended text rule now also covers
values with decimals: the value and both bounds are converted and rounded, without trailing
zeros. Ranges of negated values are still not defined. A mod whose values a base increases
("X% increased Suffix Modifier magnitudes", ADR 0022) shows the unscaled value and range and the
mark "— X% Increased".

The page gets a toggle between "Default Mod Descriptions" (the view of today, ADR 0023) and
"Advanced Mod Descriptions". All texts of the toggle and the block labels ("I", "P", "S" with
the names "Implicit", "Prefix", "Suffix") are translation keys.

Today `crafting-core` offers `Mod::display_text` (rolled values), `Item::explicit_text`
(increased values), `Item::explicit_texts` (normal view) and `ModDefinition::range_text`
(ranges without a value, for suggestions; it stops on ranges of converted values).

Facts from the data files (RePoE export 3.29.3.3): the tags of every mod and implicit are in
the order of RePoE. Nine explicit mods have ranges of converted values (life regeneration,
divisor 60 with one decimal; leech, divisor 100 without a precision). No mod and no implicit
has a range of negative values or of a negated value. No family has mods on both sides. On the
five Breach rings no family mixes mods that the damage type lock blocks with mods it does not
block. The tag `chaos_damage` occurs only on the implicit of the Iolite Ring and is not in the
list of hidden tags. The simulator has no fractured mods.

Earlier decisions that limit the design: ADR 0017 (public types keep their fields, because the
protected tests build them as struct literals), ADR 0020 (the PO's rule names tags; the only
list of them is a constant next to the rule), ADR 0022 (the item keeps unscaled values) and
ADR 0023 (rules of a view live in `crafting-core`, not on the page; files under `web/` hold no
mod names, mod texts or stat identifiers).

## Decision
- `crafting-core` gets the advanced view as one function, like the normal view:
  `Item::advanced_mods(&self) -> Vec<ModBlock>`, one block per mod: the implicits in the order
  of the base, then the prefixes, then the suffixes, each in the order they were added.
- New public types:
  ```rust
  pub enum ModKind { Implicit, Prefix, Suffix }

  /// A piece of a line in the extended item text. The pieces of a line, put together, are
  /// the line as the extended item text shows it.
  pub enum TextPart {
      /// Text of the line and the value, e.g. `+15`.
      Text(String),
      /// The range directly after a value, with its brackets, e.g. `(12-16)`.
      Range(String),
  }

  #[non_exhaustive]
  pub struct ModBlock {
      pub kind: ModKind,
      /// Name of a prefix or suffix; none for an implicit.
      pub name: Option<String>,
      /// Tier of a prefix or suffix; none for an implicit.
      pub tier: Option<u32>,
      /// The shown tags, separated by ", "; empty if the mod shows none.
      pub tags: String,
      /// One entry per line of the mod, unscaled values.
      pub lines: Vec<Vec<TextPart>>,
      /// X of "— X% Increased" if the base increases the values of the mod's side; none for
      /// an implicit and for a side without an increase.
      pub magnitude_increase: Option<u32>,
  }
  ```
  `#[non_exhaustive]` keeps code outside the crate, including the protected tests, from
  building `ModBlock` as a struct literal; they read its fields. A later field (for example a
  fractured mark) then breaks no test.
- The parts of a block are also public, so they can be tested alone:
  - `Mod::extended_lines(&self) -> Vec<Vec<TextPart>>`: each line of the text; each value is
    converted as by `display_text`, and if its stat has a range (minimum less than maximum),
    it is followed by `Range("(min-max)")` with both bounds converted the same way. Rolled
    values unscaled. It panics on a range of a negative value or of a stat with a negative
    divisor, which the rule does not define; a unit test over the data checks that no mod of
    the data has one.
  - `Mod::tag_text(&self) -> String`: the tags by the rule. The four hidden tags are a
    constant next to the function, the only place of the list. Each word of a tag (split at
    `_`) gets an uppercase first letter.
  - `Item::tier(&self, explicit: &ExplicitMod) -> u32`: 1 plus the number of mods in
    `mod_pool(&self.base.id)` (the base's pool in the game data, neither filtered by item level
    nor by the damage type lock) with the same affix type, the same family and a higher level.
    A base without a pool in the data counts as an empty pool. `advanced_mods` reads the pool
    once per call. A unit test over the data checks that no family on a locked base mixes
    blocked and unblocked mods, because there the rule would leave open which pool counts.
- Nothing else changes: `display_text`, `explicit_text`, `explicit_texts`, `range_text`,
  `name()` and the order of `prefixes` and `suffixes` stay as they are.
- `web`: `ItemView` gets one entry per block with the texts from the translations. The toggle
  is two buttons with `aria-pressed`; the page holds the choice in memory, starts with "Default
  Mod Descriptions", stores nothing and handles no key. New keys in `web/i18n/en.json`: the two
  toggle texts, the three short labels and the three names, `T{tier}` for the tier box,
  `·` between name and tags and `— {percent}% Increased`. The last two are game labels without a
  source in the game data, like `item.requires_level`.
- The range is a separate element with its own class (colour, small margin); the text gets no
  added spaces, so a copy of the line keeps the extended format. The blocks are a CSS grid
  (short label, lines, tier box) whose lines wrap; the two toggle buttons share one row and
  wrap their texts, so the page fits 390 px without horizontal scrolling.

## Alternatives considered
- Separate functions only, the page builds the view: the order of the blocks and the
  increase mark would be rules on the page (ADR 0023).
- Tiers and shown tags written into `data/mods.json` by the converter: the tier depends on the
  base's pool, so it would be per base; the data files would grow and change for a rule that
  `crafting-core` can apply to the data it already has.
- Lines as strings with the range inside, split by the page at the brackets: the page would
  parse game texts.
- Extending `range_text`: it shows ranges without a value, for suggestions; the extended text
  is another format.
- Tags as a list, joined by the page: the comma format of the rule would live on the page.
- A checkbox or a select for the toggle: the design (variant B) has two segments that show
  both states.

## Consequences
- A new RePoE export with a family that mixes blocked and unblocked mods on a locked base, or
  with a range of a negative or negated value, fails a unit test in `crafting-core`; the
  question goes to the PO instead of being guessed.
- The implicit of the Iolite Ring shows the tag `chaos_damage` as "Chaos Damage", as the rule
  is written. The PO's evidence does not cover this tag.
- Fractured mods are not modelled. When a story adds them, `advanced_mods` puts them first on
  their side, and `ModBlock` can get a field without breaking tests.
- `mod_pool` parses the data on each call; one call per shown item. If that is too slow on the
  page, the pool can be read once, as `stat_order` is.
- The rule and the block contents are tested in `crafting-core`; the layout, the toggle and the
  390 px width are checked in the local preview (ADR 0004).
