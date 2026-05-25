//! [`LightingEngine`] — owned instance of all per-scenario mutable state.
//!
//! Per [ADR-0007](../../docs/decisions/0007-extract-lighting-engine-type.md), all
//! mutable runtime state used to live in process-wide statics. That made parallel
//! tests racy and prevented embedders from holding more than one scene at a time.
//!
//! A [`LightingEngine`] owns:
//!
//! - the tile map (`Vec<u8>`)
//! - the derived cell block map (`Vec<CellDetails>`)
//! - the collision system ([`HybridCollisionMap`] — rooms + objects)
//! - the registry of active [`Light`]s
//!
//! Process-wide caches that are pure functions of compile-time constants — most
//! notably the precomputed Bresenham ray table `ALL_RAYS` in [`crate::lighting`]
//! — stay shared across all engines.
//!
//! # Back-compat shim
//!
//! A default singleton [`DEFAULT_ENGINE`] backs every `#[wasm_bindgen]` free
//! function exposed to JavaScript. New Rust callers (tests, examples, embedders)
//! should construct their own instance with [`LightingEngine::new`] and call
//! methods on it directly — that's what makes parallel test execution safe.

use std::collections::{HashMap, HashSet};
use std::sync::RwLock;

use once_cell::sync::Lazy;

use crate::block_map::{compute_cell_details_for_tile, CellDetails};
use crate::collision::HybridCollisionMap;
use crate::lighting::{build_ray_table, Color, ColorMode, Light, RayTable};

/// Default cell-grid subdivision per tile, used by [`LightingEngine::default`]
/// and the WASM back-compat [`DEFAULT_ENGINE`].
pub const DEFAULT_CELLS_PER_TILE: usize = 6;

/// Default tile-grid side length, used by [`LightingEngine::default`] and the
/// WASM back-compat [`DEFAULT_ENGINE`].
pub const DEFAULT_TILES_PER_ROW: usize = 30;

/// Owned instance of the lighting engine's mutable runtime state.
///
/// Construct one per scenario. Multiple instances coexist freely — they share
/// only immutable, process-wide ray geometry caches.
pub struct LightingEngine {
    cells_per_tile: usize,
    tiles_per_row: usize,
    max_dist: usize,
    all_rays: RayTable,
    tiles: Vec<u8>,
    cells: Vec<CellDetails>,
    collision: HybridCollisionMap,
    lights: HashMap<u8, Light>,
    /// Door edges recorded via [`LightingEngine::set_door_edge`]. Each entry is
    /// a canonical `(lo, hi)` pair of tile indices. Currently inert — placeholder
    /// for issue #67 follow-up PR that wires doors into the room graph.
    door_edges: HashSet<(usize, usize)>,
}

impl Default for LightingEngine {
    fn default() -> Self {
        Self::new(DEFAULT_CELLS_PER_TILE, DEFAULT_TILES_PER_ROW)
    }
}

impl LightingEngine {
    /// Construct an engine with empty tile map (all tiles type 0, one big
    /// room, no objects, no lights) at the given resolution.
    ///
    /// `cells_per_tile` is the cell-grid subdivision per tile (collision /
    /// lighting resolution); `tiles_per_row` is the world's tile-grid side.
    pub fn new(cells_per_tile: usize, tiles_per_row: usize) -> Self {
        assert!(cells_per_tile > 0, "cells_per_tile must be > 0");
        assert!(tiles_per_row > 0, "tiles_per_row must be > 0");
        let tiles_total = tiles_per_row * tiles_per_row;
        let cells_per_row = cells_per_tile * tiles_per_row;
        let cells_total = cells_per_row * cells_per_row;
        let tiles = vec![0u8; tiles_total];
        let cells = vec![CellDetails::default(); cells_total];
        // Default: one big room covering all cells, no walls, no objects.
        let map_data = vec![1i32; cells_per_row * cells_per_row];
        let collision = HybridCollisionMap::new(map_data, cells_per_row);
        let max_dist = crate::lighting::max_dist();
        let all_rays = build_ray_table(max_dist);
        Self {
            cells_per_tile,
            tiles_per_row,
            max_dist,
            all_rays,
            tiles,
            cells,
            collision,
            lights: HashMap::new(),
            door_edges: HashSet::new(),
        }
    }

