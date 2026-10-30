# Host the game rules as a pure `rules` module

- Status: proposed
- Date: 2026-10-30

Technical Story: masters-of-the-dungeon ADR-0025, issue #140.

## Context and Problem Statement

The game's rules (effects resolution, character derivation, door/solid
overlays, actions) lived in a TS package that called into this engine for
path/LoS/light. Rules and board reads belong together, and the rules should
run without a JS host.

## Decision Outcome

`src/rules/` holds the rules as plain Rust over serde data: no
`wasm_bindgen`, no globals, randomness passed in. It reads boards through a
`Board` trait that `RouteGrid` implements over a per-layer
`LightingEngine` it syncs from the model. `src/rules/wasm.rs` is the only
JS face (`Rules` class: `update`, `casts`, `apply`).

### Consequences

- The npm package grows `serde`/`serde-wasm-bindgen`; the lighting API is
  unchanged.
- Rule tests run natively (`cargo test --lib rules`).
