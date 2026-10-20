# 0008: Game data files embedded in crafting-core

- Status: proposed
- Date: 2026-10-20
- Work item: AB#52

## Context
AB#52 is the first story with game data: the base item Iron Ring and its implicit mod,
supplied by the PO from poedb.tw. The data must live in a data file in the repository, not
in code, and must keep every value of the PO's table, even values no function uses yet
(tags, mod family, the "Global" flag). `crafting-core` has no I/O, and the website loads
nothing from third-party servers. Base items, rarities, mods and stats are modelled in
`crafting-core`; `web` only displays them. Game texts (item name, mod text) stay English
originals and are not translated.

The PO's display text is `Adds {minimum} to {maximum} Physical Damage to Attacks`, while
the stat identifiers are `attack minimum added physical damage` and
`attack maximum added physical damage`. The PO's rule in `poe-crafting-rules` says that a
mod's display text refers to its stats by identifier, never by their order.

## Decision
- Game data lives in JSON files under `data/`, starting with `data/base_items.json`.
  `data/README.md` documents the format and the source of each file.
- `crafting-core` embeds the files at compile time with `include_str!` and parses them with
  `serde` and `serde_json`. Nothing is read from disk or the network at runtime, so the
  crate keeps its no-I/O rule and the website loads nothing from other servers.
- The embedded data is validated when it is parsed (for example: every placeholder of a
  mod text names a stat of the same mod, `min <= max`). Invalid embedded data is a
  programming error like a missing translation key (ADR 0002): it panics and is caught by
  unit tests, not handled at runtime.
- Values are stored as the PO supplies them, including identifiers with spaces and the
  display text with its placeholders. A mod text keeps the PO's placeholder names, and
  the mod has a `placeholders` map from each placeholder name to a stat identifier, for
  example `"minimum": "attack minimum added physical damage"`. The text is rendered by
  looking up each placeholder's stat by identifier, so the order of the stats in the file
  does not matter.
- Game texts (base item name, item class, mod text) are game data. They pass through
  `crafting-core` unchanged. The rule that `crafting-core` returns no user-facing text
  applies to texts written by the project (UI labels, reasons), which stay translation
  keys in `web/i18n/` (ADR 0002).

## Alternatives considered
- Placeholders named after the stat identifiers
  (`Adds {attack minimum added physical damage} to ...`): no extra map, but the file would
  no longer contain the PO's display text unchanged.
- Placeholders by position (`{0}`, `{1}`): forbidden by the PO's rule.
- Rust code generated from the data by a `build.rs`: no JSON parser in the WebAssembly
  binary, but a build script and generated code for one item.
- Loading the data at runtime with a fetch: needs I/O and an async step in the web crate,
  and `crafting-core` could no longer offer the data on its own.
- RON or TOML: no gain over JSON, which `web` and the tests already parse.

## Consequences
- `serde` (with `derive`) and `serde_json` become regular dependencies of `crafting-core`
  and are compiled into the WebAssembly binary.
- Changing game data needs a rebuild, but no code change.
- `crafting-core` refers to files outside its own directory, so it cannot be published to
  crates.io as it is. That is not planned.
- Later imports of larger poedb data sets add files under `data/` in the same format; a
  bigger import may need a new decision on size and format.
