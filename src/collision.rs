//! Collision detection system for ray casting performance optimization.
//!
//! This module provides efficient collision detection for the lighting engine's
//! ray casting system. It replaces the expensive WASM bridge calls with fast
//! native Rust implementations that support both pixel-perfect and tile-based
//! collision detection.
//!
//! # Architecture
//!
//! The collision system is built around the `CollisionDetector` trait, which
//! provides a unified interface for different collision detection strategies:
//!
//! - **Pixel-based**: Fast bitmap collision using Bresenham line algorithms
//! - **Tile-based**: Structured collision using the existing block_map system  
//! - **Hybrid**: Combination approach supporting both methods
//!
//! # Performance
//!
//! This system eliminates the WASM bridge overhead that caused ~250ms light updates.
//! Target performance: <5ms per light update with native Rust collision detection.

use crate::block_map::get_blockmap;
use crate::constants::{CELLS_PER_ROW, CELLS_TOTAL};
use std::sync::{RwLock, Arc};
use once_cell::sync::Lazy;

use crate::map_grid::UnionFind;



// Collision detection is now unified around the hybrid pixel + room system
// No mode selection is needed - the system adapts based on room configuration

/// Core trait for collision detection implementations.
///
/// This trait provides a unified interface for different collision detection
/// strategies, allowing the lighting engine to switch between pixel-perfect
/// and tile-based detection without changing the core ray casting logic.
pub trait CollisionDetector: Send + Sync {
    /// Check if a line segment between two points is blocked by obstacles.
    ///
    /// Uses efficient line-walking algorithms to detect collisions along
    /// the entire ray path from (x0, y0) to (x1, y1).
    ///
    /// # Arguments
    /// * `x0`, `y0` - Starting point of the ray segment
    /// * `x1`, `y1` - Ending point of the ray segment  
    ///
    /// # Returns
    /// `true` if the ray is blocked by any obstacle, `false` if clear
    fn is_blocked(&self, x0: i16, y0: i16, x1: i16, y1: i16) -> bool;



    /// Clear all collision data (implementation-specific behavior).
    fn clear(&mut self);

    /// Get a reference to self as Any for downcasting.
    fn as_any(&self) -> &dyn std::any::Any;

    /// Get a mutable reference to self as Any for downcasting.
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

/// Pixel-based collision detection using efficient bitmap storage.
///
/// This implementation uses bit-packed storage for memory efficiency and
/// provides pixel-perfect collision detection suitable for freeform drawing
/// and complex obstacle shapes.
pub struct PixelCollisionMap {
    /// World dimensions
    width: u16,
    height: u16,
    /// Bit-packed pixel storage (64 pixels per u64 for cache efficiency)
    pixels: Vec<u64>,
}

impl PixelCollisionMap {
    /// Create a new pixel collision map with the specified dimensions.
    ///
    /// # Arguments
    /// * `width` - Width in pixels
    /// * `height` - Height in pixels
    ///
    /// # Returns
    /// New PixelCollisionMap with all pixels initially unblocked
    pub fn new(width: u16, height: u16) -> Self {
        let total_pixels = (width as usize) * (height as usize);
        let storage_size = (total_pixels + 63) / 64; // Round up to u64 boundaries
        
        Self {
            width,
            height,
            pixels: vec![0; storage_size],
        }
    }

    /// Set the blocking state of a single pixel.
    ///
    /// # Arguments
    /// * `x`, `y` - Pixel coordinates
    /// * `blocked` - Whether the pixel should block light rays
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

    /// Get the blocking state of a single pixel.
    ///
    /// # Arguments
    /// * `x`, `y` - Pixel coordinates
    ///
    /// # Returns
    /// `true` if the pixel blocks light, `false` if clear or out of bounds
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

    /// Set multiple pixels in a batch operation for efficiency.
    ///
    /// # Arguments
    /// * `pixels` - Iterator of (x, y, blocked) tuples
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
        // Fast Bresenham line algorithm with early termination on collision
        let dx = (x1 - x0).abs();
        let dy = (y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx - dy;

        let mut x = x0;
        let mut y = y0;
        let mut step_count = 0;

        loop {
            // Check bounds and collision at current position
            if x >= 0 && y >= 0 && (x as u16) < self.width && (y as u16) < self.height {
                if self.get_pixel(x as u16, y as u16) {
                    return true; // Early termination on collision
                }
            }

            // Check if we've reached the destination
            if x == x1 && y == y1 {
                break;
            }

            // Bresenham step
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
            
            // Safety check to prevent infinite loops
            if step_count > 1000 {
                break;
            }
        }

        false
    }

