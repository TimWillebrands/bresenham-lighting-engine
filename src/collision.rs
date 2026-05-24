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

use std::sync::{Arc, RwLock};

use crate::engine::DEFAULT_ENGINE;
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

/// Cell-bitmap used for the narrow-phase Object check.
///
/// Despite the name, the indices it stores are **cells**, not screen pixels.
/// The name is preserved for WASM/JS back-compat (see `CONTEXT.md`).
pub struct PixelCollisionMap {
    width: u16,
    height: u16,
    pixels: Vec<u64>,
}

impl PixelCollisionMap {
    pub fn new(width: u16, height: u16) -> Self {
        let total = (width as usize) * (height as usize);
        let storage_size = (total + 63) / 64;
        Self {
            width,
            height,
            pixels: vec![0; storage_size],
        }
    }

    pub fn set_pixel(&mut self, x: u16, y: u16, blocked: bool) {
        if x >= self.width || y >= self.height {
            return;
        }
        let pixel_index = (y as usize) * (self.width as usize) + (x as usize);
        let storage_index = pixel_index / 64;
        let bit_offset = pixel_index % 64;
        if storage_index < self.pixels.len() {
            let mask = 1u64 << bit_offset;
            if blocked {
                self.pixels[storage_index] |= mask;
            } else {
                self.pixels[storage_index] &= !mask;
            }
        }
    }

    pub fn get_pixel(&self, x: u16, y: u16) -> bool {
        if x >= self.width || y >= self.height {
            return false;
        }
        let pixel_index = (y as usize) * (self.width as usize) + (x as usize);
        let storage_index = pixel_index / 64;
        let bit_offset = pixel_index % 64;
        if storage_index < self.pixels.len() {
            let mask = 1u64 << bit_offset;
            (self.pixels[storage_index] & mask) != 0
        } else {
            false
        }
    }

    pub fn set_pixel_batch<I>(&mut self, pixels: I)
    where
        I: IntoIterator<Item = (u16, u16, bool)>,
    {
        for (x, y, blocked) in pixels {
            self.set_pixel(x, y, blocked);
        }
    }
}

impl CollisionDetector for PixelCollisionMap {
    fn is_blocked(&self, x0: i16, y0: i16, x1: i16, y1: i16) -> bool {
        let dx = (x1 - x0).abs();
        let dy = (y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx - dy;

        let mut x = x0;
        let mut y = y0;
        let mut step_count = 0;

        loop {
            if x >= 0 && y >= 0 && (x as u16) < self.width && (y as u16) < self.height {
                if self.get_pixel(x as u16, y as u16) {
                    return true;
                }
            }
            if x == x1 && y == y1 {
                break;
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
                break;
            }
        }
        false
    }

    fn clear(&mut self) {
        self.pixels.fill(0);
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
}

impl HybridCollisionMap {
    pub fn new(map_data: Vec<i32>, map_size: usize) -> Self {
        let uf = UnionFind::new(map_data, map_size);
        Self {
            union_find: Arc::new(RwLock::new(uf)),
            pixel_map: PixelCollisionMap::new(map_size as u16, map_size as u16),
            map_size,
        }
    }

    pub fn update_map_data(&mut self, map_data: Vec<i32>, map_size: usize) {
        if let Ok(mut uf) = self.union_find.write() {
            *uf = UnionFind::new(map_data, map_size);
        }
    }

    pub fn pixel_map_mut(&mut self) -> &mut PixelCollisionMap {
        &mut self.pixel_map
    }

    pub fn pixel_map(&self) -> &PixelCollisionMap {
        &self.pixel_map
    }
}

impl CollisionDetector for HybridCollisionMap {
    fn is_blocked(&self, x0: i16, y0: i16, x1: i16, y1: i16) -> bool {
        if let Ok(mut uf) = self.union_find.write() {
            if !uf.cast_ray(x0 as i32, y0 as i32, x1 as i32, y1 as i32) {
                return true;
            }
        }
        self.pixel_map.is_blocked(x0, y0, x1, y1)
    }

    fn clear(&mut self) {
        if let Ok(mut uf) = self.union_find.write() {
            *uf = UnionFind::new(vec![0; self.map_size * self.map_size], self.map_size);
        }
        self.pixel_map.clear();
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
        let mut map = PixelCollisionMap::new(10, 10);
        assert!(!map.get_pixel(5, 5));
        map.set_pixel(5, 5, true);
        assert!(map.get_pixel(5, 5));
        map.set_pixel(5, 5, false);
        assert!(!map.get_pixel(5, 5));
    }

    #[test]
    fn test_pixel_collision_map_line_blocking() {
        let mut map = PixelCollisionMap::new(10, 10);
        map.set_pixel(5, 5, true);
        assert!(map.is_blocked(0, 5, 9, 5));
        assert!(!map.is_blocked(0, 0, 9, 0));
    }

    #[test]
    fn test_pixel_collision_map_batch_operations() {
        let mut map = PixelCollisionMap::new(10, 10);
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
