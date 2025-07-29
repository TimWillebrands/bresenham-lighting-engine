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
//! - **Configurable Colors**: Support for rainbow, solid, and custom HSV colors
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
//! // Add a rainbow light at position (100, 50) with radius 30
//! const rainbowLight = put(1, 30, 100, 50);
//!
//! // Add a red light at position (200, 50) with radius 25
//! const redLight = put_solid_color(2, 25, 200, 50, 0);
//!
//! // Add a desaturated blue light with custom color
//! const customLight = put_custom_color(3, 20, 150, 100, 170, 128);
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
//! // Create a rainbow light source (default)
//! let canvas_ptr = lighting::update_or_add_light(1, 30, 100, 50);
//!
//! // Create a solid color light source
//! let red_light = lighting::update_or_add_light_with_solid_color(2, 25, 200, 50, 0);
//!
//! // Create a custom HSV color light source
//! let custom_light = lighting::update_or_add_light_with_custom_color(3, 20, 150, 100, 170, 128);
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

use wasm_bindgen::prelude::*;

// Re-export public modules for library use
pub mod arctan;
pub mod block_map;
pub mod collision;
pub mod constants;
pub mod lighting;
pub mod ray;

/// Legacy function pointer type for obstacle detection (deprecated)
/// 
/// This type is kept for backwards compatibility but is no longer used.
/// The new collision detection system uses the `collision` module instead.
#[deprecated(note = "Use collision::is_blocked instead")]
pub type IsBlockedFn = fn(i16, i16, i16, i16) -> bool;

/// External JavaScript function for logging debug information.
///
/// This function allows the Rust code to output debug information
/// to the browser's console when running in a web environment.
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log_from_js(s: &str);
}

/// Legacy function to reset the obstacle detection function (deprecated)
///
/// This function is kept for backwards compatibility but does nothing.
/// Use the new collision detection system instead.
#[deprecated(note = "Use collision::set_collision_mode instead")]
pub fn reset_is_blocked_fn() {
    // No-op: the new collision system doesn't use function pointers
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
    // Set up panic hook to get better error messages instead of "unreachable executed"
    #[cfg(target_arch = "wasm32")]
    console_error_panic_hook::set_once();
    
    lighting::init();
    block_map::init();
    collision::init();
}

/// Updates an existing light or creates a new one.
///
/// This is the primary interface for managing lights in the scene.
/// Each light is identified by a unique ID and can be updated independently.
/// Uses the default rainbow color mode for backward compatibility.
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
/// // Create a rainbow light with radius 50 at position (200, 100)
/// const lightCanvas = put(0, 50, 200, 100);
///
/// // Later, move the same light to a new position
/// const updatedCanvas = put(0, 50, 250, 150);
/// ```
#[wasm_bindgen]
pub fn put(id: u8, r: i16, x: i16, y: i16) -> *const lighting::Color {
    lighting::update_or_add_light(id, r, x, y)
}

/// Updates an existing light or creates a new one with a solid color.
///
/// # Arguments
/// * `id` - Unique identifier for this light (0-255)
/// * `r` - Light radius/range in world units
/// * `x` - World X coordinate of the light center
/// * `y` - World Y coordinate of the light center
/// * `hue` - Color hue (0-255, representing 0-360°)
///
/// # Returns
/// A pointer to the light's rendered canvas data (RGBA pixel array).
///
/// # Example Usage (JavaScript)
///
/// ```javascript
/// // Create a red light (hue=0) with radius 50 at position (200, 100)
/// const lightCanvas = put_solid_color(0, 50, 200, 100, 0);
///
/// // Create a green light (hue=85) with radius 30 at position (150, 200)
/// const greenLight = put_solid_color(1, 30, 150, 200, 85);
/// ```
#[wasm_bindgen]
pub fn put_solid_color(id: u8, r: i16, x: i16, y: i16, hue: u8) -> *const lighting::Color {
    lighting::update_or_add_light_with_solid_color(id, r, x, y, hue)
}

