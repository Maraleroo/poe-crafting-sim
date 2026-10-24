# 0012: Currency descriptions and failure messages are game data

- Status: accepted
- Date: 2026-10-24
- Work item: AB#113

## Context
AB#113 shows what a currency does and why it cannot be applied right now. The page shows a
currency's game description ("Upgrades a normal item to a magic item") when the mouse is over
it. A currency that cannot be applied shows the game's failure message (for example
"Failed to apply item: Target is not Normal Rarity."), both on hover and on click. The PO
decided that descriptions and failure messages are game texts and therefore game data, not UI
texts. The game message "Failed to apply item: Could not generate Mod." replaces the
project's own text behind the translation key `craft.error.no_valid_mod`. The behaviour
applies to every currency; AB#113 implements it for the Orb of Transmutation, and later
currencies bring their own texts.

ADR 0011 keeps currency names in `data/currencies.json` and says that texts the project
writes around currencies, such as error messages, stay translation keys. That still holds for
texts the project writes itself, but not for failure messages that come from the game.
`crafting-core` reports failures as reason codes (`CraftError`) and returns no UI text.

## Decision
- Each entry in `data/currencies.json` gets two more fields:
  - `description`: the currency's English game description, unchanged.
  - `messages`: an object that maps a reason code to the game's failure message for this
    currency, for example
    `{ "item_not_normal": "Failed to apply item: Target is not Normal Rarity.",
       "no_valid_mod": "Failed to apply item: Could not generate Mod." }`.
- `CraftError` stays the reason code. `CraftError::code()` returns its identifier in lower
  snake case (`item_not_normal`, `no_valid_mod`). This identifier is the link between the
  code and the keys in `messages`.
- `CurrencyDefinition` gets the fields `description` and `messages`.
  `Currency::description()` returns the description, and
  `Currency::failure_message(CraftError)` returns the message for a reason code. Both read
  the data file. As in ADR 0008 and 0011, invalid data is a programming error that panics:
  for example a `messages` key that is not the code of a `CraftError`, or a missing text for
  a reason the currency can fail with. Unit tests catch both.
- `web` maps a failure to its text only through `Currency::failure_message`. It shows the
  description and the failure message from `crafting-core`, never from `web/i18n/`. The key
  `craft.error.no_valid_mod` and its text are removed.
- The literal texts appear only in `data/currencies.json`, not in the code of
  `crafting-core` or `web`. This is the same rule as for currency names (ADR 0011).
- For failure messages from the game, this replaces the sentence of ADR 0011 that error
  messages around currencies stay translation keys. Texts the project writes itself (for
  example the Reset button) stay translation keys.

## Alternatives considered
- One file of game messages shared by all currencies and keyed by reason code: no duplicate
  text when several currencies fail for the same reason. But the PO decided that every
  currency brings its own texts, and whether the game uses the same message for every
  currency has not been supplied as a rule.
- Translation keys in `web/i18n/en.json`: rejected by the PO's decision that these are game
  texts.
- `CraftError` deserialized directly as the map key: `serde` would validate the keys for
  free, but the reason code would then need serde and ordering traits on a public error
  type, and `web` and the tests would still need the identifier as text.
- `crafting-core` returning the message instead of the reason code: it would mix game texts
  into the results and go against the rule that `crafting-core` returns reason codes.

## Consequences
- Every new currency needs a description and a message for each reason it can fail with,
  or the unit tests fail.
- Every new `CraftError` variant needs a code and messages in the data of the currencies
  that can fail with it.
- A later language switch can treat these texts like the other game texts. How game texts
  get translated is still open.
