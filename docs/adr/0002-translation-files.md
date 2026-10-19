# 0002: Translation files

- Status: accepted
- Date: 2026-10-19
- Work item: AB#5

## Context
All user-facing text must come from translation files, default language English.
`crafting-core` returns reason codes, never text. Language switching is out of scope
for now, but later stories will need texts with parameters (for example a version
number or a mod count).

## Decision
- One flat JSON object per language: `web/i18n/<lang>.json`, starting with
  `web/i18n/en.json`. Keys are dotted, lower snake case, grouped by screen area,
  for example `app.title`, `footer.core_version`, `footer.disclaimer`. Values are strings.
- Placeholders use `{name}`, for example `"crafting-core v{version}"`.
- The web crate embeds the file at compile time (`include_str!`) and parses it with
  `serde_json`. Every visible text, including the document title, is looked up by key.
- `web/index.html` contains no visible text. The page is filled at runtime.
- A missing key is a programming error and must be caught by tests, not shown silently.

## Alternatives considered
- Fluent (`.ftl`): better plural and grammar support, but more dependencies and syntax
  than needed now. We can migrate when plural rules are required.
- Fetching the file at runtime: adds an async step and a second network failure mode.

## Consequences
- Adding a language means adding one JSON file and, later, a selection mechanism.
- Changing a text does not require code changes, but does require a rebuild.
- Mapping `crafting-core` reason codes to keys is the web crate's job.
