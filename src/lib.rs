//! Bresenham Lighting Engine
//!
//! A CPU-based 2D lighting engine that uses Bresenham-style ray casting algorithms
//! to calculate lighting effects without requiring GPU acceleration. This engine
//! is designed to be portable, deterministic, and embeddable across multiple platforms.
//!
//! # Overview
//!
//! The engine works by casting rays from light sources at discrete angles and distances,
//! checking for obstacles along each ray path, and calculating shadows based on
//! geometric projections. Light falloff is applied based on distance, and colors
//! are rendered using HSV color space for smooth transitions.
//!
//! # Key Features
//!
//! - **Zero GPU Dependencies**: Runs entirely on CPU using integer arithmetic
//! - **WebAssembly Ready**: Optimized for WASM deployment in browsers
//! - **Deterministic**: Same inputs always produce identical outputs
//! - **Portable**: Works on any platform with a CPU
//! - **Minimalistic**: Small codebase with no heavy dependencies
//!
//! # Architecture
//!
//! The engine consists of several key modules:
//!
//! - [`lighting`]: Core lighting calculations and ray casting
//! - [`arctan`]: Fast integer trigonometry functions
//! - [`ray`]: Bresenham-style line stepping algorithms
//! - [`block_map`]: World representation and obstacle detection
//! - [`constants`]: Global configuration and world dimensions
//!
//! # Usage
//!
//! ## From WebAssembly
//!
//! ```javascript
//! // Initialize the engine
//! init();
//!
//! // Add a light at position (100, 50) with radius 30
//! const lightCanvas = put(1, 30, 100, 50);
//!
//! // Set up some obstacles
//! set_tile(5, 3, 1);
//! set_tile(6, 3, 1);
//! ```
//!
//! ## From Rust
//!
//! ```rust,no_run
//! use bresenham_lighting_engine::*;
//!
//! // Initialize the lighting system
//! lighting::init();
//!
//! // Create a light source
//! let canvas_ptr = lighting::update_or_add_light(1, 30, 100, 50);
//! ```
//!
//! # Performance Characteristics
//!
//! The engine is optimized for scenarios where:
//! - GPU resources are limited or unavailable
//! - Deterministic behavior is required
//! - Retro/pixel-art aesthetics are desired
//! - Cross-platform compatibility is essential
//!
//! Typical performance: 1-5ms for a single light on modern hardware,
//! scaling roughly linearly with the number of active lights.

use once_cell::sync::Lazy;
use std::sync::RwLock;
use wasm_bindgen::prelude::*;

// Re-export public modules for library use
pub mod arctan;
pub mod block_map;
pub mod constants;
pub mod lighting;
pub mod ray;

/// Function pointer type for obstacle detection
pub type IsBlockedFn = fn(i16, i16, i16, i16) -> bool;

/// Global function pointer for obstacle detection (can be overridden for testing)
static IS_BLOCKED_FN: Lazy<RwLock<IsBlockedFn>> =
    Lazy::new(|| RwLock::new(default_is_blocked_impl));

/// Default implementation that calls the JavaScript function
#[cfg(target_arch = "wasm32")]
fn default_is_blocked_impl(x0: i16, y0: i16, x1: i16, y1: i16) -> bool {
    is_blocked_from_js(x0, y0, x1, y1)
}

/// Default implementation for non-WASM targets (always returns false)
#[cfg(not(target_arch = "wasm32"))]
fn default_is_blocked_impl(_x0: i16, _y0: i16, _x1: i16, _y1: i16) -> bool {
    false
}

/// External JavaScript function for logging debug information.
///
/// This function allows the Rust code to output debug information
/// to the browser's console when running in a web environment.
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log_from_js(s: &str);

    /// External JavaScript function to check if a line segment is blocked.
    ///
    /// This function is implemented on the JavaScript side and provides
    /// the obstacle detection logic for the lighting engine. It takes
    /// two points representing a line segment and returns whether that
    /// segment intersects with any obstacles in the world.
    ///
    /// # Arguments
    /// * `x0`, `y0` - Starting point of the line segment
    /// * `x1`, `y1` - Ending point of the line segment
    ///
    /// # Returns
    /// `true` if the line segment is blocked by an obstacle, `false` otherwise
    #[wasm_bindgen(js_name = IsBlocked)]
    fn is_blocked_from_js(x0: i16, y0: i16, x1: i16, y1: i16) -> bool;
}