/// Updates an existing light or creates a new one with custom HSV color.
///
/// # Arguments
/// * `id` - Unique identifier for this light (0-255)
/// * `r` - Light radius/range in world units
/// * `x` - World X coordinate of the light center
/// * `y` - World Y coordinate of the light center
/// * `hue` - Color hue (0-255, representing 0-360°)
/// * `saturation` - Color saturation (0-255, 0=grayscale, 255=full color)
///
/// # Returns
/// A pointer to the light's rendered canvas data (RGBA pixel array).
///
/// # Example Usage (JavaScript)
///
/// ```javascript
/// // Create a desaturated blue light
/// const lightCanvas = put_custom_color(0, 50, 200, 100, 170, 128);
///
/// // Create a bright cyan light  
/// const cyanLight = put_custom_color(1, 30, 150, 200, 128, 255);
/// ```
#[wasm_bindgen]
pub fn put_custom_color(id: u8, r: i16, x: i16, y: i16, hue: u8, saturation: u8) -> *const lighting::Color {
    lighting::update_or_add_light_with_custom_color(id, r, x, y, hue, saturation)
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

/// Set the collision detection mode for the lighting engine.
///
/// Switches between different collision detection strategies to optimize
/// performance for different use cases.
///
/// # Arguments
/// * `mode` - Collision detection mode:
///   - 0: Tile-based collision (structured worlds)
///   - 1: Pixel-based collision (freeform drawing)
///   - 2: Auto-select based on scenario
///
/// # Example Usage (JavaScript)
/// ```javascript
/// // Switch to pixel-perfect collision for drawing
/// set_collision_mode(1);
/// 
/// // Switch back to tile-based for structured worlds
/// set_collision_mode(0);
/// ```
#[wasm_bindgen]
pub fn set_collision_mode(mode: u8) {
    let collision_mode = match mode {
        0 => collision::CollisionMode::Tile,
        1 => collision::CollisionMode::Pixel,
        2 => collision::CollisionMode::Auto,
        _ => collision::CollisionMode::Tile, // Default fallback
    };
    collision::set_collision_mode(collision_mode);
}

/// Get the current collision detection mode.
///
/// # Returns
/// Current collision mode as u8:
/// - 0: Tile-based collision
/// - 1: Pixel-based collision  
/// - 2: Auto-select mode
#[wasm_bindgen]
pub fn get_collision_mode() -> u8 {
    match collision::get_collision_mode() {
        collision::CollisionMode::Tile => 0,
        collision::CollisionMode::Pixel => 1,
        collision::CollisionMode::Auto => 2,
    }
}

/// Set multiple pixels as blocked or unblocked in a batch operation.
///
/// This function provides efficient batch updates for pixel-based collision
/// detection, minimizing the overhead of individual pixel updates.
///
/// # Arguments
/// * `pixels` - Byte array where each 3 consecutive bytes represent one pixel:
///   - [x_low, x_high, y_low, y_high, blocked, ...] pattern
///   - x/y coordinates are split into low/high bytes for u16 support
///   - blocked: 0 = unblocked, non-zero = blocked
///
/// # Performance
/// 
/// Batch updates are significantly more efficient than individual pixel
/// updates when modifying multiple pixels simultaneously.
///
/// # Example Usage (JavaScript)
/// ```javascript
/// // Set pixels (10,20), (11,21), (12,22) as blocked
/// const pixels = new Uint8Array([
///     10, 0, 20, 0, 1,  // (10,20) blocked
///     11, 0, 21, 0, 1,  // (11,21) blocked  
///     12, 0, 22, 0, 0,  // (12,22) unblocked
/// ]);
/// set_pixel_batch(pixels);
/// ```
#[wasm_bindgen]
pub fn set_pixel_batch(pixels: &[u8]) {
    // Each pixel requires 5 bytes: x_low, x_high, y_low, y_high, blocked
    if pixels.len() % 5 != 0 {
        console_log!("Warning: pixel batch data length {} is not divisible by 5", pixels.len());
        return;
    }

    let pixel_updates: Vec<(u16, u16, bool)> = pixels
        .chunks_exact(5)
        .map(|chunk| {
            let x = u16::from_le_bytes([chunk[0], chunk[1]]);
            let y = u16::from_le_bytes([chunk[2], chunk[3]]);
            let blocked = chunk[4] != 0;
            (x, y, blocked)
        })
        .collect();
    
    if !collision::set_pixel_batch(pixel_updates) {
        console_log!("Warning: set_pixel_batch called but current collision mode is not pixel-based");
    }
}

/// Set a single pixel in the collision map if in pixel mode.
///
/// # Arguments
/// * `x`, `y` - Pixel coordinates
/// * `blocked` - Whether the pixel should block light rays (0 = unblocked, non-zero = blocked)
#[wasm_bindgen]
pub fn set_pixel(x: u16, y: u16, blocked: u8) {
    let is_blocked = blocked != 0;
    
    if !collision::set_pixel(x, y, is_blocked) {
        console_log!("Warning: set_pixel called but current collision mode is not pixel-based");
    }
}

/// Clear all pixel collision data.
///
/// Resets all pixels to unblocked state. This is useful for clearing
/// the collision map when starting fresh or switching scenes.
#[wasm_bindgen]
pub fn clear_pixel_collisions() {
    collision::clear_collisions();
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
pub use collision::{init as init_collision, CollisionMode};
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
