# 0001: Cargo workspace and web stack

- Status: accepted
- Date: 2026-10-19
- Work item: AB#5

## Context
The walking skeleton needs a page that talks to `crafting-core` through WebAssembly,
can be previewed locally with one command and can be published as static files on
GitHub Pages under the sub-path `/poe-crafting-sim/`. All code must pass the same
gates (`cargo fmt`, `cargo clippy`, `cargo test`). There is no interactive UI yet.

## Decision
- One Cargo workspace at the repository root with two members:
  - `crafting-core/`: package `crafting-core`, library `crafting_core`. Pure Rust,
    no dependencies on `wasm-bindgen`, `web-sys` or any I/O.
  - `web/`: package `crafting-web`, a binary crate compiled to `wasm32-unknown-unknown`.
    It depends on `crafting-core` by path and uses `wasm-bindgen` and `web-sys` to
    build the DOM. Pure logic (translations, text formatting) lives in modules that
    compile and are unit-tested natively.
- `crafting-core` exposes its version as `pub fn version() -> &'static str`, implemented
  with `env!("CARGO_PKG_VERSION")`. The web crate calls it at runtime in the browser.
- Trunk builds and serves the web crate (`web/index.html`, `web/Trunk.toml`).
  Local preview: `trunk serve --open` in `web/`. Release build for Pages:
  `trunk build --release --public-url /poe-crafting-sim/`.
- `rust-toolchain.toml` pins the stable channel with `rustfmt`, `clippy` and the
  `wasm32-unknown-unknown` target. Trunk is installed in a pinned version.
- Both crates start at version 0.1.0. `Cargo.lock` is committed.
- No UI framework yet. The choice (for example Leptos or Yew) is made in its own ADR
  in the first story with interactive UI.

## Alternatives considered
- Hand-written JavaScript UI with a thin `wasm-bindgen` wrapper (`wasm-pack`): less Rust,
  but the UI would sit outside the Rust quality gates and need its own tooling.
- A UI framework now: more dependencies and compile time for one heading and a footer;
  migration from plain `web-sys` to a framework is cheap at this size.
- `wasm-pack` plus a static file server: two tools and a hand-written loader instead of
  Trunk's single build and serve command.

## Consequences
- Contributors need `rustup` and Trunk. The README lists the setup steps.
- `cargo clippy --all-targets` and `cargo test --all` also build the web crate natively,
  so browser-only code must compile on the host target.
- The Pages sub-path must be passed as `--public-url`; the local preview serves from `/`.