    /// Cell-grid subdivision per tile (was the module-level `CELLS_PER_TILE`
    /// constant; now per-instance per ADR-0008).
    pub fn cells_per_tile(&self) -> usize {
        self.cells_per_tile
    }

    /// World's tile-grid side length (was the module-level `TILES_PER_ROW`).
    pub fn tiles_per_row(&self) -> usize {
        self.tiles_per_row
    }

    /// Cell-grid side length (`cells_per_tile * tiles_per_row`).
    pub fn cells_per_row(&self) -> usize {
        self.cells_per_tile * self.tiles_per_row
    }

    /// Read-only view of the tile-type array (row-major, `TILES_TOTAL` long).
    pub fn tiles(&self) -> &[u8] {
        &self.tiles
    }

    /// Read-only view of the derived block-map cell array
    /// (row-major, `CELLS_TOTAL` long).
    pub fn block_map(&self) -> &[CellDetails] {
        &self.cells
    }

    /// Reference to the underlying collision system. Useful for diagnostics.
    pub fn collision(&self) -> &HybridCollisionMap {
        &self.collision
    }

    /// Check whether the segment `(x0,y0)→(x1,y1)` (cell coords) is blocked by
    /// a wall or object.
    pub fn is_blocked(&self, x0: i16, y0: i16, x1: i16, y1: i16) -> bool {
        use crate::collision::CollisionDetector;
        self.collision.is_blocked(x0, y0, x1, y1)
    }

    /// Set a single tile type. Out-of-range coordinates are ignored.
    /// Recomputes the affected cell edge flags and refreshes the room
    /// union-find from the new tile map.
    pub fn set_tile(&mut self, x: u32, y: u32, tile: u8) {
        let tiles_per_row = self.tiles_per_row;
        let index = (x as usize) + (y as usize * tiles_per_row);
        if index >= self.tiles.len() {
            return;
        }
        self.tiles[index] = tile;
        self.recompute_block_map();
        self.refresh_collision_from_tiles();
    }

    /// Overwrite the entire tile map. Length must match `tiles_per_row²`;
    /// otherwise the call is a no-op.
    pub fn set_tile_map(&mut self, tiles: Vec<u8>) {
        if tiles.len() != self.tiles.len() {
            return;
        }
        self.tiles = tiles;
        self.recompute_block_map();
        self.refresh_collision_from_tiles();
    }

    /// Replace the room map data of the broad-phase collision detector.
    /// Used by external callers (e.g. WASM) that want to push a precomputed
    /// room layout without going through the tile setter.
    pub fn update_map_data(&mut self, map_data: Vec<i32>, map_size: usize) {
        self.collision.update_map_data(map_data, map_size);
    }

    /// Mark a single cell as blocking (an Object cell) or not.
    pub fn set_pixel(&mut self, x: u16, y: u16, blocked: bool) {
        self.collision.pixel_map_mut().set_pixel(x, y, blocked);
    }

    /// Batched form of [`set_pixel`].
    pub fn set_pixel_batch<I>(&mut self, pixels: I)
    where
        I: IntoIterator<Item = (u16, u16, bool)>,
    {
        self.collision.pixel_map_mut().set_pixel_batch(pixels);
    }

    /// Record (or remove) a door edge between two tiles.
    ///
    /// **Placeholder per issue #67.** The call is currently inert with respect
    /// to lighting and pathfinding — only the canonical `(min, max)` tile-index
    /// pair is stored in [`door_edges`](Self::door_edges) so the follow-up PR
    /// (doors as first-class room-graph edges) can wire the read side. Calling
    /// it today does *not* affect rendered light, ray occlusion, or room
    /// connectivity.
    ///
    /// `open=true` records the edge; `open=false` removes it. Out-of-range
    /// tile indices are recorded as-is — validation is the caller's problem
    /// until the wiring lands.
    pub fn set_door_edge(&mut self, t1_idx: usize, t2_idx: usize, open: bool) {
        let pair = canonical_edge(t1_idx, t2_idx);
        if open {
            self.door_edges.insert(pair);
        } else {
            self.door_edges.remove(&pair);
        }
    }

