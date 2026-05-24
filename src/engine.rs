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

use std::collections::HashMap;
use std::sync::RwLock;

use once_cell::sync::Lazy;

use crate::block_map::{compute_cell_details_for_tile, CellDetails};
use crate::collision::HybridCollisionMap;
use crate::constants::{CELLS_PER_ROW, CELLS_TOTAL, TILES_PER_ROW, TILES_TOTAL};
use crate::lighting::{Color, ColorMode, Light};

/// Owned instance of the lighting engine's mutable runtime state.
///
/// Construct one per scenario. Multiple instances coexist freely — they share
/// only immutable, process-wide ray geometry caches.
pub struct LightingEngine {
    tiles: Vec<u8>,
    cells: Vec<CellDetails>,
    collision: HybridCollisionMap,
    lights: HashMap<u8, Light>,
}

impl Default for LightingEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl LightingEngine {
    /// Construct an engine with an empty tile map (all tiles type 0, one big
    /// room, no objects, no lights).
    pub fn new() -> Self {
        let tiles = vec![0u8; TILES_TOTAL];
        let cells = vec![CellDetails::default(); CELLS_TOTAL];
        // Default: one big room covering all cells, no walls, no objects.
        let map_data = vec![1i32; CELLS_PER_ROW * CELLS_PER_ROW];
        let collision = HybridCollisionMap::new(map_data, CELLS_PER_ROW);
        Self {
            tiles,
            cells,
            collision,
            lights: HashMap::new(),
        }
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
        let index = (x as usize) + (y as usize * TILES_PER_ROW);
        if index >= TILES_TOTAL {
            return;
        }
        self.tiles[index] = tile;
        self.recompute_block_map();
        self.refresh_collision_from_tiles();
    }

    /// Overwrite the entire tile map. Length must be `TILES_TOTAL`; otherwise
    /// the call is a no-op.
    pub fn set_tile_map(&mut self, tiles: Vec<u8>) {
        if tiles.len() != TILES_TOTAL {
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
        let clamped_r = r.min(crate::lighting::max_dist() as i16).max(1);

        let needs_new = match self.lights.get(&id) {
            Some(existing) => existing.radius() != clamped_r || existing.color_mode() != &color_mode,
            None => true,
        };
        if needs_new {
            self.lights
                .insert(id, Light::new((x, y), clamped_r, color_mode.clone()));
        }

        // Disjoint borrows: `lights` mutably, `collision` immutably.
        let collision = &self.collision;
        let light = self
            .lights
            .get_mut(&id)
            .expect("just inserted or known to exist");
        light.set_state((x, y), clamped_r, color_mode);
        light.update(collision)
    }

    fn recompute_block_map(&mut self) {
        for tile_index in 0..TILES_TOTAL {
            compute_cell_details_for_tile(tile_index, &self.tiles, &mut self.cells);
        }
    }

    fn refresh_collision_from_tiles(&mut self) {
        // Preserve historical behaviour: collision map is updated with the
        // tile-resolution layout (`TILES_PER_ROW`), not the cell resolution.
        let map_data: Vec<i32> = self.tiles.iter().map(|&t| t as i32).collect();
        self.collision.update_map_data(map_data, TILES_PER_ROW);
    }
}

/// Default process-wide engine instance, used as a back-compat shim for the
/// WASM/JS API. Rust callers should prefer constructing their own
/// [`LightingEngine`] via [`LightingEngine::new`].
pub static DEFAULT_ENGINE: Lazy<RwLock<LightingEngine>> =
    Lazy::new(|| RwLock::new(LightingEngine::new()));

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
        let engine = LightingEngine::new();
        assert!(engine.render_canvas_text(0).is_none());
    }

    #[test]
    fn render_canvas_text_dimensions_match_canvas() {
        let mut engine = LightingEngine::new();
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
        let mut engine = LightingEngine::new();
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
        let mut a = LightingEngine::new();
        let b = LightingEngine::new();
        a.update_or_add_light(7, 2, 4, 4);
        assert!(a.light_canvas(7).is_some());
        assert!(b.light_canvas(7).is_none());
    }
}
