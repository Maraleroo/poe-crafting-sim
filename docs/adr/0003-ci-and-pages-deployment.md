# 0003: CI and GitHub Pages deployment

- Status: accepted
- Date: 2026-10-19
- Work item: AB#5

## Context
Every pull request must show format check, clippy without warnings and all tests.
Every merge to `main` with green CI must republish the page without a manual step.
GitHub Pages is configured with build type "workflow" (deploy via GitHub Actions).

## Decision
- One workflow `.github/workflows/ci.yml`, triggered on `pull_request` targeting `main`
  and on `push` to `main`. Never `pull_request_target`.
- Job `check`: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test --all`.
- Job `build-web` (needs `check`): `trunk build --release --public-url /poe-crafting-sim/`
  in `web/`, then `actions/upload-pages-artifact` with `web/dist`. Runs on PRs too, so
  a broken WebAssembly build is visible before the merge.
- Job `deploy` (needs `build-web`, only on `push` to `main`): `actions/deploy-pages`
  with environment `github-pages`. Because it depends on the checks in the same run,
  a red run never deploys.
- Permissions: workflow default `contents: read`; only `deploy` gets `pages: write`
  and `id-token: write`. A `concurrency` group prevents parallel deployments.
- Tool versions are pinned: toolchain via `rust-toolchain.toml`, Trunk by explicit version.

## Alternatives considered
- Separate deploy workflow triggered by `workflow_run`: more moving parts for the same gate.
- Deploying to a `gh-pages` branch: needs write access to contents; the repository is
  already set up for Actions-based deployment.

## Consequences
- The page on `main` is always the result of a green run.
- A PR could still be merged while red unless `main` requires the `check` and
  `build-web` status checks. That is a repository setting for the PO.
