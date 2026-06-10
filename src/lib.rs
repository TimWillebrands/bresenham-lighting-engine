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
pub mod engine;
pub mod lighting;
pub mod map_grid;
pub mod ray;
pub mod scenarios;

pub use engine::LightingEngine;

/// External JavaScript function for logging debug information.
///
/// This function allows the Rust code to output debug information
/// to the browser's console when running in a web environment.
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log_from_js(s: &str);
}

/// Export a logging function for JavaScript use.
///
/// This provides a way for JavaScript to log messages through the Rust
/// WASM module, which can be useful for debugging and development.
#[wasm_bindgen]
pub fn log(message: &str) {
    log_from_js(message);
}

/// Returns the WebAssembly.Memory object backing this module so JS callers
/// can construct typed-array views over pointers returned by other exports
/// (e.g. the canvas pointer from `put`).
#[wasm_bindgen]
pub fn wasm_memory() -> JsValue {
    wasm_bindgen::memory()
}

/// Maximum light radius the engine will honour. Light canvases returned by
/// `put`, `put_solid_color`, and `put_custom_color` are sized
/// `(min(r, max_light_radius()) * 2 + 1)²`. JS callers must clamp `r` to this
/// value (or read the actual canvas side length back) before constructing a
/// typed-array view over the returned pointer, or they will run off the end
/// of the canvas allocation.
#[wasm_bindgen]
pub fn max_light_radius() -> u16 {
    lighting::max_dist() as u16
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

// Collision detection is now unified around the hybrid pixel + room system
// No mode configuration is needed - the system adapts based on room configuration

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

/// Set map data for room-based collision optimization.
///
/// This function configures the room layout for the collision system.
/// Each cell in the map represents a tile where 0 = blocked (wall) and >0 = open (room).
/// Contiguous areas with the same non-zero value form rooms.
///
/// # Arguments
/// * `map_data` - Flat array representing the tilemap in row-major order
/// * `map_size` - Width/height of the square map (e.g. 180 for 180x180)
///
/// # Example Usage (JavaScript)
/// ```javascript
/// // Create a simple 3x3 map with one room
/// const mapData = new Int32Array([
///   0, 0, 0,  // wall row
///   0, 1, 0,  // room with walls
///   0, 0, 0   // wall row
/// ]);
/// set_map_data(mapData, 3);
/// ```
#[wasm_bindgen]
pub fn set_map_data(map_data: Vec<i32>, map_size: usize) {
    collision::update_map_data(map_data, map_size);
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
pub use collision::{init as init_collision};
pub use engine::{DEFAULT_CELLS_PER_TILE, DEFAULT_TILES_PER_ROW};
pub use lighting::{init as init_lighting, Color};

/// WASM/JS-facing wrapper around [`engine::LightingEngine`]. One instance per
/// `Layer` (see [ADR-0002](../../docs/adr/0002-lighting-engine-per-layer.md));
/// JS owns the handle and forwards tile/light mutations through it.
#[wasm_bindgen(js_name = LightingEngine)]
pub struct WasmLightingEngine {
    inner: engine::LightingEngine,
}

#[wasm_bindgen(js_class = LightingEngine)]
impl WasmLightingEngine {
    #[wasm_bindgen(constructor)]
    pub fn new(cells_per_tile: usize, tiles_per_row: usize) -> Self {
        Self {
            inner: engine::LightingEngine::new(cells_per_tile, tiles_per_row),
        }
    }

    pub fn cells_per_tile(&self) -> usize {
        self.inner.cells_per_tile()
    }

    pub fn tiles_per_row(&self) -> usize {
        self.inner.tiles_per_row()
    }

    pub fn cells_per_row(&self) -> usize {
        self.inner.cells_per_row()
    }

    /// Set a single tile's type. Triggers a block-map + room-graph refresh.
    pub fn set_tile(&mut self, x: u32, y: u32, tile: u8) {
        self.inner.set_tile(x, y, tile);
    }

    /// Push a tile-resolution map into the engine: copies the array into the
    /// engine's tile storage, refreshes the derived cell-edge block-map, and
    /// rebuilds the broad-phase room graph. Used by `YMapgrid` on layer init
    /// to bring a freshly-observed tile array into a per-layer engine.
    ///
    /// `map_size` must equal the engine's `tiles_per_row`; mismatches are
    /// silently ignored so callers see no panic at the wasm boundary.
    pub fn set_map_data(&mut self, map_data: Vec<i32>, map_size: usize) {
        if map_size != self.inner.tiles_per_row() {
            return;
        }
        let tiles: Vec<u8> = map_data.iter().map(|&v| v as u8).collect();
        self.inner.set_tile_map(tiles);
    }

    /// Mark a single cell as blocking (an Object cell — chairs, slimes, etc.).
    pub fn set_pixel(&mut self, x: u16, y: u16, blocked: u8) {
        self.inner.set_pixel(x, y, blocked != 0);
    }

    /// Clear all object cells (does not touch the tile map).
    pub fn clear_pixel_collisions(&mut self) {
        self.inner.clear_pixel_collisions();
    }

    /// Record (or remove) a door edge between two tiles. Open doors join
    /// the two tiles' rooms for both pathfinding and lighting (per ADR-0003).
    pub fn set_door_edge(&mut self, t1_idx: usize, t2_idx: usize, open: bool) {
        self.inner.set_door_edge(t1_idx, t2_idx, open);
    }

    /// Forget every recorded door edge. JS re-emits the door set from
    /// scratch when the Yjs token list changes.
    pub fn clear_door_edges(&mut self) {
        self.inner.clear_door_edges();
    }

    /// Tile-coord BFS pathfinder. Returns the chain of tile indices from
    /// `(x1,y1)` to `(x2,y2)`, or an empty `Vec` if no route exists.
    pub fn path(&mut self, x1: i32, y1: i32, x2: i32, y2: i32) -> Vec<usize> {
        self.inner.path(x1, y1, x2, y2)
    }

    /// Tile-coord line-of-sight check.
    pub fn cast_ray(&mut self, x1: i32, y1: i32, x2: i32, y2: i32) -> bool {
        self.inner.cast_ray(x1, y1, x2, y2)
    }

    /// 4- or 8-connected room-graph neighbours of `tile_idx`. Returned as a
    /// flat `Uint32Array`-compatible vector for the JS facade.
    pub fn neighbours(&mut self, tile_idx: usize, include_diagonal: bool) -> Vec<usize> {
        self.inner.neighbours(tile_idx, include_diagonal)
    }

    /// Tile type at `tile_idx` (or `-1` for out-of-range).
    pub fn tile_at(&self, tile_idx: usize) -> i32 {
        self.inner.tile_at(tile_idx)
    }

    /// Tile-resolution room id of `tile_idx`. Two tiles share a room iff
    /// `tile_find(a) == tile_find(b)`.
    pub fn tile_find(&mut self, tile_idx: usize) -> usize {
        self.inner.tile_find(tile_idx)
    }

    /// Create or update a rainbow light. Returns a pointer to the rendered
    /// canvas (RGBA, `(r*2+1)²` pixels) in wasm linear memory.
    pub fn put(&mut self, id: u8, r: i16, x: i16, y: i16) -> *const lighting::Color {
        self.inner.update_or_add_light(id, r, x, y)
    }

    /// Create or update a solid-color light.
    pub fn put_solid_color(
        &mut self,
        id: u8,
        r: i16,
        x: i16,
        y: i16,
        hue: u8,
    ) -> *const lighting::Color {
        self.inner.update_or_add_light_with_solid_color(id, r, x, y, hue)
    }

    /// Create or update a room-bounded ambient emitter. Floods the same-type
    /// `UnionFind` room of tile `(tile_x, tile_y)` with a flat `(r, g, b)`,
    /// returning a pointer to its full-map canvas (`cells_per_row²` RGBA cells).
    /// Mirrors `put_solid_color` but preserves authored RGB (no hue/saturation
    /// lossiness) and is room-bounded rather than radial. A non-floor tile
    /// (`tile <= 0`) yields an empty canvas. See ADR-0004.
    pub fn put_ambient(
        &mut self,
        id: u8,
        tile_x: i16,
        tile_y: i16,
        r: u8,
        g: u8,
        b: u8,
    ) -> *const lighting::Color {
        self.inner.update_or_add_ambient(id, tile_x, tile_y, r, g, b)
    }

    /// Compute the live field-of-view mask for a flat array of viewer points in
    /// cell coords (`[x0, y0, x1, y1, …]`, arriving as an `Int16Array`) and
    /// return a pointer to a full-map **FOV canvas** (`cells_per_row²` RGBA
    /// cells in wasm linear memory). Cells reached by any viewer's rays are
    /// opaque white; everything else is transparent. Binary alpha — no falloff.
    /// The canvas is reused between calls, so read it back through the
    /// wasm-memory view before the next `compute_fov`. See ADR-0006.
    pub fn compute_fov(&mut self, viewers: Vec<i16>) -> *const lighting::Color {
        self.inner.compute_fov(&viewers)
    }
}

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
        use engine::DEFAULT_TILES_PER_ROW;
        // Test tile setting with valid coordinates
        set_tile(0, 0, 1);
        set_tile(DEFAULT_TILES_PER_ROW as u32 - 1, DEFAULT_TILES_PER_ROW as u32 - 1, 2);

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