    // Unified collision system - no mode differentiation needed

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

/// Tile-based collision detection using the existing block_map system.
///
/// This implementation leverages the structured tile and cell system for
/// efficient collision detection in grid-based worlds. It provides fast
/// lookups for large structured environments.
pub struct TileCollisionMap;

impl TileCollisionMap {
    /// Create a new tile collision detector.
    ///
    /// This detector uses the global block_map data, so no initialization
    /// parameters are needed.
    pub fn new() -> Self {
        Self
    }
}

/// Hybrid collision detection using UnionFind for broad-phase and PixelCollisionMap for narrow-phase.
pub struct HybridCollisionMap {
    union_find: Arc<RwLock<UnionFind>>,
    pixel_map: PixelCollisionMap,
    map_size: usize,
}

impl HybridCollisionMap {
    /// Create a new hybrid collision map.
    ///
    /// # Arguments
    /// * `map_data` - The initial map data for UnionFind (0 for blocked, >0 for open)
    /// * `map_size` - The size of the square map (e.g., 180 for 180x180)
    pub fn new(map_data: Vec<i32>, map_size: usize) -> Self {
        let uf = UnionFind::new(map_data, map_size);
        Self {
            union_find: Arc::new(RwLock::new(uf)),
            pixel_map: PixelCollisionMap::new(map_size as u16, map_size as u16),
            map_size,
        }
    }

    /// Update the underlying map data for the UnionFind structure.
    pub fn update_map_data(&mut self, map_data: Vec<i32>, map_size: usize) {
        if let Ok(mut uf) = self.union_find.write() {
            *uf = UnionFind::new(map_data, map_size);
        }
    }

