//! Global constants for the lighting engine's world representation.
//!
//! This module defines the fundamental dimensions and scales used throughout
//! the lighting engine. These constants determine the resolution and size
//! of the world that can be rendered and lit.
//!
//! # World Structure
//!
//! The world is organized in a hierarchical structure:
//! - **Tiles**: Large grid cells that represent distinct areas or objects
//! - **Cells**: Fine-grained subdivisions within each tile for precise collision detection
//!
//! This two-level system allows for efficient storage and processing while
//! maintaining the precision needed for accurate lighting calculations.

/// Number of cells along one edge of a single tile.
///
/// Each tile is subdivided into a grid of cells for fine-grained collision detection.
/// A larger value provides more precision but increases memory usage and computation time.
///
/// # Value
/// Currently set to 6, meaning each tile contains a 6×6 grid of cells (36 cells total).
pub const CELLS_PER_TILE: usize = 6;

/// Number of tiles along one edge of the world.
///
/// The world is represented as a square grid of tiles. This value determines
/// the maximum world size that can be represented.
///
/// # Value
/// Currently set to 30, creating a 30×30 tile world (900 tiles total).
///
/// # Memory Impact
/// Increasing this value has a quadratic effect on memory usage, as both
/// tile storage and cell storage scale with the square of this value.
pub const TILES_PER_ROW: usize = 30;

/// Number of cells along one edge of the entire world.
///
/// This is a derived constant that represents the total resolution of the world
/// in terms of cells. It's calculated as `CELLS_PER_TILE × TILES_PER_ROW`.
///
/// # Value
/// With current settings: 6 × 30 = 180 cells per row
///
/// # Usage
/// This constant is used for:
/// - Converting between tile coordinates and cell coordinates
/// - Calculating array indices for cell-based operations
/// - Determining world boundaries for collision detection
pub const CELLS_PER_ROW: usize = CELLS_PER_TILE * TILES_PER_ROW;

/// Total number of cells in the entire world.
///
/// This represents the complete cell array size needed to store collision
/// information for the entire world. It's calculated as `CELLS_PER_ROW²`.
///
/// # Value
/// With current settings: 180² = 32,400 cells total
///
/// # Memory Usage
/// Each cell typically stores 4 boolean values (for edge blocking),
/// so the total memory for cell data is approximately:
/// `CELLS_TOTAL × 4 bytes = 129,600 bytes ≈ 127 KB`
pub const CELLS_TOTAL: usize = CELLS_PER_ROW * CELLS_PER_ROW;

/// Total number of tiles in the entire world.
///
/// This represents the complete tile array size needed to store tile type
/// information for the entire world. It's calculated as `TILES_PER_ROW²`.
///
/// # Value
/// With current settings: 30² = 900 tiles total
///
/// # Memory Usage
/// Each tile typically stores a single byte (tile type ID),
/// so the total memory for tile data is approximately:
/// `TILES_TOTAL × 1 byte = 900 bytes`
pub const TILES_TOTAL: usize = TILES_PER_ROW * TILES_PER_ROW;

// Compile-time assertions to ensure our constants make sense
const _: () = {
    // Ensure CELLS_PER_ROW is correctly calculated
    assert!(CELLS_PER_ROW == CELLS_PER_TILE * TILES_PER_ROW);

    // Ensure CELLS_TOTAL is correctly calculated
    assert!(CELLS_TOTAL == CELLS_PER_ROW * CELLS_PER_ROW);

    // Ensure TILES_TOTAL is correctly calculated
    assert!(TILES_TOTAL == TILES_PER_ROW * TILES_PER_ROW);

    // Ensure we have at least one cell per tile (sanity check)
    assert!(CELLS_PER_TILE > 0);

    // Ensure we have at least one tile (sanity check)
    assert!(TILES_PER_ROW > 0);
};

/// Converts tile coordinates to the starting cell coordinates.
///
/// # Arguments
/// * `tile_x` - X coordinate of the tile
/// * `tile_y` - Y coordinate of the tile
///
/// # Returns
/// A tuple containing the (cell_x, cell_y) coordinates of the top-left cell in the tile.
///
/// # Example
/// ```
/// use bresenham_lighting_engine::constants::tile_to_cell_coords;
///
/// let (cell_x, cell_y) = tile_to_cell_coords(2, 3);
/// // For a tile at (2, 3), the starting cell would be at (12, 18)
/// // since each tile is 6×6 cells: (2*6, 3*6) = (12, 18)
/// ```
#[inline]
pub const fn tile_to_cell_coords(tile_x: usize, tile_y: usize) -> (usize, usize) {
    (tile_x * CELLS_PER_TILE, tile_y * CELLS_PER_TILE)
}

