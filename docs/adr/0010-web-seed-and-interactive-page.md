# 0010: Seed of the web page and the first interactive page

- Status: accepted
- Date: 2026-10-23
- Work item: AB#62

## Context
AB#62 adds the first interaction to the start page: an "Orb of Transmutation" button that
makes the Iron Ring magic and a "Reset" button that restores the normal ring. Every click
can give a different result, also after reloading the page, while tests use fixed seeds.
ADR 0009 left open where the web page takes its seed from until the first story whose rolls
must differ between visits; until now the page used the fixed seed `0`. The page must not
store anything in the visitor's browser and must not load anything from third-party
servers (ADR 0007). ADR 0004 left the choice of browser tests to the first interactive
story.

## Decision
- When the start page loads, it takes 8 random bytes from the browser's Web Crypto API
  (`crypto.getRandomValues`, `web-sys` feature `Crypto`) and creates one `SeededRng` from
  them. This generator lives in memory while the page is open; every roll of the page
  (creating the item, Orb of Transmutation, Reset) draws from it. Each click therefore
  continues the sequence, and each page load starts a new one.
- `getRandomValues` is built into the browser: it sends no request and uses no browser
  storage. The seed is neither stored nor shown.
- If the Web Crypto API is unavailable, the page reports the error like any other
  rendering error. There is no fallback to a fixed or time-based seed.
- `crafting-core` is unchanged in this respect: it never creates seeds, every rolling
  function takes the caller's `Rng` (ADR 0009).
- The web crate keeps the shown item, its base item and the generator in one plain Rust
  struct with one method per button action and a method that says whether the orb can be
  used (asked from `crafting-core`). The struct and the item view built from it compile
  natively and are unit-tested without a browser. The DOM code only registers the click
  listeners (`wasm-bindgen` closures that live as long as the page) and renders the item
  section again from the view after each click.
- Still no headless browser tests (ADR 0004): the behaviour behind the buttons is covered
  by native unit tests and by the acceptance tests through `crafting-core`; the rendering
  is checked by the PO in the local preview.

## Alternatives considered
- `Math.random()`: no better than the Web Crypto API for this use, and it needs `js-sys` as
  a direct dependency.
- The current time (`Date.now()`, `performance.now()`): predictable, and two loads in the
  same millisecond give the same rolls.
- The `getrandom` crate (directly or through `rand`): a new dependency that needs extra
  features and compiler configuration for `wasm32-unknown-unknown`, which ADR 0009 avoided.
- A fresh seed from the Web Crypto API for every click: also works, but adds a second
  source of randomness for no gain; one generator per page keeps one code path.
- A seed in the URL: would make rolls reproducible and shareable, but nothing asks for that
  yet. A later share-link story can decide it.
- Headless browser tests: more tooling than two buttons justify (ADR 0004).

## Consequences
- `web-sys` gets the feature `Crypto` and the features needed for buttons and click
  listeners. No new crate is added.
- Rolls on the page cannot be reproduced afterwards. A reproducible seed, for example for
  share links, needs a new decision.
- Tests never depend on the browser's seed; they create `SeededRng::new(seed)` themselves.
- Later interactive features add methods to the same page state struct and stay testable
  without a browser.
