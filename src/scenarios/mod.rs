//! Named, reusable lighting scenarios.
//!
//! A **scenario** is a plain Rust function that takes a fresh
//! [`crate::engine::LightingEngine`], populates it with tiles, objects, and
//! lights, and returns the id of the primary light to inspect. Scenarios are
//! the shared definition consumed by:
//!
//! - the **exploration loop** — `examples/scenario.rs`, which builds the
//!   scenario into a fresh engine and prints the ASCII matrix (and/or PNG)
//!   so a developer or agent can see what the engine produced.
//! - the **regression loop** — `tests/scenarios.rs`, which builds the same
//!   scenario in each test, asserts invariants on the canvas, and panics
//!   with the ASCII matrix embedded if anything fails.
//!
//! # Stack size
//!
//! Tests that build a scenario need a larger thread stack than the default,
//! because the precomputed `ALL_RAYS` lookup table touches a lot of memory.
//! `.cargo/config.toml` sets `RUST_MIN_STACK=8388608` so `cargo test` works
//! out of the box; set the same env var manually if invoking the test
//! binary directly.
//!
//! # Adding a scenario
//!
//! 1. Write a function `pub fn my_scenario(e: &mut LightingEngine) -> u8`.
//! 2. Add a [`Scenario`] entry to [`SCENARIOS`].
//! 3. Run `cargo run --example scenario -- --name my_scenario` to eyeball it.
//! 4. Add a regression test in `tests/scenarios.rs`.

use crate::engine::LightingEngine;

/// A named scenario the exploration and regression loops can reference.
pub struct Scenario {
    /// Identifier passed on the CLI (`--name <name>`) and used in test code.
    pub name: &'static str,
    /// Human-readable one-liner. Surfaced by `--list`.
    pub description: &'static str,
    /// Builds the scenario into the given engine and returns the id of the
    /// primary light to inspect.
    pub build: fn(&mut LightingEngine) -> u8,
}

/// All scenarios known to the exploration and regression loops.
pub const SCENARIOS: &[Scenario] = &[
    Scenario {
        name: "single_light",
        description: "One rainbow light at the centre of an empty world.",
        build: single_light,
    },
    Scenario {
        name: "object_shadow",
        description: "One light with a blocking object cell to its east, casting a shadow.",
        build: object_shadow,
    },
    Scenario {
        name: "object_wall",
        description: "One light fully enclosed by a ring of object cells.",
        build: object_wall,
    },
    Scenario {
        name: "tile_wall_shadow",
        description: "Two rooms split by tile-authored wall; light on one side should not leak to the other.",
        build: tile_wall_shadow,
    },
];

/// Look up a scenario by name.
pub fn find(name: &str) -> Option<&'static Scenario> {
    SCENARIOS.iter().find(|s| s.name == name)
}

// ----- scenario definitions ---------------------------------------------

/// Single rainbow light, no walls, no objects.
pub fn single_light(engine: &mut LightingEngine) -> u8 {
    engine.update_or_add_light(1, 5, 90, 90);
    1
}

/// One light with a vertical line of object cells to the east — should
/// cast a shadow on the east side of the canvas.
pub fn object_shadow(engine: &mut LightingEngine) -> u8 {
    let (cx, cy) = (90i16, 90i16);
    // A short vertical wall of object cells, 2 cells east of the light.
    for dy in -3..=3 {
        engine.set_pixel((cx + 2) as u16, (cy + dy) as u16, true);
    }
    engine.update_or_add_light(1, 5, cx, cy);
    1
}

/// Two rooms separated by a column of wall *tiles* (not object cells). A
/// light placed in the western room should not leak into the eastern room.
///
/// This is the regression test for issue #67: walls authored via the
/// tile-map API must occlude light, not just `set_pixel` Objects.
pub fn tile_wall_shadow(engine: &mut LightingEngine) -> u8 {
    let tpr = engine.tiles_per_row();
    let cpt = engine.cells_per_tile();
    // West half = room "1"; east half = room "2"; the boundary between them
    // is a wall between two non-equal-type tiles (room-graph edge absent).
    let mut tiles = vec![0u8; tpr * tpr];
    for ty in 0..tpr {
        for tx in 0..tpr {
            tiles[ty * tpr + tx] = if tx < tpr / 2 { 1 } else { 2 };
        }
    }
    engine.set_tile_map(tiles);
    // Light pressed up against the *east* edge of the west room — the
    // tile-boundary then sits exactly one cell east of the light, so the
    // entire east half of the rendered canvas is on the far side of the wall.
    let boundary_tx = tpr / 2;
    let light_cx = (boundary_tx * cpt) as i16 - 1;
    let light_cy = ((tpr / 2) * cpt + cpt / 2) as i16;
    engine.update_or_add_light(1, 5, light_cx, light_cy);
    1
}

/// Light surrounded on all four sides by object cells at distance 2.
pub fn object_wall(engine: &mut LightingEngine) -> u8 {
    let (cx, cy) = (90i16, 90i16);
    let r = 2i16;
    for d in -r..=r {
        engine.set_pixel((cx + d) as u16, (cy - r) as u16, true);
        engine.set_pixel((cx + d) as u16, (cy + r) as u16, true);
        engine.set_pixel((cx - r) as u16, (cy + d) as u16, true);
        engine.set_pixel((cx + r) as u16, (cy + d) as u16, true);
    }
    engine.update_or_add_light(1, 5, cx, cy);
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_scenario_builds_without_panic() {
        for s in SCENARIOS {
            let mut e = LightingEngine::default();
            let id = (s.build)(&mut e);
            assert!(
                e.light_canvas(id).is_some(),
                "scenario {:?} did not produce light {}",
                s.name,
                id
            );
        }
    }

    #[test]
    fn find_returns_matching_scenario() {
        assert!(find("single_light").is_some());
        assert!(find("does_not_exist").is_none());
    }

    #[test]
    fn scenario_names_are_unique() {
        let mut names: Vec<&str> = SCENARIOS.iter().map(|s| s.name).collect();
        let count = names.len();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), count, "duplicate scenario names");
    }
}