/// Converts cell coordinates to tile coordinates.
///
/// # Arguments
/// * `cell_x` - X coordinate of the cell
/// * `cell_y` - Y coordinate of the cell
///
/// # Returns
/// A tuple containing the (tile_x, tile_y) coordinates of the tile containing the cell.
///
/// # Example
/// ```
/// use bresenham_lighting_engine::constants::cell_to_tile_coords;
///
/// let (tile_x, tile_y) = cell_to_tile_coords(14, 20);
/// // Cell (14, 20) belongs to tile (2, 3) since:
/// // 14 / 6 = 2, 20 / 6 = 3
/// ```
#[inline]
pub const fn cell_to_tile_coords(cell_x: usize, cell_y: usize) -> (usize, usize) {
    (cell_x / CELLS_PER_TILE, cell_y / CELLS_PER_TILE)
}

/// Calculates the linear array index for a tile at the given coordinates.
///
/// # Arguments
/// * `tile_x` - X coordinate of the tile
/// * `tile_y` - Y coordinate of the tile
///
/// # Returns
/// The linear index that can be used to access the tile in a flat array.
///
/// # Example
/// ```
/// use bresenham_lighting_engine::constants::tile_index;
///
/// let index = tile_index(2, 3);
/// // For a 30×30 tile grid, tile (2, 3) has index: 3*30 + 2 = 92
/// ```
#[inline]
pub const fn tile_index(tile_x: usize, tile_y: usize) -> usize {
    tile_y * TILES_PER_ROW + tile_x
}

/// Calculates the linear array index for a cell at the given coordinates.
///
/// # Arguments
/// * `cell_x` - X coordinate of the cell
/// * `cell_y` - Y coordinate of the cell
///
/// # Returns
/// The linear index that can be used to access the cell in a flat array.
///
/// # Example
/// ```
/// use bresenham_lighting_engine::constants::cell_index;
///
/// let index = cell_index(14, 20);
/// // For a 180×180 cell grid, cell (14, 20) has index: 20*180 + 14 = 3614
/// ```
#[inline]
pub const fn cell_index(cell_x: usize, cell_y: usize) -> usize {
    cell_y * CELLS_PER_ROW + cell_x
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coordinate_conversions() {
        // Test tile to cell conversion
        let (cell_x, cell_y) = tile_to_cell_coords(2, 3);
        assert_eq!(cell_x, 12);
        assert_eq!(cell_y, 18);

        // Test cell to tile conversion
        let (tile_x, tile_y) = cell_to_tile_coords(14, 20);
        assert_eq!(tile_x, 2);
        assert_eq!(tile_y, 3);

        // Test round-trip conversion
        let original_tile = (5, 7);
        let (cell_x, cell_y) = tile_to_cell_coords(original_tile.0, original_tile.1);
        let recovered_tile = cell_to_tile_coords(cell_x, cell_y);
        assert_eq!(original_tile, recovered_tile);
    }

    #[test]
    fn test_index_calculations() {
        // Test tile indexing
        let index = tile_index(2, 3);
        assert_eq!(index, 3 * TILES_PER_ROW + 2);

        // Test cell indexing
        let index = cell_index(14, 20);
        assert_eq!(index, 20 * CELLS_PER_ROW + 14);

        // Test boundary conditions
        let max_tile_index = tile_index(TILES_PER_ROW - 1, TILES_PER_ROW - 1);
        assert_eq!(max_tile_index, TILES_TOTAL - 1);

        let max_cell_index = cell_index(CELLS_PER_ROW - 1, CELLS_PER_ROW - 1);
        assert_eq!(max_cell_index, CELLS_TOTAL - 1);
    }

    #[test]
    fn test_constants_consistency() {
        // Verify that our derived constants are calculated correctly
        assert_eq!(CELLS_PER_ROW, CELLS_PER_TILE * TILES_PER_ROW);
        assert_eq!(CELLS_TOTAL, CELLS_PER_ROW * CELLS_PER_ROW);
        assert_eq!(TILES_TOTAL, TILES_PER_ROW * TILES_PER_ROW);
    }
}
