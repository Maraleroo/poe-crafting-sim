# 0006: Pinned CI dependencies and Dependabot

- Status: accepted
- Date: 2026-10-20
- Work item: AB#25

## Context
The workflows reference third-party actions by mutable tags (`@v4`, `@v1`, `@v2`). A tag can
be moved to other code without any change in this repository, so an action could change
unnoticed. `rust-toolchain.toml` uses `channel = "stable"`, so a new Rust release can turn CI
red (new clippy lints) without a code change, although ADR 0003 says tool versions are pinned.
`actions/checkout@v4` runs on Node.js 20 and causes a deprecation warning.

Runtimes of the actions in use, read from their `action.yml` at the pinned commits:

| Action | Version | Runtime |
| --- | --- | --- |
| `actions/checkout` | v7.0.1 | node24 (v4 was node20) |
| `Swatinem/rust-cache` | v2.9.2 | node24 |
| `taiki-e/install-action` | v2.87.29 | composite, shell only |
| `actions/upload-pages-artifact` | v5.0.0 | composite, uses `actions/upload-artifact` v7.0.0 (node24) |
| `actions/deploy-pages` | v5.0.1 | node24 |
| `anthropics/claude-code-action` | v1.0.254 (same commit as `v1`) | composite, uses `oven-sh/setup-bun` v2.2.0 (node24) |

## Decision
- Every `uses:` in `ci.yml`, `review.yml` and `claude.yml` references a full 40-character
  commit SHA, followed by the version as a comment:
  `uses: owner/repo@<sha> # vX.Y.Z`. Annotated tags are dereferenced to their commit.
  The SHAs are looked up read-only with `gh api` (see the plan of AB#25).
- `actions/checkout` moves from v4 to v7.0.1. The breaking changes since v4 (Node.js 24,
  credentials in a separate file, fork checkout blocked for `pull_request_target` and
  `workflow_run`) do not affect these workflows: they use neither trigger.
- `.github/dependabot.yml` (version 2) has two entries, both for directory `/` with a monthly
  schedule (`interval: "monthly"`) and one group each (`patterns: ["*"]`), so there is at most
  one update PR per ecosystem and month. Monthly is the PO's decision: every Dependabot PR needs
  the manual override described in ADR 0005, so fewer PRs mean less manual work:
  - `github-actions`, commit message prefix `ci`;
  - `cargo` (the workspace `Cargo.toml` and `Cargo.lock`), commit message prefix `chore`.
  Dependabot keeps the SHA and the version comment in sync.
- `rust-toolchain.toml` sets `channel = "1.99.1"` with the components `rustfmt` and `clippy`
  and the target `wasm32-unknown-unknown`. The toolchain is updated on purpose, in its own
  work item; the PO installs the new version locally first, because agents never install
  toolchains. The Trunk version in `ci.yml` (`TRUNK_VERSION`) stays pinned and is also updated
  by hand; Dependabot does not cover either of them.
- Acceptance tests check the format of the pins (40-character SHA, version comment, minimum
  major version of `actions/checkout`, an exact `X.Y.Z` toolchain), never a specific SHA or
  version. Otherwise every Dependabot PR would break a protected acceptance test.

## Alternatives considered
- Keeping major tags (`@v4`): updates arrive without a PR, which is the risk this ADR removes.
- Pinning only third-party actions and trusting `actions/*` tags: smaller diff, but the story
  asks for all actions, and GitHub's own tags are just as mutable.
- Renovate instead of Dependabot: more options (for example the Rust toolchain), but an extra
  GitHub App; Dependabot is built in.
- A weekly or daily schedule: updates arrive sooner, but each PR needs the manual override,
  and `claude-code-action` releases almost daily.
- A Dependabot `cooldown` (waiting some days before proposing a new release): reduces churn
  and the risk of a freshly published bad release; left out to keep the configuration minimal.
  It can be added later without changing this decision.
- `channel = "1.99"` (latest patch of a minor version): still changes without a code change.

## Consequences
- An action can only change through a PR that shows the new SHA.
- Every bump of `actions/checkout` or `anthropics/claude-code-action` changes `review.yml`, so
  that Dependabot PR cannot be reviewed in CI (ADR 0005). Dependabot PRs and the review
  workflow: see the amendment for AB#25 in ADR 0005.
- Dependabot PRs have no work item and do not follow the branch naming of `CLAUDE.md`.
  How the PO handles them is described in ADR 0005 (amendment AB#25).
- Action and crate updates, including security fixes, arrive up to a month late unless
  someone updates them by hand.
- A Rust release no longer changes CI; the toolchain falls behind until someone updates it.
