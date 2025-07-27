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
use std::sync::RwLock;
use once_cell::sync::Lazy;

/// Collision detection strategy enumeration.
///
/// Determines which collision detection method to use for ray casting.
/// Different strategies offer trade-offs between precision, performance,
/// and memory usage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollisionMode {
    /// Use pixel-perfect collision detection with bitmap storage
    Pixel,
    /// Use tile-based collision detection with structured world data  
    Tile,
    /// Automatically choose best method based on scenario
    Auto,
}

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

    /// Get the collision mode identifier for this detector.
    fn mode(&self) -> CollisionMode;

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
        }

        false
    }

    fn mode(&self) -> CollisionMode {
        CollisionMode::Pixel
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

    fn mode(&self) -> CollisionMode {
        CollisionMode::Tile
    }

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

/// Global collision detection system state.
///
/// Provides thread-safe access to the active collision detector and
/// mode switching capabilities.
pub struct CollisionSystem {
    detector: Box<dyn CollisionDetector>,
    mode: CollisionMode,
}

impl CollisionSystem {
    /// Create a new collision system with the specified mode.
    fn new(mode: CollisionMode) -> Self {
        let detector: Box<dyn CollisionDetector> = match mode {
            CollisionMode::Pixel => Box::new(PixelCollisionMap::new(180, 180)), // Default to demo size
            CollisionMode::Tile => Box::new(TileCollisionMap::new()),
            CollisionMode::Auto => Box::new(TileCollisionMap::new()), // Default to tile for now
        };

        Self { detector, mode }
    }

    /// Switch collision detection mode.
    pub fn set_mode(&mut self, mode: CollisionMode) {
        if mode != self.mode {
            self.detector = match mode {
                CollisionMode::Pixel => Box::new(PixelCollisionMap::new(180, 180)),
                CollisionMode::Tile => Box::new(TileCollisionMap::new()),
                CollisionMode::Auto => Box::new(TileCollisionMap::new()),
            };
            self.mode = mode;
        }
    }

    /// Get the current collision detector.
    pub fn detector(&self) -> &dyn CollisionDetector {
        self.detector.as_ref()
    }

    /// Get mutable access to the current collision detector.
    pub fn detector_mut(&mut self) -> &mut dyn CollisionDetector {
        self.detector.as_mut()
    }

    /// Get the current collision mode.
    pub fn mode(&self) -> CollisionMode {
        self.mode
    }
}

/// Global collision system instance.
///
/// Thread-safe access to the collision detection system used by the
/// lighting engine for ray casting collision checks.
pub static COLLISION_SYSTEM: Lazy<RwLock<CollisionSystem>> = 
    Lazy::new(|| RwLock::new(CollisionSystem::new(CollisionMode::Tile)));

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

/// Set the collision detection mode.
///
/// # Arguments
/// * `mode` - The collision detection mode to use
pub fn set_collision_mode(mode: CollisionMode) {
    if let Ok(mut system) = COLLISION_SYSTEM.write() {
        system.set_mode(mode);
    }
}

/// Get the current collision detection mode.
pub fn get_collision_mode() -> CollisionMode {
    if let Ok(system) = COLLISION_SYSTEM.read() {
        system.mode()
    } else {
        CollisionMode::Tile // Default fallback
    }
}

/// Clear all collision data.
pub fn clear_collisions() {
    if let Ok(mut system) = COLLISION_SYSTEM.write() {
        system.detector_mut().clear();
    }
}

/// Set a single pixel in the collision map if in pixel mode.
///
/// # Arguments
/// * `x`, `y` - Pixel coordinates
/// * `blocked` - Whether the pixel should block light rays
///
/// # Returns
/// `true` if the pixel was set successfully, `false` if not in pixel mode
pub fn set_pixel(x: u16, y: u16, blocked: bool) -> bool {
    if let Ok(mut system) = COLLISION_SYSTEM.write() {
        if let Some(pixel_map) = system.detector_mut().as_any_mut().downcast_mut::<PixelCollisionMap>() {
            pixel_map.set_pixel(x, y, blocked);
            return true;
        }
    }
    false
}

/// Set multiple pixels in batch if in pixel mode.
///
/// # Arguments
/// * `pixels` - Iterator of (x, y, blocked) tuples
///
/// # Returns
/// `true` if the pixels were set successfully, `false` if not in pixel mode
pub fn set_pixel_batch<I>(pixels: I) -> bool 
where
    I: IntoIterator<Item = (u16, u16, bool)>,
{
    if let Ok(mut system) = COLLISION_SYSTEM.write() {
        if let Some(pixel_map) = system.detector_mut().as_any_mut().downcast_mut::<PixelCollisionMap>() {
            pixel_map.set_pixel_batch(pixels);
            return true;
        }
    }
    false
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
    fn test_collision_mode_switching() {
        set_collision_mode(CollisionMode::Pixel);
        assert_eq!(get_collision_mode(), CollisionMode::Pixel);
        
        set_collision_mode(CollisionMode::Tile);
        assert_eq!(get_collision_mode(), CollisionMode::Tile);
    }

    #[test]
    fn test_collision_system_basic() {
        // Test that the system doesn't panic with basic usage
        clear_collisions();
        let _blocked = is_blocked(0, 0, 10, 10);
    }
} 