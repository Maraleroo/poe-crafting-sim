# 0004: Acceptance tests for web and pipeline criteria

- Status: accepted
- Date: 2026-10-19
- Work item: AB#5

## Context
Acceptance tests live in `crafting-core/tests/acceptance/` and are protected from changes.
Some criteria are about the web page, the CI pipeline or the public URL. A browser test
harness would add tooling (headless browser, driver) that the project does not need yet.

## Decision
- Criteria with a stable, file- or API-level contract are covered by Rust acceptance
  tests in `crafting-core/tests/acceptance/story_<id>.rs`:
  - the public API of `crafting-core` (for example `version()`);
  - the English translation file `web/i18n/en.json`: it parses, contains the required keys
    and the exact texts from the acceptance criteria;
  - that visible texts from the criteria appear in no file under `web/` other than the
    translation file, and that `web/index.html` has no visible text.
  These tests read files relative to `env!("CARGO_MANIFEST_DIR")`. `serde_json` is a
  dev-dependency of `crafting-core` only; the library itself stays free of I/O.
- Criteria that need a running browser, the CI system or the public URL are checked
  outside `cargo test`: by the CI run visible on the PR, by the PO in the local preview
  and on the published page after the merge. The PR description lists these steps.
- For CI and process stories (workflows, skills, agents), the file-level checks stay lean
  and text-level: a file exists, contains or does not contain a string. No parsers or
  structural models of YAML or Markdown files (added by AB#24, see `writing-acceptance-tests`).
- No headless browser tests for now. When the first interactive story needs them, a new
  ADR decides the tool and the test location.

## Alternatives considered
- Headless browser tests (`wasm-bindgen-test` or Playwright) in CI: real end-to-end
  coverage, but more tooling and CI time than a walking skeleton justifies.
- A separate acceptance test location under `web/`: cleaner separation, but it requires
  changing the protected-test rules in `CLAUDE.md` and the test skill, which is the PO's call.

## Consequences
- `crafting-core` tests depend on the path of the web translation file. Moving it
  means a new story with updated acceptance tests.
- Rendering, WebAssembly loading and deployment are not covered by `cargo test`.