    /// Get a mutable reference to the pixel collision map.
    pub fn pixel_map_mut(&mut self) -> &mut PixelCollisionMap {
        &mut self.pixel_map
    }
}

impl CollisionDetector for TileCollisionMap {
    fn is_blocked(&self, x0: i16, y0: i16, x1: i16, y1: i16) -> bool {
        // Use existing block_map collision logic
        // For now, implement a simple Bresenham walk checking cell boundaries
        
        let dx = (x1 - x0).abs();
        let dy = (y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx - dy;

        let mut x = x0;
        let mut y = y0;

        // Get block map data
        let blockmap_ptr = get_blockmap();
        if blockmap_ptr.is_null() {
            return false;
        }

        loop {
            // Check bounds
            if x < 0 || y < 0 || x >= CELLS_PER_ROW as i16 || y >= CELLS_PER_ROW as i16 {
                return false;
            }

            // Check cell blocking using block_map data
            let cell_index = (y as usize) * CELLS_PER_ROW + (x as usize);
            if cell_index < CELLS_TOTAL {
                unsafe {
                    let cells = std::slice::from_raw_parts(blockmap_ptr, CELLS_TOTAL);
                    let cell = &cells[cell_index];
                    
                    // Check if any edge of this cell is blocked
                    if cell.n_blocked || cell.e_blocked || cell.s_blocked || cell.w_blocked {
                        return true;
                    }
                }
            }

            // Check if we've reached the destination
            if x == x1 && y == y1 {
                break;
            }

            // Bresenham step
            let e2 = 2 * err;
            if e2 > -dy {
                err -= dy;
                x += sx;
            }
            if e2 < dx {
                err += dx;
                y += sy;
            }
        }

        false
    }

    // Legacy tile collision - kept for reference but not used

    fn clear(&mut self) {
        // Tile collision map uses global block_map data
        // Clearing would require resetting all tiles, which is handled
        // by the block_map module
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl CollisionDetector for HybridCollisionMap {
    fn is_blocked(&self, x0: i16, y0: i16, x1: i16, y1: i16) -> bool {
        // Broad-phase check with UnionFind
        if let Ok(mut uf) = self.union_find.write() {
            if !uf.cast_ray(x0 as i32, y0 as i32, x1 as i32, y1 as i32) {
                return true; // Ray crosses a room boundary, so it's blocked
            }
        }

        // Narrow-phase check with PixelCollisionMap
        self.pixel_map.is_blocked(x0, y0, x1, y1)
    }

    // Unified hybrid collision system

    fn clear(&mut self) {
        // Clear both UnionFind and PixelCollisionMap
        if let Ok(mut uf) = self.union_find.write() {
            // Reinitialize UnionFind with an empty map or default map
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

/// Global collision detection system state.
///
/// Uses a unified hybrid collision system combining room-based broad-phase
/// and pixel-based narrow-phase collision detection.
pub struct CollisionSystem {
    detector: HybridCollisionMap,
}

impl CollisionSystem {
    /// Create a new collision system with default hybrid detector.
    fn new() -> Self {
        // Start with a simple open area - no rooms configured initially
        let map_size = 180;
        let map_data = vec![1; map_size * map_size]; // All open (room 1)
        let detector = HybridCollisionMap::new(map_data, map_size);

        Self { detector }
    }

    /// Update the map data for room-based collision.
    ///
    /// # Arguments
    /// * `map_data` - Room layout where 0 = wall, >0 = room ID
    /// * `map_size` - Width/height of the square map
    pub fn update_map_data(&mut self, map_data: Vec<i32>, map_size: usize) {
        self.detector.update_map_data(map_data, map_size);
    }

    /// Get the current collision detector.
    pub fn detector(&self) -> &HybridCollisionMap {
        &self.detector
    }

    /// Get mutable access to the current collision detector.
    pub fn detector_mut(&mut self) -> &mut HybridCollisionMap {
        &mut self.detector
    }
}

/// Global collision system instance.
///
/// Thread-safe access to the unified hybrid collision detection system
/// used by the lighting engine for ray casting collision checks.
pub static COLLISION_SYSTEM: Lazy<RwLock<CollisionSystem>> = 
    Lazy::new(|| RwLock::new(CollisionSystem::new()));

/// Check if a line segment is blocked using the active collision detector.
///
/// This is the main entry point for collision detection used by the lighting
/// engine. It uses the currently configured collision detector to perform
/// efficient ray-obstacle intersection testing.
///
/// # Arguments
/// * `x0`, `y0` - Starting point of the ray segment
/// * `x1`, `y1` - Ending point of the ray segment
///
/// # Returns
/// `true` if the ray is blocked by any obstacle, `false` if clear
pub fn is_blocked(x0: i16, y0: i16, x1: i16, y1: i16) -> bool {
    if let Ok(system) = COLLISION_SYSTEM.read() {
        system.detector().is_blocked(x0, y0, x1, y1)
    } else {
        false // Default to unblocked if lock fails
    }
}

/// Update the map data for room-based collision.
///
/// # Arguments
/// * `map_data` - Room layout where 0 = wall, >0 = room ID
/// * `map_size` - Width/height of the square map
pub fn update_map_data(map_data: Vec<i32>, map_size: usize) {
    if let Ok(mut system) = COLLISION_SYSTEM.write() {
        system.update_map_data(map_data, map_size);
    }
}

/// Clear all collision data.
pub fn clear_collisions() {
    if let Ok(mut system) = COLLISION_SYSTEM.write() {
        system.detector_mut().clear();
    }
}

/// Set a single pixel in the collision map.
///
/// # Arguments
/// * `x`, `y` - Pixel coordinates
/// * `blocked` - Whether the pixel should block light rays
///
/// # Returns
/// `true` if the pixel was set successfully
pub fn set_pixel(x: u16, y: u16, blocked: bool) -> bool {
    if let Ok(mut system) = COLLISION_SYSTEM.write() {
        system.detector_mut().pixel_map_mut().set_pixel(x, y, blocked);
        true
    } else {
        false
    }
}

/// Set multiple pixels in batch.
///
/// # Arguments
/// * `pixels` - Iterator of (x, y, blocked) tuples
///
/// # Returns
/// `true` if the pixels were set successfully
pub fn set_pixel_batch<I>(pixels: I) -> bool 
where
    I: IntoIterator<Item = (u16, u16, bool)>,
{
    if let Ok(mut system) = COLLISION_SYSTEM.write() {
        system.detector_mut().pixel_map_mut().set_pixel_batch(pixels);
        true
    } else {
        false
    }
}

/// Initialize the collision detection system.
///
/// This function should be called during engine startup to ensure the
/// collision system is properly initialized.
pub fn init() {
    // Force initialization of the lazy static
    Lazy::force(&COLLISION_SYSTEM);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pixel_collision_map_basic() {
        let mut map = PixelCollisionMap::new(10, 10);
        
        // Initially all pixels should be unblocked
        assert!(!map.get_pixel(5, 5));
        
        // Set a pixel as blocked
        map.set_pixel(5, 5, true);
        assert!(map.get_pixel(5, 5));
        
        // Clear the pixel
        map.set_pixel(5, 5, false);
        assert!(!map.get_pixel(5, 5));
    }

    #[test]
    fn test_pixel_collision_map_line_blocking() {
        let mut map = PixelCollisionMap::new(10, 10);
        
        // Place a blocking pixel in the middle of a line
        map.set_pixel(5, 5, true);
        
        // Line that passes through the blocking pixel should be blocked
        assert!(map.is_blocked(0, 5, 9, 5));
        
        // Line that doesn't pass through should be clear
        assert!(!map.is_blocked(0, 0, 9, 0));
    }

    #[test]
    fn test_pixel_collision_map_batch_operations() {
        let mut map = PixelCollisionMap::new(10, 10);
        
        // Set multiple pixels in batch
        let pixels = vec![(1, 1, true), (2, 2, true), (3, 3, true)];
        map.set_pixel_batch(pixels);
        
        assert!(map.get_pixel(1, 1));
        assert!(map.get_pixel(2, 2));
        assert!(map.get_pixel(3, 3));
        assert!(!map.get_pixel(4, 4));
    }

    #[test]
    fn test_unified_collision_system() {
        // Test that the unified system works without mode switching
        clear_collisions();
        let _blocked = is_blocked(0, 0, 10, 10);
        
        // Test pixel setting
        assert!(set_pixel(5, 5, true));
        assert!(set_pixel(5, 5, false));
    }

    #[test]
    fn test_collision_system_basic() {
        // Test that the system doesn't panic with basic usage
        clear_collisions();
        let _blocked = is_blocked(0, 0, 10, 10);
    }
} 