/// Export the IsBlocked function for JavaScript use.
///
/// This is a wrapper around the external IsBlocked function to make it
/// available as a WASM export that JavaScript can import and use.
#[wasm_bindgen(js_name = doTheThing)]
pub fn is_blocked(x0: i16, y0: i16, x1: i16, y1: i16) -> bool {
    if let Ok(func) = IS_BLOCKED_FN.read() {
        (*func)(x0, y0, x1, y1)
    } else {
        false
    }
}

/// Set a custom obstacle detection function (useful for testing)
///
/// # Arguments
/// * `func` - Function that takes (x0, y0, x1, y1) and returns true if blocked
///
/// # Example
/// ```rust,no_run
/// use bresenham_lighting_engine::set_is_blocked_fn;
///
/// // Set a custom function for testing
/// set_is_blocked_fn(|x0, y0, x1, y1| {
///     // Custom logic here
///     false
/// });
/// ```
pub fn set_is_blocked_fn(func: IsBlockedFn) {
    if let Ok(mut current_func) = IS_BLOCKED_FN.write() {
        *current_func = func;
    }
}

/// Reset the obstacle detection function to the default implementation
pub fn reset_is_blocked_fn() {
    if let Ok(mut current_func) = IS_BLOCKED_FN.write() {
        *current_func = default_is_blocked_impl;
    }
}

/// Export a logging function for JavaScript use.
///
/// This provides a way for JavaScript to log messages through the Rust
/// WASM module, which can be useful for debugging and development.
#[wasm_bindgen]
pub fn log(message: &str) {
    log_from_js(message);
}

/// Initializes the lighting engine.
///
/// This function must be called before any other lighting operations.
/// It sets up internal data structures and pre-computes ray trajectories
/// for all possible angles and distances.
///
/// # WebAssembly Export
///
/// This function is automatically called when the WASM module starts,
/// but can also be called manually if needed.
///
/// # Performance Note
///
/// Initialization involves computing 60 × 360 ray trajectories, which
/// can take 10-100ms depending on the target platform. Consider calling
/// this during a loading screen or startup phase.
#[wasm_bindgen(start)]
pub fn start() {
    lighting::init();
    block_map::init();
}

/// Updates an existing light or creates a new one.
///
/// This is the primary interface for managing lights in the scene.
/// Each light is identified by a unique ID and can be updated independently.
///
/// # Arguments
/// * `id` - Unique identifier for this light (0-255)
/// * `r` - Light radius/range in world units
/// * `x` - World X coordinate of the light center
/// * `y` - World Y coordinate of the light center
///
/// # Returns
/// A pointer to the light's rendered canvas data (RGBA pixel array).
/// The canvas size is determined by the light's radius and contains
/// pre-rendered lighting information that can be blitted to a framebuffer.
///
/// Returns null pointer if the operation fails (e.g., due to thread contention).
///
/// # Canvas Format
///
/// The returned canvas is a square array of RGBA pixels where:
/// - Size: (radius * 2 + 1)² pixels
/// - Format: 4 bytes per pixel (R, G, B, A)
/// - Origin: Center of the array represents the light's position
/// - Colors: HSV-based with hue determined by ray angle
///
/// # Thread Safety
///
/// This function is thread-safe and can be called concurrently from
/// multiple threads to update different lights simultaneously.
///
/// # Example Usage (JavaScript)
///
/// ```javascript
/// // Create a red light with radius 50 at position (200, 100)
/// const lightCanvas = put(0, 50, 200, 100);
///
/// // Later, move the same light to a new position
/// const updatedCanvas = put(0, 50, 250, 150);
/// ```
#[wasm_bindgen]
pub fn put(id: u8, r: i16, x: i16, y: i16) -> *const lighting::Color {
    lighting::update_or_add_light(id, r, x, y)
}

/// Returns a pointer to the world's tile data array.
///
/// The tile array represents the high-level structure of the world,
/// where each tile can have a different type that affects lighting
/// and obstacle behavior.
///
/// # Returns
/// A pointer to an array of `constants::TILES_TOTAL` bytes, where
/// each byte represents the type ID of one tile in the world.
/// Tiles are stored in row-major order.
///
/// # Array Layout
/// ```text
/// Tile(x,y) = tiles[y * TILES_PER_ROW + x]
/// ```
///
/// # Safety
///
/// The returned pointer is valid for the lifetime of the program.
/// Callers must not write to this memory or access beyond
/// `constants::TILES_TOTAL` elements.
#[wasm_bindgen]
pub fn get_tiles() -> *const u8 {
    block_map::get_tiles()
}

