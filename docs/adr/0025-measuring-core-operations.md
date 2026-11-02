# 0025: Measuring the core operations natively and in every pull request

- Status: accepted
- Date: 2026-11-02
- Work item: AB#335

## Context
Since AB#282 (0.17.0), every display of the item parsed the whole game data again; an Orb of
Alteration took 3.5 ms instead of 0.2 ms. Nobody noticed for three versions, until the PO watched
the advisor's acceptance checks. AB#314 fixed it. The PO wants every pull request to show whether
it makes the core faster or slower.

PO decisions on AB#335: the Rust core is measured natively in release mode, not the website in the
browser (no Trunk, no `wasm-opt`). Timings vary on CI machines, so they never fail the build; they
are only shown. More precise measurements for a later analysis (for example with criterion) are
made on a branch of their own that is never merged.

Constraints: no new dependencies, only the standard library and the public functions of
`crafting-core`; nothing is compiled into the website; the change is not a release. Review
criterion 8 counts changes to `web/`, `crafting-core/src/`, `data/`, `Cargo.toml`, `Cargo.lock` and
`rust-toolchain.toml` as release-relevant.

## Decision
- The measuring program is the Cargo example `crafting-core/examples/measure.rs`. Cargo finds it
  without an entry in `Cargo.toml`. It can only use the dependencies of `crafting-core`; by
  decision it uses only `std` and `crafting_core`. The website's build never compiles examples
  of its dependencies, and no release-relevant file changes. `cargo clippy --all-targets` lints it
  and `cargo test --all` compiles it, so it cannot rot unnoticed.
- Local command: `cargo run --release -p crafting-core --example measure`. It measures every
  operation and prints a table.
- Every operation has a stable key (ASCII, `snake_case`). Currency operations use the
  currency's project identifier (`Currency::id()`, not a game text, ADR 0011); the others are
  `default_view_rows`, `advanced_view_rows` and `first_data_load`. The value is the mean time
  per call. It is measured for a fixed time per operation after a warm-up, with a
  `SeededRng` from one fixed seed per operation. Each measured call must succeed, so an error
  path is never measured.
- `first_data_load` cannot be repeated in one process, because the game data is parsed once
  behind a `OnceLock`. The program starts itself again (`std::env::current_exe()` and
  `std::process::Command`) with an internal argument. The child process times its first call of
  `base_items()` with `std::time::Instant` and prints the time; the parent averages over the
  children. Starting the process is not part of the time.
- Command line, kept stable from this ADR on, because CI runs the program of `main` with the
  workflow of the pull request:
  - `--out <file>` also writes one line `<key>\t<nanoseconds>` per operation;
  - `--compare <main file> <PR file> [--summary <file>]` writes a Markdown table (operation,
    time on `main`, time in the PR, change in percent) to the summary file, or to standard output
    without `--summary`. On standard output, it prints one GitHub warning command
    (`::warning title=...::<key> ...`) for each operation that is more than 50 % slower than on
    `main`. A missing main file, or an operation that only one side has, shows `–` and gives no
    warning. The exit code is 0 whatever the timings are.
- A separate workflow `.github/workflows/measure.yml`, triggered on `pull_request` to `main`
  (never `pull_request_target`), with `contents: read`, actions pinned as in ADR 0006. One job
  measures both states on the same runner: the PR state is the checked-out merge commit and the
  `main` state is its first parent, checked out as a `git worktree`. If `main` has no measuring
  program yet, the summary says so and only the PR is measured. Timings never fail the job; a
  build or run error of the measuring program does.

## Alternatives considered
- A new workspace crate (for example `tools/measure`): needs a new member in `Cargo.toml` and a
  change to `Cargo.lock`, both release-relevant under review criterion 8.
- `crafting-core/benches/` with `harness = false`: needs a `[[bench]]` entry in
  `crafting-core/Cargo.toml` and offers nothing over an example without a bench library.
- A benchmark library such as criterion: a new dependency, excluded by the story. Allowed only on
  a discarded analysis branch.
- A job in `ci.yml`: it would need an `if:` for pull requests, which the review counts as a
  skippable check, and it would mix an informational measurement with the gating checks.
- Comparing in shell or `awk` in the workflow: harder to run locally and to read than Rust in the
  same program.
- Failing the build on a slowdown: excluded by the PO, because timings on CI machines vary.

## Consequences
- Every pull request, including Dependabot pull requests, shows a table of the core operations in
  its run summary and a yellow warning for each operation that is more than 50 % slower. The
  first pull request (AB#335) has no times for `main`. The comparison is first visible on a
  pull request after the merge, so AB#335 is verified after the merge.
- Noise on shared runners can cause a false warning; it never blocks.
- Changing the command line of the program needs a transition, because the workflow of a pull
  request runs the program of `main`.
- The lazy pool of each base is resolved in the warm-up of the first currency call, not in
  `first_data_load`.
- New operations are added to the program as the core grows; the plan of the story that adds a
  currency names its key.