    /// All currently-open door edges as canonical `(lo, hi)` tile-index pairs.
    pub fn door_edges(&self) -> &HashSet<(usize, usize)> {
        &self.door_edges
    }

    /// Whether a door edge between `t1_idx` and `t2_idx` is currently recorded.
    /// Order-insensitive.
    pub fn has_door_edge(&self, t1_idx: usize, t2_idx: usize) -> bool {
        self.door_edges.contains(&canonical_edge(t1_idx, t2_idx))
    }

    /// Clear all object cells (does not touch the tile map).
    pub fn clear_pixel_collisions(&mut self) {
        use crate::collision::CollisionDetector;
        self.collision.clear();
    }

    /// Create or update a rainbow light. Returns a pointer to the rendered
    /// canvas (used by the WASM shim). Rust callers should prefer
    /// [`Self::light_canvas`] after this call.
    pub fn update_or_add_light(&mut self, id: u8, r: i16, x: i16, y: i16) -> *const Color {
        self.update_light_with_color_mode(id, r, x, y, None)
    }

    /// Create or update a solid-color light.
    pub fn update_or_add_light_with_solid_color(
        &mut self,
        id: u8,
        r: i16,
        x: i16,
        y: i16,
        hue: u8,
    ) -> *const Color {
        self.update_light_with_color_mode(id, r, x, y, Some(ColorMode::Solid(hue)))
    }

    /// Create or update a custom-HSV light.
    pub fn update_or_add_light_with_custom_color(
        &mut self,
        id: u8,
        r: i16,
        x: i16,
        y: i16,
        hue: u8,
        saturation: u8,
    ) -> *const Color {
        self.update_light_with_color_mode(
            id,
            r,
            x,
            y,
            Some(ColorMode::Custom { hue, saturation }),
        )
    }

    /// Borrow a light's canvas if one with the given id exists.
    pub fn light_canvas(&self, id: u8) -> Option<&[Color]> {
        self.lights.get(&id).map(|l| l.canvas())
    }

    /// Side length (in cells) of a light's square canvas.
    pub fn light_canvas_size(&self, id: u8) -> Option<usize> {
        self.lights.get(&id).map(|l| l.canvas_size())
    }

    /// Light position in cell coords.
    pub fn light_position(&self, id: u8) -> Option<(i16, i16)> {
        self.lights.get(&id).map(|l| l.pos())
    }

    /// Light radius in cells.
    pub fn light_radius(&self, id: u8) -> Option<i16> {
        self.lights.get(&id).map(|l| l.radius())
    }

    fn update_light_with_color_mode(
        &mut self,
        id: u8,
        r: i16,
        x: i16,
        y: i16,
        color_mode: Option<ColorMode>,
    ) -> *const Color {
        let clamped_r = r.min(self.max_dist as i16).max(1);

        let needs_new = match self.lights.get(&id) {
            Some(existing) => existing.radius() != clamped_r || existing.color_mode() != &color_mode,
            None => true,
        };
        if needs_new {
            self.lights
                .insert(id, Light::new((x, y), clamped_r, color_mode.clone()));
        }

        // Disjoint borrows: `lights` mutably, `collision`+`all_rays` immutably.
        let collision = &self.collision;
        let all_rays = &self.all_rays;
        let max_dist = self.max_dist;
        let light = self
            .lights
            .get_mut(&id)
            .expect("just inserted or known to exist");
        light.set_state((x, y), clamped_r, color_mode);
        light.update(collision, all_rays, max_dist)
    }

    fn recompute_block_map(&mut self) {
        let tiles_total = self.tiles.len();
        for tile_index in 0..tiles_total {
            compute_cell_details_for_tile(
                tile_index,
                &self.tiles,
                &mut self.cells,
                self.cells_per_tile,
                self.tiles_per_row,
            );
        }
    }

