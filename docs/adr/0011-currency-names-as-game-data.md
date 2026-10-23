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
