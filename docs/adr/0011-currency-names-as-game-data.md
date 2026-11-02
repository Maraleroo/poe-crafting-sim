# 0011: Currency names are game data

- Status: accepted
- Date: 2026-10-23
- Work item: AB#62

## Context
AB#62 adds the first currency button, labelled "Orb of Transmutation". The PO decided that
the name of a currency is a game text like an item name, not a UI text: a language switch
is planned, so game texts and UI texts must stay cleanly separated, and whether and how
game texts get translated is a later decision. ADR 0008 keeps game texts in data files
under `data/`, embedded into `crafting-core`; UI texts are translation keys in
`web/i18n/` (ADR 0002). So far the only game data is base items and mods. The only
currency data the PO has supplied is the name used in the story.

## Decision
- Currencies get their own data file, `data/currencies.json`, in the format and with the
  embedding and validation of ADR 0008:
  `{ "currencies": [ { "id": "orb_of_transmutation", "name": "Orb of Transmutation" } ] }`.
- `id` is a project identifier in lower snake case, not a game value. It links the entry to
  the rule code in `crafting-core`. `name` is the English game text, passed through
  unchanged. The file holds only values the PO has supplied; more fields (for example a
  poedb identifier or the stack size) are added when the PO supplies them.
- `crafting-core` has an enum `Currency` with one variant per implemented currency. Each
  variant knows its `id`, and `Currency::name()` returns the name from the data file. A
  variant without a data entry is a programming error that panics and is caught by a unit
  test. `currencies()` returns all entries of the file.
- The web page takes a currency button's label from `Currency::name()`, never from
  `web/i18n/`. Texts the project writes around currencies (for example error messages and
  the Reset button) stay translation keys.
- The literal currency name appears only in `data/currencies.json`, not in the code of
  `crafting-core` or `web`. This is the same rule as for item names and mod texts.

## Amendment (AB#326, 2026-11-02)
- Game texts (names of items, item classes, currencies and mods, mod texts, currency
  descriptions and messages; ADR 0008, ADR 0012) must not appear as text literals in the
  code of `crafting-core` and `web`. Comments and documentation may name them, for example
  the name of a currency in a doc comment.
- Reason: a comment is never shown on the website and never translated, so it does not touch
  the goal of the rule, one source for game texts that can be translated later. Paraphrases
  such as "the regal orb" made comments harder to understand, and a test in AB#299 failed on
  a comment.
- "Code" means everything in the files under `crafting-core/src` and `web/` (without build
  output) except comments: `//`, `///`, `//!` and `/* */` in Rust, `/* */` in CSS,
  `<!-- -->` in HTML and `#` in TOML, each outside string literals. Files without comment
  syntax count in full: the translation files in `web/i18n/` and the privacy pages in
  `web/legal/`, which are content of the website, not documentation.
- The acceptance tests that check that a game text lives only in the game data search the
  file contents without comments. Other text checks keep searching comments too, for
  example that the page does not use `valid_mods` (AB#248).

## Alternatives considered
- Translation key in `web/i18n/en.json`: simplest, but it mixes a game text into the UI
  texts, against the PO's decision.
- The name as a constant in `crafting-core`: no data file, but game data would be in code,
  against ADR 0008.
- A poedb metadata path as `id`, like the base items: consistent, but the PO has not
  supplied it, and game data must not be guessed. It can replace the project identifier
  when the PO supplies it or currencies are imported.

## Consequences
- A language switch can translate UI texts without touching game texts. How game texts
  are translated is still open.
- Every new currency needs a data entry and a `Currency` variant.
- If currencies are imported from poedb later, the identifier may change; the enum keeps
  the call sites stable.
