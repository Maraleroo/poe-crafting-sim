# poe-crafting-sim
Path of Exile 1 crafting simulator in Rust and WebAssembly, built by a team of Claude agents.

Live page: https://maraleroo.github.io/poe-crafting-sim/ (published automatically from `main`).

## Project layout
- `crafting-core/`: pure crafting logic, no UI and no I/O.
- `web/`: user interface, compiled to WebAssembly with Trunk. All visible text lives in
  `web/i18n/en.json`.
- `data/`: imported game data (JSON files, embedded into `crafting-core` at build time, see `data/README.md`).
- `data-import/`: converter from the RePoE export to the files in `data/` (not part of the website), see `data/README.md`.
- `docs/adr/`: architecture decision records.

## Local preview

Prerequisites:
1. Rust via [rustup](https://rustup.rs/). The repository's `rust-toolchain.toml` selects the
   stable toolchain with `rustfmt` and `clippy`. If your setup does not install the
   WebAssembly target automatically, run `rustup target add wasm32-unknown-unknown`.
2. [Trunk](https://trunkrs.dev/), version 0.21.14: `cargo install trunk --locked --version 0.21.14`
   (or download a binary from the [Trunk releases](https://github.com/trunk-rs/trunk/releases)).

Start the preview from the repository root:

```sh
cd web
trunk serve --open
```

Trunk builds the page, opens it in your browser (http://127.0.0.1:8080/) and rebuilds on every change.
You should see the heading "Hello Crafting", below it a normal item (the Iron Ring with its name, class,
rarity, level requirement and implicit mod) and, in the footer, the `crafting-core` version and
the disclaimer.

## Development checks
```sh
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test --all
```
The same checks run in CI on every pull request. After a merge to `main`, CI builds the page with
`trunk build --release --public-url /poe-crafting-sim/` and publishes it to GitHub Pages.
