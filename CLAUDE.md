# PoE Crafting Simulator

Web-based crafting simulator for Path of Exile 1, written in Rust and compiled
to WebAssembly, hosted on GitHub Pages. Built by a team of Claude agents;
the human is the Product Owner (PO).

## Architecture
- `crafting-core/`: pure crafting logic (items, mod pools, weights, currencies). No UI, no I/O.
- `web/`: user interface, uses `crafting-core` via WebAssembly. All UI text comes from
  translation files; default language is English.
- `crafting-core` returns structured results and reason codes, never UI text.
- All texts that come from the game, such as names of items, currencies and mods and mod texts, are data (ADR 0008, ADR 0011).
- `data/`: imported game data.
- `docs/adr/`: architecture decision records.

## Commands
- Format: `cargo fmt --all`
- Lint: `cargo clippy --all-targets -- -D warnings`
- Test: `cargo test --all`
- Local preview: `trunk serve --open` in `web/` (needs Trunk and the `wasm32-unknown-unknown` target, see README)
- Pages build: `trunk build --release --public-url /poe-crafting-sim/ --dist ../target/pages` in `web/`.
  It writes to `target/pages`, which Git ignores, not to `web/dist`, so a running `trunk serve`
  keeps its preview.
  In Git Bash on Windows, run it as `MSYS_NO_PATHCONV=1 trunk build ...`; otherwise Git Bash
  rewrites `/poe-crafting-sim/` into a Windows path.

## Workflow
- Every change belongs to a work item in Azure Boards. Stories start with `/story AB#<id>`.
- Work items are written in German. Everything in this repository (code, comments,
  commits, pull requests, docs) is written in English.
- One branch per work item: `<prefix>/<id>-<short-name>` with prefix `feat`, `fix`, `chore`,
  `ci` or `docs`, created from `origin/main` with `--no-track`.
- Commits and PR titles follow Conventional Commits. The type follows the kind of change
  (`feat` only for product functionality). Scopes: core, web, data, team (`.claude/` and
  `CLAUDE.md`); no scope that repeats the type.
- Every PR description contains `Fixes AB#<id>`, or `Related AB#<id>` when the story has
  acceptance criteria that can only be checked after the merge.
- Versioning follows Semantic Versioning 2.0.0 (https://semver.org/) strictly.
- The public API is the user-visible behaviour of the website. Once items can be pasted from the game, the accepted item texts (Ctrl+C and Ctrl+Alt+C in Path of Exile 1) are part of it.
- A release is every merge that changes the published website. Merges that only change the team configuration or documentation are not releases.
- Before 1.0.0, every release increments the minor version and resets the patch version to 0.

## Hard rules
- Never push to `main`, never force-push, never merge a pull request. Merging is the PO's acceptance.
- In Azure Boards, only create Tasks. Never create User Stories, Features or Epics;
  propose them to the PO instead.
- Never modify, delete or disable existing acceptance tests (`crafting-core/tests/acceptance.rs` and `crafting-core/tests/acceptance/`). If one looks wrong, stop and report.
  Only the test designer adds new acceptance tests, including the story's `mod` line in `crafting-core/tests/acceptance.rs`.
  Exception: when the PO has confirmed for a story that one of its criteria replaces a criterion of an earlier story, the test designer adapts the existing test of that criterion in the story's `test(...)` commit. The developer never changes acceptance tests.
- Never hard-code expected test values or add special cases that exist only for tests.
- If a Path of Exile rule is missing or unclear in `poe-crafting-rules`, report it. Never guess game mechanics.
- This repository is public: never commit secrets, tokens or personal data.
- Never silence clippy warnings globally.
- Never hard-code UI text. Add a translation key instead.
- Never download or install tools or binaries yourself. Use only tools that are already installed; if one is missing, stop and ask the PO to install it.
- Only the PO adds or removes the review-override label. Agents never change labels on pull requests.
- The website loads nothing from third-party servers and stores nothing in the visitor's browser. Any change to that first needs a privacy policy updated by the PO.
- The website shows no ads, affiliate links or donation links. Adding any of them first needs a PO decision on the Impressum, because it changes the legal situation.
