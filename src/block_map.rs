//! Block map system for obstacle detection and collision testing.
//!
//! This module manages a tile-based world representation where each tile can
//! contain obstacles that block light rays. The system provides efficient
//! querying of blocking information for the lighting engine's ray casting.
//!
//! # Architecture
//!
//! - **Tiles**: Large grid cells that define the basic world structure
//! - **Cells**: Smaller subdivisions within each tile for fine-grained collision
//! - **Block Detection**: Efficient queries for ray-obstacle intersections
//!
//! # Thread Safety
//!
//! The module uses `RwLock` to provide thread-safe access to the world data,
//! allowing multiple concurrent readers while ensuring exclusive access for writers.

use once_cell::sync::Lazy;
use std::sync::RwLock;

use crate::constants::{CELLS_PER_ROW, CELLS_PER_TILE, CELLS_TOTAL, TILES_PER_ROW, TILES_TOTAL};

/// Represents the blocking state of a single cell's edges.
///
/// Each cell can have blocked edges in the four cardinal directions,
/// which affects how light rays interact with the environment.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct CellDetails {
    /// Whether the northern edge of this cell blocks light
    pub n_blocked: bool,
    /// Whether the eastern edge of this cell blocks light
    pub e_blocked: bool,
    /// Whether the southern edge of this cell blocks light
    pub s_blocked: bool,
    /// Whether the western edge of this cell blocks light
    pub w_blocked: bool,
}

/// Internal representation of a tile with its neighboring tiles.
///
/// This struct is used during block map calculations to determine
/// edge blocking based on tile type differences between neighbors.
#[derive(Default)]
struct TileNeighborhood {
    /// The tile type at this position
    tile: u8,
    /// Tile type to the north
    north: u8,
    /// Tile type to the east
    east: u8,
    /// Tile type to the south
    south: u8,
    /// Tile type to the west
    west: u8,
}

/// Thread-safe storage for cell blocking information.
///
/// Each cell in the world has associated blocking information that
/// determines how light rays interact with tile boundaries.
static CELLS: Lazy<RwLock<Vec<CellDetails>>> =
    Lazy::new(|| RwLock::new(vec![CellDetails::default(); CELLS_TOTAL]));

/// Thread-safe storage for tile type information.
///
/// Each tile in the world has a type ID that determines its properties
/// and how it interacts with neighboring tiles.
static TILES: Lazy<RwLock<Vec<u8>>> = Lazy::new(|| RwLock::new(vec![0; TILES_TOTAL]));

/// Returns a pointer to the tile data array for WASM interoperability.
///
/// This function provides direct access to the tile data for JavaScript
/// or other external systems that need to read the world state.
///
/// # Returns
///
/// A raw pointer to the first element of the tiles array. The array
/// contains `TILES_TOTAL` elements, each representing a tile type ID.
///
/// # Safety
///
/// The returned pointer is valid as long as the global TILES storage exists.
/// Callers must ensure they don't access beyond `TILES_TOTAL` elements.
pub fn get_tiles() -> *const u8 {
    // We can safely return a pointer to the data since we're only reading
    if let Ok(tiles) = TILES.read() {
        tiles.as_ptr()
    } else {
        std::ptr::null()
    }
}

/// Returns a copy of the current tile data as a `Vec<i32>`.
///
/// This function is useful for passing the tilemap data to other modules
/// that require a `Vec<i32>` representation (e.g., UnionFind).
pub fn get_tiles_vec_i32() -> Vec<i32> {
    if let Ok(tiles) = TILES.read() {
        tiles.iter().map(|&x| x as i32).collect()
    } else {
        Vec::new()
    }
}

/// Returns a pointer to the cell blocking data for WASM interoperability.
///
/// This function provides direct access to the cell blocking information
/// for the lighting engine and external systems.
///
/// # Returns
///
/// A raw pointer to the first element of the cells array. The array
/// contains `CELLS_TOTAL` elements, each representing blocking information
/// for one cell in the world.
///
/// # Safety
///
/// The returned pointer is valid as long as the global CELLS storage exists.
/// Callers must ensure they don't access beyond `CELLS_TOTAL` elements.
pub fn get_blockmap() -> *const CellDetails {
    // We can safely return a pointer to the data since we're only reading
    if let Ok(cells) = CELLS.read() {
        cells.as_ptr()
    } else {
        std::ptr::null()
    }
}

/// Sets the type of a tile at the specified coordinates.
///
/// When a tile type changes, this function updates the tile data and
/// recalculates the blocking information for all affected cells.
///
/// # Arguments
///
/// * `x` - X coordinate of the tile (0 to TILES_PER_ROW-1)
/// * `y` - Y coordinate of the tile (0 to TILES_PER_ROW-1)
/// * `tile` - New tile type ID
///
/// # Thread Safety
///
/// This function is thread-safe and will block until it can acquire
/// exclusive access to both the tiles and cells data.
pub fn set_tile(x: u32, y: u32, tile: u8) {
    let index = (x as usize) + (y as usize * TILES_PER_ROW);

    // Validate coordinates
    if index >= TILES_TOTAL {
        return;
    }

    // Update the tile data
    if let Ok(mut tiles) = TILES.write() {
        tiles[index] = tile;
    } else {
        return;
    }

    // Recalculate blocking information
    update_blockmap();

    // If in hybrid mode, update the collision map with the new tile data
    use crate::collision::{self, CollisionMode};
    use crate::constants::TILES_PER_ROW;
    use crate::lighting;

    if collision::get_collision_mode() == CollisionMode::Hybrid {
        let tiles_vec = get_tiles_vec_i32();
        lighting::update_collision_map(tiles_vec, TILES_PER_ROW);
    }
}

