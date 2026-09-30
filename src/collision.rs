//! Unified Wall + Object collision detection for the lighting engine.
//!
//! Per [ADR-0006](../../docs/decisions/0006-unify-collision-detection.md), every
//! `is_blocked` query runs two phases on the same [`HybridCollisionMap`]:
//!
//! 1. **Broad phase** — [`crate::map_grid::UnionFind`] rejects rays whose
//!    endpoints lie in different rooms (i.e. a Wall lies between them).
//! 2. **Narrow phase** — bitmap walk through the cell-level [`PixelCollisionMap`]
//!    catches rays that hit an Object.
//!
//! Free functions in this module are back-compat shims operating on
//! [`crate::engine::DEFAULT_ENGINE`]; new Rust callers should construct a
//! [`crate::engine::LightingEngine`] and call methods on it.

use std::collections::HashSet;
use std::sync::{Arc, RwLock};

use crate::engine::{canonical_edge, DEFAULT_ENGINE};
use crate::map_grid::UnionFind;

/// Unified interface for collision detection backends. Kept as a trait so
/// tests can substitute alternative implementations if needed; the live
/// system uses a single [`HybridCollisionMap`].
pub trait CollisionDetector: Send + Sync {
    /// Returns `true` if the segment `(x0,y0)→(x1,y1)` is blocked.
    fn is_blocked(&self, x0: i16, y0: i16, x1: i16, y1: i16) -> bool;

    /// Reset all collision data (implementation-specific).
    fn clear(&mut self);

    fn as_any(&self) -> &dyn std::any::Any;
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

/// Per-cell opacity map used for the narrow-phase Object check (0 clear …
/// 255 opaque). Cells inside the walk origin's tile never occlude it.
///
/// Despite the name, the indices it stores are **cells**, not screen pixels.
/// The name is preserved for WASM/JS back-compat (see `CONTEXT.md`).
pub struct PixelCollisionMap {
    width: u16,
    height: u16,
    cells_per_tile: u16,
    opacity: Vec<u8>,
    /// Count of non-zero cells; 0 = every walk is fully transmissive.
    occupied: usize,
}

/// Below this a ray's transmittance counts as zero (would render as alpha 0).
pub const MIN_TRANSMITTANCE: f32 = 1.0 / 255.0;

impl PixelCollisionMap {
    pub fn new(width: u16, height: u16, cells_per_tile: u16) -> Self {
        let total = (width as usize) * (height as usize);
        Self {
            width,
            height,
            cells_per_tile: cells_per_tile.max(1),
            opacity: vec![0; total],
            occupied: 0,
        }
    }

    fn index(&self, x: u16, y: u16) -> Option<usize> {
        if x >= self.width || y >= self.height {
            return None;
        }
        Some((y as usize) * (self.width as usize) + (x as usize))
    }

    /// Set a cell's opacity (0 clear … 255 opaque). Out-of-bounds is ignored.
    pub fn set_opacity(&mut self, x: u16, y: u16, opacity: u8) {
        let Some(i) = self.index(x, y) else { return };
        let was = self.opacity[i];
        if was == 0 && opacity != 0 {
            self.occupied += 1;
        } else if was != 0 && opacity == 0 {
            self.occupied -= 1;
        }
        self.opacity[i] = opacity;
    }

    /// Cell opacity (0 clear … 255 opaque); out-of-bounds reads as clear.
    pub fn opacity(&self, x: u16, y: u16) -> u8 {
        self.index(x, y).map_or(0, |i| self.opacity[i])
    }

    pub fn set_pixel(&mut self, x: u16, y: u16, blocked: bool) {
        self.set_opacity(x, y, if blocked { 255 } else { 0 });
    }

    /// `true` iff the cell is fully opaque.
    pub fn get_pixel(&self, x: u16, y: u16) -> bool {
        self.opacity(x, y) == 255
    }

    pub fn set_pixel_batch<I>(&mut self, pixels: I)
    where
        I: IntoIterator<Item = (u16, u16, bool)>,
    {
        for (x, y, blocked) in pixels {
            self.set_pixel(x, y, blocked);
        }
    }

    /// Transparency product along the Bresenham walk `(x0,y0)→(x1,y1)`,
    /// skipping cells in the origin's tile: `(before, after)` the end cell.
    pub fn transmittance(&self, x0: i16, y0: i16, x1: i16, y1: i16) -> (f32, f32) {
        if self.occupied == 0 {
            return (1.0, 1.0);
        }
        let cpt = self.cells_per_tile as i16;
        let origin_tile = (x0.div_euclid(cpt), y0.div_euclid(cpt));
        let dx = (x1 - x0).abs();
        let dy = (y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx - dy;

        let mut x = x0;
        let mut y = y0;
        let mut t = 1.0f32;
        let mut step_count = 0;

        loop {
            let at_end = x == x1 && y == y1;
            let before = t;
            if (x.div_euclid(cpt), y.div_euclid(cpt)) != origin_tile
                && x >= 0
                && y >= 0
            {
                let o = self.opacity(x as u16, y as u16);
                if o != 0 {
                    t *= 1.0 - o as f32 / 255.0;
                    if t < MIN_TRANSMITTANCE {
                        t = 0.0;
                    }
                }
            }
            if at_end {
                return (before, t);
            }
            if t == 0.0 {
                return (0.0, 0.0);
            }
            let e2 = 2 * err;
            if e2 > -dy {
                err -= dy;
                x += sx;
            }
            if e2 < dx {
                err += dx;
                y += sy;
            }
            step_count += 1;
            if step_count > 1000 {
                return (t, t);
            }
        }
    }
}

impl CollisionDetector for PixelCollisionMap {
    fn is_blocked(&self, x0: i16, y0: i16, x1: i16, y1: i16) -> bool {
        self.transmittance(x0, y0, x1, y1).1 == 0.0
    }