    fn refresh_collision_from_tiles(&mut self) {
        // The broad-phase room graph is queried in **cell coordinates** by
        // [`crate::lighting::Light::update`] (ray walks are at cell resolution),
        // so the union-find must be built at cell resolution as well. Expand
        // the tile map: every cell inherits the type of its containing tile.
        // Two cells share a room iff their tiles share a type AND are
        // 4-connected via same-type tiles — the union-find's adjacency union
        // takes care of that automatically.
        let cells_per_tile = self.cells_per_tile;
        let tiles_per_row = self.tiles_per_row;
        let cells_per_row = cells_per_tile * tiles_per_row;
        let mut cell_map = vec![0i32; cells_per_row * cells_per_row];
        for tile_y in 0..tiles_per_row {
            for tile_x in 0..tiles_per_row {
                let tile = self.tiles[tile_y * tiles_per_row + tile_x] as i32;
                let cy0 = tile_y * cells_per_tile;
                let cx0 = tile_x * cells_per_tile;
                for dy in 0..cells_per_tile {
                    let row = (cy0 + dy) * cells_per_row;
                    for dx in 0..cells_per_tile {
                        cell_map[row + cx0 + dx] = tile;
                    }
                }
            }
        }
        self.collision.update_map_data(cell_map, cells_per_row);
    }
}

/// Canonicalise an unordered tile-index pair so `(a, b)` and `(b, a)` map
/// to the same `HashSet` entry.
fn canonical_edge(a: usize, b: usize) -> (usize, usize) {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}

/// Default process-wide engine instance, used as a back-compat shim for the
/// WASM/JS API. Rust callers should prefer constructing their own
/// [`LightingEngine`] via [`LightingEngine::new`].
pub static DEFAULT_ENGINE: Lazy<RwLock<LightingEngine>> =
    Lazy::new(|| RwLock::new(LightingEngine::default()));

/// Gradient used by [`LightingEngine::render_canvas_text`], from darkest to
/// brightest. Ten characters → integer division by 255 maps cleanly.
const ASCII_GRADIENT: &[u8; 10] = b" .:-=+*#%@";

