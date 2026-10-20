# 0007: Legal pages in the web app and privacy constraints

- Status: accepted
- Date: 2026-10-20
- Work item: AB#39

## Context
The website needs a privacy policy in German (legally binding) and English (translation).
The PO supplies both texts as Markdown files and the agents must not change their wording;
only conversion to HTML is allowed. The footer of every page links to the English policy,
and the two policies link to each other. The site is a single Trunk page under the
sub-path `/poe-crafting-sim/` (ADR 0001); the local preview serves from `/`. All visible UI
text comes from translation files (ADR 0002), and `web/index.html` has no visible text.
The privacy policy states that the site loads nothing from third-party servers and stores
nothing in the visitor's browser, so the code has to keep it that way.

## Decision
- Legal texts are content files written by the PO: `web/legal/privacy.de.md` and
  `web/legal/privacy.en.md`. They are committed unchanged and are not copied into
  `web/i18n/*.json`. Agents never edit their wording.
- The web crate embeds them with `include_str!` and converts them to HTML at runtime with
  `pulldown-cmark` (`default-features = false`, feature `html`). The conversion:
  - renders soft line breaks as `<br>`, so address blocks keep their lines;
  - turns raw HTML in the Markdown into escaped text, so a legal file can never embed
    content;
  - leaves bare URLs as text (CommonMark does not link them). If the PO wants a clickable
    link, the PO writes `<https://...>` in the Markdown, which becomes a normal `<a>` link.
  Images are not supported; tests check that the legal files contain none.
- The legal pages are routes of the existing single page, selected by a query parameter:
  - `?page=privacy`: English policy;
  - `?page=privacy-de`: German policy;
  - anything else: the home page.
  Links use these relative hrefs, so they resolve correctly under `/poe-crafting-sim/` and
  under `trunk serve`. Following a link reloads the page; there is no History API and no
  event listener. The route is parsed by a pure function that is unit-tested natively.
- The same render code builds the footer for every route, so every page has it. The German
  policy is placed in an element with `lang="de"`.
- UI text around the policies (footer link, cross-links, document title) comes from
  `web/i18n/en.json`. The UI stays English on both policy pages; language switching is
  still out of scope (ADR 0002).
- Privacy constraints: the website loads no resource from third-party servers and uses no
  browser storage (cookies, Local Storage, Session Storage, IndexedDB). This is a hard rule
  in `CLAUDE.md`, a critical criterion in `reviewing-code` and covered by the text-level
  acceptance tests of AB#39 (ADR 0004). A change to either constraint first needs a privacy
  policy updated by the PO.

## Alternatives considered
- Static HTML files copied by Trunk (`copy-file`): pages work without WebAssembly, but the
  footer and the link texts would be hard-coded and duplicated, against ADR 0002.
- A Trunk post-build hook that generates static pages with a native Rust tool: no
  hard-coded text and no WebAssembly needed to read the policy, but a second build tool,
  a staging-directory contract and more CI time for two pages.
- Hash routes (`#privacy`): no reload, but needs a `hashchange` listener and closures.
- Conversion in a `build.rs`: smaller WebAssembly, but an extra build script and generated
  files; the runtime cost of converting two short documents is negligible.
- Putting the texts into the translation JSON: loses the document structure and mixes a
  legally binding German text into the English UI translations.

## Consequences
- Reading the policy needs JavaScript and WebAssembly, like the rest of the site.
- The PO updates a policy by replacing its Markdown file; no code change is needed.
- `pulldown-cmark` is a new dependency of the web crate; Dependabot keeps it updated.
- The URLs `?page=privacy` and `?page=privacy-de` are public and should stay stable when a
  router is introduced later.
- Rendering and real network or storage behaviour are not covered by `cargo test`; they are
  checked in the release build and on the published page after the merge.
