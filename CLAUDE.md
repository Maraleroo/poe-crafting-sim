# PoE Crafting Simulator

Web-based crafting simulator for Path of Exile 1, written in Rust and compiled
to WebAssembly, hosted on GitHub Pages. Built by a team of Claude agents;
the human is the Product Owner (PO).

## Architecture
- `crafting-core/`: pure crafting logic (items, mod pools, weights, currencies). No UI, no I/O.
- `web/`: user interface, uses `crafting-core` via WebAssembly. All user-facing text comes
  from translation files; default language is English.
- `crafting-core` returns structured results and reason codes, never user-facing text.
- `data/`: imported game data.
- `docs/adr/`: architecture decision records.

## Commands
- Format: `cargo fmt --all`
- Lint: `cargo clippy --all-targets -- -D warnings`
- Test: `cargo test --all`
- Local preview: defined in the walking skeleton story

## Workflow
- Every change belongs to a work item in Azure Boards. Stories start with `/story AB#<id>`.
- Work items are written in German. Everything in this repository (code, comments,
  commits, pull requests, docs) is written in English.
- One branch per work item: `feat/<id>-<short-name>` or `fix/<id>-<short-name>`.
- Commits and PR titles follow Conventional Commits. Scopes: core, web, data, ci, docs.
- Every PR description contains `Fixes AB#<id>`.
- Versioning follows Semantic Versioning, starting at 0.1.0.

## Hard rules
- Never push to `main`, never force-push, never merge a pull request. Merging is the PO's acceptance.
- In Azure Boards, only create Tasks. Never create User Stories, Features or Epics;
  propose them to the PO instead.
- Never modify, delete or disable acceptance tests (`crafting-core/tests/acceptance.rs` and `crafting-core/tests/acceptance/`). If one looks wrong, stop and report.
- Never hard-code expected test values or add special cases that exist only for tests.
- If a Path of Exile rule is missing or unclear in `poe-crafting-rules`, report it. Never guess game mechanics.
- This repository is public: never commit secrets, tokens or personal data.
- Never silence clippy warnings globally.
- Never hard-code user-facing text. Add a translation key instead.