impl LightingEngine {
    /// Render a light's canvas as a multiline ASCII matrix.
    ///
    /// One character per cell, one line per row, trailing `\n` after each
    /// row. Returns `None` if no light with the given id exists.
    ///
    /// This is the agent-readable debug primitive used by the exploration
    /// loop (printed unconditionally) and the regression loop (embedded in
    /// panic messages on assertion failure) so a failing test produces
    /// self-explanatory output.
    pub fn render_canvas_text(&self, id: u8) -> Option<String> {
        let light = self.lights.get(&id)?;
        let canvas = light.canvas();
        let size = light.canvas_size();
        let mut out = String::with_capacity((size + 1) * size);
        for row in 0..size {
            for col in 0..size {
                let pixel = canvas[row * size + col];
                let brightness = pixel.0.max(pixel.1).max(pixel.2);
                let idx = (brightness as usize * (ASCII_GRADIENT.len() - 1)) / 255;
                out.push(ASCII_GRADIENT[idx] as char);
            }
            out.push('\n');
        }
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_canvas_text_is_none_for_missing_light() {
        let engine = LightingEngine::new(6, 30);
        assert!(engine.render_canvas_text(0).is_none());
    }

    #[test]
    fn render_canvas_text_dimensions_match_canvas() {
        let mut engine = LightingEngine::new(6, 30);
        engine.update_or_add_light(1, 3, 5, 5);
        let s = engine.render_canvas_text(1).expect("light exists");
        let lines: Vec<&str> = s.lines().collect();
        assert_eq!(lines.len(), 7, "radius-3 light renders 7 rows, got: {}\n{}", lines.len(), s);
        for (i, line) in lines.iter().enumerate() {
            assert_eq!(
                line.chars().count(),
                7,
                "row {} expected 7 cols, got {}: {:?}",
                i,
                line.chars().count(),
                line
            );
        }
    }

    #[test]
    fn render_canvas_text_center_brighter_than_corner() {
        let mut engine = LightingEngine::new(6, 30);
        engine.update_or_add_light(1, 3, 5, 5);
        let s = engine.render_canvas_text(1).unwrap();
        let lines: Vec<&str> = s.lines().collect();
        let center = lines[3].chars().nth(3).unwrap();
        let corner = lines[0].chars().nth(0).unwrap();
        let g = std::str::from_utf8(ASCII_GRADIENT).unwrap();
        let center_idx = g.find(center).unwrap_or_else(|| panic!("center {:?} not in gradient\n{}", center, s));
        let corner_idx = g.find(corner).unwrap_or_else(|| panic!("corner {:?} not in gradient\n{}", corner, s));
        assert!(
            center_idx >= corner_idx,
            "center {:?} (idx {}) should be at least as bright as corner {:?} (idx {}). Canvas:\n{}",
            center, center_idx, corner, corner_idx, s
        );
    }

    #[test]
    fn independent_engines_do_not_share_lights() {
        let mut a = LightingEngine::new(6, 30);
        let b = LightingEngine::new(6, 30);
        a.update_or_add_light(7, 2, 4, 4);
        assert!(a.light_canvas(7).is_some());
        assert!(b.light_canvas(7).is_none());
    }

    #[test]
    fn set_tile_at_non_default_resolution_marks_cell_edges() {
        // Engine with explicit non-default resolution (4 cells per tile, 5x5 tiles).
        let mut engine = LightingEngine::new(4, 5);
        // Empty world: all tiles type 0 → no internal boundaries.
        // Flip tile (2,2) to type 1, leaving its neighbours type 0.
        engine.set_tile(2, 2, 1);
        let cells = engine.block_map();
        let cells_per_row = engine.cells_per_row();
        // Top-left cell of tile (2,2) sits at cell (8,8). With cells_per_tile=4
        // its edges are at cell rows/cols 8..=11. The north edge should be
        // blocked (different from tile above), as should the west edge.
        let nw = cells[8 * cells_per_row + 8];
        assert!(nw.n_blocked, "NW cell should have north edge blocked");
        assert!(nw.w_blocked, "NW cell should have west edge blocked");
        // Inner cell (9,9) sits inside tile (2,2) — no edges blocked.
        let inner = cells[9 * cells_per_row + 9];
        assert!(!inner.n_blocked && !inner.s_blocked && !inner.e_blocked && !inner.w_blocked);
    }

    #[test]
    fn engine_exposes_resolution() {
        let e = LightingEngine::new(6, 30);
        assert_eq!(e.cells_per_tile(), 6);
        assert_eq!(e.tiles_per_row(), 30);

        let e2 = LightingEngine::new(9, 20);
        assert_eq!(e2.cells_per_tile(), 9);
        assert_eq!(e2.tiles_per_row(), 20);
    }

    #[test]
    fn set_door_edge_records_canonical_pair() {
        let mut e = LightingEngine::default();
        assert!(e.door_edges().is_empty());

        // open=true records the edge; the pair is order-insensitive.
        e.set_door_edge(5, 7, true);
        assert!(e.has_door_edge(5, 7));
        assert!(e.has_door_edge(7, 5));
        assert_eq!(e.door_edges().len(), 1);

        // open=false removes it.
        e.set_door_edge(7, 5, false);
        assert!(!e.has_door_edge(5, 7));
        assert!(e.door_edges().is_empty());
    }

    #[test]
    fn set_door_edge_does_not_affect_lighting_yet() {
        // Placeholder semantics per issue #67: recording the door edge must
        // not change the rendered canvas — the wiring lands in PR #2.
        let mut e = LightingEngine::default();
        e.update_or_add_light(1, 4, 10, 10);
        let before: Vec<u8> = e
            .light_canvas(1)
            .unwrap()
            .iter()
            .map(|c| c.0.max(c.1).max(c.2))
            .collect();

        e.set_door_edge(0, 1, true);
        // Re-render the light (door edge must not change the canvas).
        e.update_or_add_light(1, 4, 10, 10);
        let after: Vec<u8> = e
            .light_canvas(1)
            .unwrap()
            .iter()
            .map(|c| c.0.max(c.1).max(c.2))
            .collect();
        assert_eq!(before, after, "set_door_edge must be a no-op for lighting");
    }
}
