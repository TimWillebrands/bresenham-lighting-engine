# Per-engine `ALL_RAYS` and runtime-configurable resolution

- Status: accepted
- Date: 2026-05-25
- Revises: [ADR-0007](0007-extract-lighting-engine-type.md) on the `ALL_RAYS` ownership question

## Context

ADR-0007 extracts `LightingEngine` as the owner of per-instance mutable state but explicitly keeps `ALL_RAYS` (the precomputed Bresenham ray LUT) as a process-wide static. That choice was sound under the assumption that `CELLS_PER_TILE`, `TILES_PER_ROW`, and therefore the maximum useful ray length are compile-time constants shared by every engine instance in the process.

The downstream consumer (Dypgangers, see workspace root [CONTEXT.md](../../../../CONTEXT.md)) hit two facts that break that assumption:

1. **JS and Rust held divergent `CELLS_PER_TILE` values** (9 vs 6). Rather than picking one, we want both sides to read the resolution from a single runtime source so this class of drift can't recur.
2. **Per-`Layer` `LightingEngine` instances** are useful in the game (one engine per map layer). A future map with different cell granularity for, say, a coarse overview layer vs a detail layer is a plausible extension, and the resolution choice is also genuinely useful to A/B test for visual quality.

If resolution is per-engine, `MAX_DIST` (which is tied to the maximum sensible light radius, which is tied to `CELLS_PER_ROW`) is also per-engine, and a process-wide `ALL_RAYS` no longer composes.

## Decision

- `CELLS_PER_TILE` and `TILES_PER_ROW` become **`LightingEngine` constructor parameters**, not module constants. The `pub const` values in `constants.rs` are removed (or kept only as suggested defaults).
- `ALL_RAYS` moves into `LightingEngine` as `Vec<Vec<Point>>` (or equivalent), computed in `LightingEngine::new` from that engine's `MAX_DIST = max_light_radius(cells_per_row)`.
- The process-wide cache is removed. Tests, examples, and the WASM default singleton each construct their own engine and pay the precomputation cost once at construction time (10–100 ms per ADR-0007's note, acceptable at engine-construction granularity).

## Consequences

- The compile-time asserts in `constants.rs` that depend on the consts go away or move into runtime asserts in `LightingEngine::new`.
- Memory cost: one LUT per engine instead of one per process. At ~MB scale per engine and a small handful of engines (one per `Layer`), still negligible against the cell/block-map memory the same engine already owns.
- Construction cost: `LightingEngine::new` becomes "slow" (10–100 ms). Callers that want hot starts pre-construct engines; the game does this at layer-init time, off the render path.
- The `tile_to_cell_coords` / `cell_to_tile_coords` / `tile_index` / `cell_index` free functions in `constants.rs` either take the resolution as parameters or move onto `LightingEngine` as methods.
- WASM exports change shape: the singleton API in the default engine is constructed with hard-coded defaults; new `#[wasm_bindgen]` constructors expose the parameterised form for JS consumers.