/// Returns a pointer to the world's cell blocking data.
///
/// The cell array provides fine-grained collision information for
/// the lighting engine's ray casting system. Each cell contains
/// information about which edges block light propagation.
///
/// # Returns
/// A pointer to an array of `constants::CELLS_TOTAL` `CellDetails` structures,
/// where each structure contains blocking information for one cell.
/// Cells are stored in row-major order.
///
/// # Array Layout
/// ```text
/// Cell(x,y) = cells[y * CELLS_PER_ROW + x]
/// ```
///
/// # Safety
///
/// The returned pointer is valid for the lifetime of the program.
/// Callers must not write to this memory or access beyond
/// `constants::CELLS_TOTAL` elements.
#[wasm_bindgen]
pub fn get_blockmap() -> *const block_map::CellDetails {
    block_map::get_blockmap()
}

/// Sets the type of a tile at the specified coordinates.
///
/// When a tile's type changes, the system automatically recalculates
/// the blocking information for all cells within that tile and
/// updates the collision detection data used by the lighting engine.
///
/// # Arguments
/// * `x` - X coordinate of the tile (0 to `TILES_PER_ROW - 1`)
/// * `y` - Y coordinate of the tile (0 to `TILES_PER_ROW - 1`)
/// * `tile` - New tile type ID (typically 0 = empty, >0 = various obstacle types)
///
/// # Behavior
///
/// - Coordinates outside the valid range are ignored (no panic)
/// - Setting a tile triggers recalculation of all cell blocking data
/// - The change immediately affects subsequent lighting calculations
///
/// # Performance
///
/// This operation is O(n) where n is the total number of tiles,
/// as it recalculates the entire block map. For frequent updates,
/// consider batching changes or implementing incremental updates.
///
/// # Example Usage (JavaScript)
///
/// ```javascript
/// // Create a wall by setting several tiles to type 1
/// set_tile(10, 5, 1);
/// set_tile(11, 5, 1);
/// set_tile(12, 5, 1);
///
/// // Remove an obstacle by setting it back to empty
/// set_tile(10, 5, 0);
/// ```
#[wasm_bindgen]
pub fn set_tile(x: u32, y: u32, tile: u8) {
    block_map::set_tile(x, y, tile);
}

/// Utility macro for logging debug information to the console.
///
/// This macro provides a convenient way to output debug information
/// when running in a WebAssembly environment. It formats the arguments
/// and calls the JavaScript console.log function.
///
/// # Arguments
///
/// Takes the same arguments as the standard `println!` macro.
///
/// # Example
///
/// ```rust,no_run
/// # use bresenham_lighting_engine::console_log;
/// # use wasm_bindgen::prelude::*;
/// # #[wasm_bindgen]
/// # extern "C" {
/// #     #[wasm_bindgen(js_namespace = console)]
/// #     fn log(s: &str);
/// # }
/// # let id = 1; let x = 10; let y = 20;
/// console_log!("Light {} updated at position ({}, {})", id, x, y);
/// ```
#[macro_export]
macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

// Re-export commonly used types for convenience
pub use block_map::{init as init_block_map, CellDetails};
pub use constants::*;
pub use lighting::{init as init_lighting, Color};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_initialization() {
        // Test that initialization doesn't panic
        start();
    }

    #[test]
    fn test_tile_operations() {
        // Test tile setting with valid coordinates
        set_tile(0, 0, 1);
        set_tile(TILES_PER_ROW as u32 - 1, TILES_PER_ROW as u32 - 1, 2);

        // Test tile setting with invalid coordinates (should not panic)
        set_tile(1000, 1000, 1);
    }

    #[test]
    #[cfg(target_arch = "wasm32")]
    fn test_light_operations() {
        // Skip full initialization in tests to avoid stack overflow
        // just test that the function doesn't panic
        let _canvas_ptr = put(1, 30, 100, 50);
        // Will return null in test environment since lighting isn't initialized
        // and JavaScript is_blocked function is not available, but shouldn't panic
    }

    #[test]
    fn test_data_access() {
        // Initialize block map without lighting (to avoid stack overflow in tests)
        block_map::init();

        // Test that data accessor functions return non-null pointers
        let tiles_ptr = get_tiles();
        let cells_ptr = get_blockmap();

        assert!(!tiles_ptr.is_null());
        assert!(!cells_ptr.is_null());
    }
}
