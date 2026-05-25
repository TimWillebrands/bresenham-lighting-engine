//! Regression loop for the scenarios in [`bresenham_lighting_engine::scenarios`].
//!
//! Each test below constructs its **own** [`LightingEngine`] — the whole point
//! of [ADR-0007](../docs/decisions/0007-extract-lighting-engine-type.md) is
//! that tests can run in parallel without racing on shared globals, so do not
//! use [`bresenham_lighting_engine::engine::DEFAULT_ENGINE`] from here.
//!
//! Assertions describe **invariants** (centre lit, blocked side dark, etc.)
//! rather than golden pixel values. When an invariant fails, the panic
//! message embeds the ASCII canvas matrix so the failure is self-explanatory
//! without re-running the example.
//!
//! # Stack size
//!
//! `.cargo/config.toml` sets `RUST_MIN_STACK=8388608` so `cargo test` works
//! out of the box. If invoking the test binary directly, set the same env
//! var manually.

use bresenham_lighting_engine::engine::LightingEngine;
use bresenham_lighting_engine::lighting::Color;
use bresenham_lighting_engine::scenarios;

/// Per-pixel brightness as a simple max-channel proxy (matches the gradient
/// used by [`LightingEngine::render_canvas_text`]).
fn brightness(pixel: Color) -> u8 {
    pixel.0.max(pixel.1).max(pixel.2)
}

/// Build a scenario by name into a fresh engine. Returns the engine and the
/// id of the primary light.
fn build(name: &str) -> (LightingEngine, u8) {
    let scenario = scenarios::find(name)
        .unwrap_or_else(|| panic!("unknown scenario {:?} — fix the test", name));
    let mut engine = LightingEngine::default();
    let id = (scenario.build)(&mut engine);
    (engine, id)
}

/// Render the canvas of `light_id` as an ASCII matrix for panic messages.
fn matrix(engine: &LightingEngine, light_id: u8) -> String {
    engine
        .render_canvas_text(light_id)
        .unwrap_or_else(|| String::from("<no canvas>"))
}

/// Brightness of the cell at the centre of the light's canvas.
fn center_brightness(engine: &LightingEngine, light_id: u8) -> u8 {
    let size = engine
        .light_canvas_size(light_id)
        .expect("light exists");
    let canvas = engine.light_canvas(light_id).expect("light exists");
    brightness(canvas[(size / 2) * size + size / 2])
}

/// Average brightness of one half of the canvas (split by column).
fn half_brightness(engine: &LightingEngine, light_id: u8, half: Half) -> u32 {
    let size = engine.light_canvas_size(light_id).expect("light exists");
    let canvas = engine.light_canvas(light_id).expect("light exists");
    let (col_start, col_end) = match half {
        Half::West => (0, size / 2),
        Half::East => (size / 2 + 1, size),
    };
    let mut total: u32 = 0;
    let mut count: u32 = 0;
    for row in 0..size {
        for col in col_start..col_end {
            total += brightness(canvas[row * size + col]) as u32;
            count += 1;
        }
    }
    if count == 0 {
        0
    } else {
        total / count
    }
}

#[derive(Copy, Clone)]
enum Half {
    West,
    East,
}

#[test]
fn single_light_centre_is_lit() {
    let (engine, id) = build("single_light");
    let centre = center_brightness(&engine, id);
    assert!(
        centre > 200,
        "expected centre brightness > 200 but got {centre}. Canvas:\n{}",
        matrix(&engine, id)
    );
}

#[test]
fn single_light_is_roughly_symmetric() {
    let (engine, id) = build("single_light");
    let west = half_brightness(&engine, id, Half::West);
    let east = half_brightness(&engine, id, Half::East);
    let diff = west.abs_diff(east);
    assert!(
        diff < 10,
        "empty-world light should be ~symmetric W/E: west={west}, east={east}. Canvas:\n{}",
        matrix(&engine, id)
    );
}

#[test]
fn object_shadow_blocks_east_side() {
    let (engine, id) = build("object_shadow");
    let west = half_brightness(&engine, id, Half::West);
    let east = half_brightness(&engine, id, Half::East);
    assert!(
        west > east,
        "east half should be darker than west: west={west}, east={east}. Canvas:\n{}",
        matrix(&engine, id)
    );
}

#[test]
fn object_wall_dims_canvas_edge() {
    // A light fully ringed by object cells at r=2 should produce a canvas
    // whose outer ring (r > 2) is dark.
    let (engine, id) = build("object_wall");
    let size = engine.light_canvas_size(id).expect("light exists");
    let canvas = engine.light_canvas(id).expect("light exists");

    // Sample the four canvas corners; all should be dark.
    let corners = [
        canvas[0],
        canvas[size - 1],
        canvas[(size - 1) * size],
        canvas[size * size - 1],
    ];
    for (i, &corner) in corners.iter().enumerate() {
        let b = brightness(corner);
        assert!(
            b < 80,
            "corner {i} should be dark but had brightness {b}. Canvas:\n{}",
            matrix(&engine, id)
        );
    }
}

#[test]
fn tile_wall_blocks_light_from_crossing_into_adjacent_room() {
    // Regression test for issue #67: walls authored as tiles via the engine's
    // tile-map API must occlude light. Reproduces the multi-room screenshot.
    let (engine, id) = build("tile_wall_shadow");
    let east = half_brightness(&engine, id, Half::East);
    let west = half_brightness(&engine, id, Half::West);
    assert!(
        east < 30,
        "light placed in west room must not leak east of the wall: \
         east_avg={east} (want <30), west_avg={west}. Canvas:\n{}",
        matrix(&engine, id)
    );
}

#[test]
fn independent_engines_are_isolated() {
    // Build two scenarios in parallel-friendly form: separate engines, no
    // global state in play. If they ever share state, one will see the
    // other's objects.
    let (engine_a, id_a) = build("single_light");
    let (engine_b, id_b) = build("object_shadow");

    let a_east = half_brightness(&engine_a, id_a, Half::East);
    let b_east = half_brightness(&engine_b, id_b, Half::East);

    assert!(
        a_east > b_east,
        "engine_a (no objects) east={a_east} should be brighter than engine_b (object east) east={b_east}.\n\
         engine_a:\n{}\nengine_b:\n{}",
        matrix(&engine_a, id_a),
        matrix(&engine_b, id_b),
    );
}
