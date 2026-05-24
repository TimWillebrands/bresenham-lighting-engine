# Change Log

All notable changes to `bresenham-lighting-engine` are documented here.
This project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased] — 2026-05-25

### Added

- **`LightingEngine` type** ([`src/engine.rs`](src/engine.rs)). Construct one
  with `LightingEngine::new()` and call methods on it directly. Each engine
  owns its own tile map, block map, collision system, and light registry —
  multiple engines can coexist in one process. This unlocks two things that
  were previously impossible:
    - **Parallel tests**: each test builds its own engine, so the default
      `cargo test` thread pool no longer races on shared globals.
    - **Embedding**: Rust callers (servers, level editors, comparison
      harnesses) can hold more than one scene at a time. See
      [ADR-0007](docs/decisions/0007-extract-lighting-engine-type.md).
- **`LightingEngine::render_canvas_text(light_id)`** — ASCII-matrix view of a
  light's canvas, suitable for stdout, panic messages, or piping to other
  tools.
- **Scenarios module** ([`src/scenarios/`](src/scenarios/mod.rs)) — plain Rust
  functions like `single_light` and `object_shadow` that populate a
  `LightingEngine`. Shared between the exploration CLI and the regression
  tests so the same scene definition drives both.
- **`scenario` example** — `cargo run --example scenario -- --list` enumerates
  the available scenarios; `--name <NAME>` prints the ASCII matrix;
  `--output-format png --out path.png` renders a PNG.
- **`CONTEXT.md`** — canonical vocabulary (Tile, Cell, Wall, Object, Room,
  LightingEngine, Light, Canvas, Ray). Read this before contributing.
- **`.cargo/config.toml`** sets `RUST_MIN_STACK=8388608` so `cargo test`
  works without remembering the env var.

### Changed

- `lighting::*`, `collision::*`, and `block_map::*` free functions are now
  thin shims that forward to a process-wide `DEFAULT_ENGINE` singleton.
  **WASM/JS callers are unaffected** — every `#[wasm_bindgen]` function
  keeps its current name and signature, including `put`, `put_solid_color`,
  `put_custom_color`, `set_tile`, `set_map_data`, `set_pixel`,
  `set_pixel_batch`, `clear_pixel_collisions`, `get_tiles`, and
  `get_blockmap`.
- New Rust code should prefer `LightingEngine` methods; the free functions
  exist for back-compat and operate on a shared global, which serialises
  callers under a `RwLock`.

### Removed

- `IsBlockedFn` and `reset_is_blocked_fn` (dead since the collision-system
  rewrite — they were never read at runtime).
- `TileCollisionMap` (superseded by the unified `HybridCollisionMap` in
  [ADR-0006](docs/decisions/0006-unify-collision-detection.md); was no
  longer reachable from any code path).
- `VISUAL_TESTING.md`, `tests/output_mechanisms.rs`, `tests/README.md`, the
  `test_output/` and `test_output.before/` directories, and
  `benches/collision_performance.rs`. The PNG-snapshot harness they
  described had silently broken when the collision modes were unified —
  every "obstacle" snapshot in version control was either from
  pre-unification code or from post-unification code with no occlusion
  wired up. Replaced by the scenarios CLI and `tests/scenarios.rs`.

### Migration notes

- **JS / WASM callers**: no change required. The `pkg/` artifact keeps the
  same exports and ABI.
- **Rust callers using free functions**: still work, but each call now
  takes the global write lock. For tests or any code that wants
  independent scenes, switch to `LightingEngine::new()` and call methods
  on the instance directly.
- **Test authors**: do **not** use `DEFAULT_ENGINE` from tests. Each test
  must construct its own `LightingEngine` — that is what makes parallel
  test execution safe.

## [0.2.7] and earlier

See `git log` for prior history.
