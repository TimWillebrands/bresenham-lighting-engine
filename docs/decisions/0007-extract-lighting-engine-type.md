# Extract LightingEngine type to own mutable runtime state

- Status: proposed
- Date: 2026-05-24

## Context and Problem Statement

All mutable runtime state in the engine lives in process-wide statics: `COLLISION_SYSTEM` (`collision.rs`), `LIGHTS` (`lighting.rs`), `BLOCKMAP` (`block_map.rs`), and the implicit tile map. This was natural for the original use case (one WASM instance per browser page), but it creates two distinct problems:

1. **No parallel tests.** `cargo test` runs threads in parallel by default. Two scenarios that touch the engine race on shared globals, producing non-deterministic results. The existing `tests/output_mechanisms.rs` suite is silently affected — generated images vary with thread scheduling, and the suite passes regardless of correctness.
2. **No embeddability.** External Rust callers that want more than one independent engine instance (e.g. a server rendering several scenes, a test harness comparing variants, a level editor previewing alternatives) cannot have one. The crate's *only* supported usage shape is "one engine per process."

The visual feedback loop we want to build for agent-driven development (see [CONTEXT.md](../../CONTEXT.md) and ADR-0006) requires (1). The crate's long-term framing as a reusable lighting engine — not a WASM module that happens to be open-source — requires (2).

## Decision Drivers

- Scenario-level state isolation for parallel tests
- Embeddability for non-WASM Rust callers
- Zero breakage of the existing WASM/JS surface — browser demos must continue to work
- Avoid duplicating per-instance state that is genuinely process-global (the precomputed `ALL_RAYS` lookup table)

## Considered Options

- **A. Keep globals; force single-threaded tests** (`#[serial]` or `--test-threads=1`)
- **B. Keep globals; reset between scenarios** (per-scenario `world::reset_all()` plus `#[serial]`)
- **C. Extract a `LightingEngine` struct** that owns all per-instance mutable state; expose top-level functions as thin wrappers over a default singleton for WASM back-compat

## Decision Outcome

Chosen option: **C**.

Define `LightingEngine` in `src/engine.rs` (or top-level `lib.rs`) owning:

- the tile map (`Vec<TileId>` + derived metadata)
- the block map (`BlockMap` — derived from the tile map; recomputed on tile mutation)
- the collision system (`HybridCollisionMap` containing `UnionFind` for rooms and `PixelCollisionMap` for cell-level blockers)
- the light registry (`LightRegistry` holding `Light` instances and their canvases)

Provide methods that mirror today's free functions: `update_or_add_light`, `set_pixel`, `set_tile_map`, `is_blocked`, `light_canvas`, etc.

Keep `ALL_RAYS` (the precomputed Bresenham ray lookup table) as a `pub(crate) static Lazy<...>`. It is a pure function of the `MAX_DIST` and `ANGLES` compile-time constants, immutable, and large (~MB). Duplicating it per instance has no upside and a real memory cost.

Preserve the existing public API by adding a default singleton:

```rust
static DEFAULT_ENGINE: Lazy<RwLock<LightingEngine>> =
    Lazy::new(|| RwLock::new(LightingEngine::new()));

#[wasm_bindgen]
pub fn update_or_add_light(id: u8, r: u8, x: i16, y: i16) -> *const Color {
    DEFAULT_ENGINE.write().unwrap().update_or_add_light(id, r, x, y).as_ptr()
}
```

The singleton is documented as a back-compat shim for WASM/JS callers; Rust code (tests, examples, future embedders) is expected to construct its own `LightingEngine::new()` and call methods on it. This is what makes parallel tests safe — each test owns its instance.

### Positive Consequences

- Tests construct fresh engines, run in parallel, never race.
- The crate gains a real embedder-facing Rust API (`LightingEngine::new()` + methods) rather than only a WASM surface.
- Scenarios become trivially composable: take `&mut LightingEngine`, mutate, hand back.
- The default singleton can be deprecated later (a separate ADR) if/when WASM consumers migrate to constructing their own instance via `wasm_bindgen`.

### Negative Consequences

- Real refactor surface — every public function in `collision`, `lighting`, `block_map` gains a method counterpart; internal callers (notably `lighting.rs:196` calling `collision::is_blocked`) must take `&self` or pass the engine through.
- Two API paths exist (free function via default singleton, method on `&mut LightingEngine`). Documented as "free functions = WASM shim only," but a future contributor could still pick the wrong one.

## Pros and Cons of the Options

### A. Force single-threaded tests

- Good, because tiny diff — one attribute or one config line.
- Good, because no engine changes.
- Bad, because doesn't fix embeddability at all.
- Bad, because test suite serialises forever; easy to forget the attribute on a new test and get a silent flake.

### B. Reset globals between scenarios

- Good, because cheap to ship.
- Good, because preserves existing API.
- Bad, because still serial (resets are mutually exclusive with anything else touching globals).
- Bad, because nothing about it improves the crate as a library.
- Bad, because a missed reset between scenarios is a silent correctness bug, not a loud failure.

### C. Extract `LightingEngine`

- Good, because solves both problems (parallel tests + embeddability) in one move.
- Good, because pushes the crate toward being a real library, not a WASM-only artifact.
- Bad, because biggest refactor of the three.
- Bad, because temporarily creates two API paths (singleton wrappers + struct methods).

## Implementation Sketch

1. Land removals first (separate PR): delete `IS_BLOCKED_FN`/`reset_is_blocked_fn` (`lib.rs:100-118`), `TileCollisionMap` (`collision.rs`, dead since ADR-0006), and the `MOCK_OBSTACLES` apparatus in `tests/output_mechanisms.rs`. Smaller diff for the refactor that follows.
2. Introduce `LightingEngine` with the four owned fields. Move `impl` blocks from current modules' free functions into methods.
3. Convert each existing free function into a thin `DEFAULT_ENGINE.write().unwrap().<method>(...)` wrapper, preserving `#[wasm_bindgen]` attributes.
4. Migrate internal cross-module calls (e.g. `lighting.rs:196`) to operate on `&self` / `&mut self`.
5. Add `LightingEngine::render_canvas_text(light_id) -> String` as the debug primitive consumed by both the exploration loop (always) and the regression loop (on failure).
6. Build the scenarios module and the two-loop infrastructure on top.

## Links

- Builds on [ADR-0006](0006-unify-collision-detection.md) — collision is already unified; this ADR moves the unified system into per-instance ownership.
- Vocabulary aligned with [CONTEXT.md](../../CONTEXT.md).
- Enables the agent-driven visual feedback loop (exploration via `cargo run --example scenario`, regression via `cargo test`).