    fn clear(&mut self) {
        self.opacity.fill(0);
        self.occupied = 0;
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// Combined room-graph (broad phase) + cell-bitmap (narrow phase) detector.
pub struct HybridCollisionMap {
    union_find: Arc<RwLock<UnionFind>>,
    pixel_map: PixelCollisionMap,
    map_size: usize,
    /// Canonical `(lo, hi)` cell-index pairs where the broad-phase walk is
    /// allowed to step between two cells that the union-find considers to be
    /// in different rooms. Populated by the engine from its edge overrides —
    /// a door dissolves the wall only along its own cell-edges, not across
    /// the entire room boundary (which is what a UF union would do).
    pass_cell_edges: HashSet<(usize, usize)>,
    /// Inverse of `pass_cell_edges`: cell-pairs the walk may NOT step across
    /// even within one room (a closed door sealing a same-room edge).
    block_cell_edges: HashSet<(usize, usize)>,
}

impl HybridCollisionMap {
    pub fn new(map_data: Vec<i32>, map_size: usize, cells_per_tile: usize) -> Self {
        let uf = UnionFind::new(map_data, map_size);
        Self {
            union_find: Arc::new(RwLock::new(uf)),
            pixel_map: PixelCollisionMap::new(
                map_size as u16,
                map_size as u16,
                cells_per_tile as u16,
            ),
            map_size,
            pass_cell_edges: HashSet::new(),
            block_cell_edges: HashSet::new(),
        }
    }

    pub fn update_map_data(&mut self, map_data: Vec<i32>, map_size: usize) {
        if let Ok(mut uf) = self.union_find.write() {
            *uf = UnionFind::new(map_data, map_size);
        }
        self.map_size = map_size;
    }

    pub fn pixel_map_mut(&mut self) -> &mut PixelCollisionMap {
        &mut self.pixel_map
    }

    /// Replace both cell-edge override sets (canonical `(lo, hi)` pairs).
    /// `pass`: the broad-phase walk may step across despite a room boundary.
    /// `block`: it may not, despite sharing a room.
    pub fn set_edge_cell_overrides(
        &mut self,
        pass: HashSet<(usize, usize)>,
        block: HashSet<(usize, usize)>,
    ) {
        self.pass_cell_edges = pass;
        self.block_cell_edges = block;
    }

    pub fn pixel_map(&self) -> &PixelCollisionMap {
        &self.pixel_map
    }
}

impl HybridCollisionMap {
    /// Broad phase only: does a Wall (room boundary / sealed edge) cut the
    /// walk `(x0,y0)→(x1,y1)`? Objects are [`Self::transmittance`]'s job.
    pub fn room_blocked(&self, x0: i16, y0: i16, x1: i16, y1: i16) -> bool {
        let size = self.map_size as i32;
        let in_bounds = |x: i32, y: i32| x >= 0 && y >= 0 && x < size && y < size;
        let (x0i, y0i, x1i, y1i) = (x0 as i32, y0 as i32, x1 as i32, y1 as i32);

        if in_bounds(x0i, y0i) && in_bounds(x1i, y1i) {
            if let Ok(mut uf) = self.union_find.write() {
                let dx = x1i - x0i;
                let dy = y1i - y0i;
                let nx = dx.abs();
                let ny = dy.abs();
                let sx = if dx > 0 { 1 } else { -1 };
                let sy = if dy > 0 { 1 } else { -1 };

                let mut px = x0i;
                let mut py = y0i;
                let mut ix = 0;
                let mut iy = 0;
                let mut current_idx = (py * size + px) as usize;
                let mut current_room = uf.find(current_idx);

                while ix < nx || iy < ny {
                    let prev_idx = current_idx;
                    if (ix as f32 + 0.5) / (nx as f32) < (iy as f32 + 0.5) / (ny as f32) {
                        px += sx;
                        ix += 1;
                    } else {
                        py += sy;
                        iy += 1;
                    }
                    if !in_bounds(px, py) {
                        return true;
                    }
                    let next_idx = (py * size + px) as usize;
                    let next_room = uf.find(next_idx);
                    if next_room != current_room {
                        if !self
                            .pass_cell_edges
                            .contains(&canonical_edge(prev_idx, next_idx))
                        {
                            return true;
                        }
                    } else if !self.block_cell_edges.is_empty()
                        && self
                            .block_cell_edges
                            .contains(&canonical_edge(prev_idx, next_idx))
                    {
                        return true;
                    }
                    current_idx = next_idx;
                    current_room = next_room;
                }
            }
        }
        false
    }

    /// Narrow phase: Object transmittance `(before, after)` the end cell.
    pub fn transmittance(&self, x0: i16, y0: i16, x1: i16, y1: i16) -> (f32, f32) {
        self.pixel_map.transmittance(x0, y0, x1, y1)
    }
}

impl CollisionDetector for HybridCollisionMap {
    fn is_blocked(&self, x0: i16, y0: i16, x1: i16, y1: i16) -> bool {
        self.room_blocked(x0, y0, x1, y1) || self.transmittance(x0, y0, x1, y1).1 == 0.0
    }

    fn clear(&mut self) {
        if let Ok(mut uf) = self.union_find.write() {
            *uf = UnionFind::new(vec![0; self.map_size * self.map_size], self.map_size);
        }
        self.pixel_map.clear();
        self.pass_cell_edges.clear();
        self.block_cell_edges.clear();
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// ------------------------------- shims ----------------------------------

/// WASM/back-compat shim. Forwards to [`crate::engine::DEFAULT_ENGINE`].
pub fn is_blocked(x0: i16, y0: i16, x1: i16, y1: i16) -> bool {
    DEFAULT_ENGINE
        .read()
        .map(|e| e.is_blocked(x0, y0, x1, y1))
        .unwrap_or(false)
}

/// WASM/back-compat shim. Forwards to [`crate::engine::DEFAULT_ENGINE`].
pub fn update_map_data(map_data: Vec<i32>, map_size: usize) {
    if let Ok(mut e) = DEFAULT_ENGINE.write() {
        e.update_map_data(map_data, map_size);
    }
}

/// WASM/back-compat shim. Forwards to [`crate::engine::DEFAULT_ENGINE`].
pub fn clear_collisions() {
    if let Ok(mut e) = DEFAULT_ENGINE.write() {
        e.clear_pixel_collisions();
    }
}

/// WASM/back-compat shim. Forwards to [`crate::engine::DEFAULT_ENGINE`].
pub fn set_pixel(x: u16, y: u16, blocked: bool) -> bool {
    if let Ok(mut e) = DEFAULT_ENGINE.write() {
        e.set_pixel(x, y, blocked);
        true
    } else {
        false
    }
}

/// WASM/back-compat shim. Forwards to [`crate::engine::DEFAULT_ENGINE`].
pub fn set_pixel_batch<I>(pixels: I) -> bool
where
    I: IntoIterator<Item = (u16, u16, bool)>,
{
    if let Ok(mut e) = DEFAULT_ENGINE.write() {
        e.set_pixel_batch(pixels);
        true
    } else {
        false
    }
}

/// Force initialization of the default engine. Cheap; idempotent.
pub fn init() {
    once_cell::sync::Lazy::force(&DEFAULT_ENGINE);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pixel_collision_map_basic() {
        let mut map = PixelCollisionMap::new(10, 10, 1);
        assert!(!map.get_pixel(5, 5));
        map.set_pixel(5, 5, true);
        assert!(map.get_pixel(5, 5));
        map.set_pixel(5, 5, false);
        assert!(!map.get_pixel(5, 5));
    }

    #[test]
    fn test_pixel_collision_map_line_blocking() {
        let mut map = PixelCollisionMap::new(10, 10, 1);
        map.set_pixel(5, 5, true);
        assert!(map.is_blocked(0, 5, 9, 5));
        assert!(!map.is_blocked(0, 0, 9, 0));
    }

    #[test]
    fn transmittance_skips_origin_tile_and_deposits_before_end() {
        let mut map = PixelCollisionMap::new(12, 12, 4);
        map.set_opacity(1, 1, 255); // origin tile (0,0)
        map.set_opacity(6, 1, 128); // tile (1,0)
        assert_eq!(map.transmittance(0, 1, 5, 1), (1.0, 1.0), "origin tile skipped");
        let (before, after) = map.transmittance(0, 1, 6, 1);
        assert_eq!(before, 1.0);
        assert!((after - 127.0 / 255.0).abs() < 1e-6);
        assert!(map.is_blocked(6, 1, 1, 1) && !map.is_blocked(0, 1, 11, 1));
    }

    #[test]
    fn test_pixel_collision_map_batch_operations() {
        let mut map = PixelCollisionMap::new(10, 10, 1);
        let pixels = vec![(1, 1, true), (2, 2, true), (3, 3, true)];
        map.set_pixel_batch(pixels);
        assert!(map.get_pixel(1, 1));
        assert!(map.get_pixel(2, 2));
        assert!(map.get_pixel(3, 3));
        assert!(!map.get_pixel(4, 4));
    }

    #[test]
    fn test_unified_collision_system() {
        clear_collisions();
        let _blocked = is_blocked(0, 0, 10, 10);
        assert!(set_pixel(5, 5, true));
        assert!(set_pixel(5, 5, false));
    }

    #[test]
    fn test_collision_system_basic() {
        clear_collisions();
        let _blocked = is_blocked(0, 0, 10, 10);
    }
}