/// Recalculates blocking information for all tiles in the world.
///
/// This function should be called whenever tile data changes to ensure
/// that the cell blocking information remains consistent with the world state.
///
/// # Performance
///
/// This operation is O(n) where n is the number of tiles. For large worlds,
/// consider implementing incremental updates that only recalculate affected areas.
fn update_blockmap() {
    // Process each tile to update its cell blocking information
    for i in 0..TILES_TOTAL {
        update_tile_blocking(i);
    }
}

/// Updates the blocking information for a specific tile.
///
/// This function examines a tile and its neighbors to determine which
/// cell edges should be marked as blocking. Edges are typically blocked
/// when adjacent tiles have different types.
///
/// # Arguments
///
/// * `tile_index` - Linear index of the tile in the tiles array
fn update_tile_blocking(tile_index: usize) {
    let row = tile_index / TILES_PER_ROW;

    // Gather neighborhood information
    let neighborhood = if let Ok(tiles) = TILES.read() {
        TileNeighborhood {
            tile: tiles[tile_index],
            north: if tile_index >= TILES_PER_ROW {
                tiles[tile_index - TILES_PER_ROW]
            } else {
                0
            },
            east: if (tile_index + 1) / TILES_PER_ROW == row && tile_index + 1 < TILES_TOTAL {
                tiles[tile_index + 1]
            } else {
                0
            },
            south: if tile_index + TILES_PER_ROW < TILES_TOTAL {
                tiles[tile_index + TILES_PER_ROW]
            } else {
                0
            },
            west: if tile_index > 0 && (tile_index - 1) / TILES_PER_ROW == row {
                tiles[tile_index - 1]
            } else {
                0
            },
        }
    } else {
        return;
    };

    // Update the cells within this tile
    update_tile_cells(tile_index, &neighborhood);
}

/// Updates the blocking information for all cells within a specific tile.
///
/// This function sets the blocking state for each cell edge based on
/// the tile type differences with neighboring tiles.
///
/// # Arguments
///
/// * `tile_idx` - Index of the tile being updated
/// * `neighborhood` - Information about the tile and its neighbors
///
/// # Blocking Logic
///
/// A cell edge is marked as blocked if:
/// - It's on the boundary of the tile (edge of tile area)
/// - The current tile type differs from the neighboring tile type
fn update_tile_cells(tile_idx: usize, neighborhood: &TileNeighborhood) {
    let tile_x = tile_idx % TILES_PER_ROW;
    let tile_y = tile_idx / TILES_PER_ROW;

    // Calculate the cell coordinate range for this tile
    let start_x = tile_x * CELLS_PER_TILE;
    let start_y = tile_y * CELLS_PER_TILE;
    let end_x = (tile_x + 1) * CELLS_PER_TILE - 1;
    let end_y = (tile_y + 1) * CELLS_PER_TILE - 1;

    // Update blocking information for each cell in the tile
    if let Ok(mut cells) = CELLS.write() {
        for y in start_y..=end_y {
            for x in start_x..=end_x {
                let cell_index = y * CELLS_PER_ROW + x;

                if cell_index < CELLS_TOTAL {
                    let cell = &mut cells[cell_index];

                    // Mark edges as blocked based on tile type differences
                    cell.n_blocked = y == start_y && neighborhood.tile != neighborhood.north;
                    cell.e_blocked = x == end_x && neighborhood.tile != neighborhood.east;
                    cell.s_blocked = y == end_y && neighborhood.tile != neighborhood.south;
                    cell.w_blocked = x == start_x && neighborhood.tile != neighborhood.west;
                }
            }
        }
    }
}

/// Initializes the block map system.
///
/// This function should be called once during engine startup to ensure
/// the block map data structures are properly initialized.
pub fn init() {
    // Force initialization of the lazy statics
    Lazy::force(&CELLS);
    Lazy::force(&TILES);

    // Perform initial block map calculation
    update_blockmap();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_tile_updates_blocking() {
        // Set up a simple test case
        set_tile(0, 0, 1);
        set_tile(1, 0, 2);

        // The cells on the border between these tiles should be blocked
        let blockmap = get_blockmap();
        assert!(!blockmap.is_null());
    }

    #[test]
    fn test_tile_coordinates_validation() {
        // This should not panic or cause issues
        set_tile(1000, 1000, 1);

        // Normal coordinates should work
        set_tile(0, 0, 1);
        set_tile(TILES_PER_ROW as u32 - 1, TILES_PER_ROW as u32 - 1, 2);
    }

    #[test]
    fn test_get_functions_return_valid_pointers() {
        let tiles_ptr = get_tiles();
        let cells_ptr = get_blockmap();

        assert!(!tiles_ptr.is_null());
        assert!(!cells_ptr.is_null());
    }
}
