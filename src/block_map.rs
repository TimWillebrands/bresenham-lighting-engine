//! Per-cell edge-blocking flags derived from the tile map.
//!
//! Each cell carries `n/e/s/w_blocked` flags marking which of its four edges
//! sit on a Wall (a boundary between two Tiles of different types). The
//! derivation is a pure function of the tile map; [`LightingEngine`] holds the
//! current tile array and the derived cell array as owned fields.
//!
//! Free functions in this module are back-compat shims that operate on
//! [`crate::engine::DEFAULT_ENGINE`].
//!
//! [`LightingEngine`]: crate::engine::LightingEngine

use crate::engine::DEFAULT_ENGINE;

/// Per-cell edge-blocking flags.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct CellDetails {
    /// Whether the northern edge of this cell sits on a Wall.
    pub n_blocked: bool,
    /// Whether the eastern edge of this cell sits on a Wall.
    pub e_blocked: bool,
    /// Whether the southern edge of this cell sits on a Wall.
    pub s_blocked: bool,
    /// Whether the western edge of this cell sits on a Wall.
    pub w_blocked: bool,
}

#[derive(Default)]
struct TileNeighborhood {
    tile: u8,
    north: u8,
    east: u8,
    south: u8,
    west: u8,
}

fn neighborhood_of(tile_index: usize, tiles: &[u8], tiles_per_row: usize) -> TileNeighborhood {
    let tiles_total = tiles.len();
    let row = tile_index / tiles_per_row;
    TileNeighborhood {
        tile: tiles[tile_index],
        north: if tile_index >= tiles_per_row {
            tiles[tile_index - tiles_per_row]
        } else {
            0
        },
        east: if (tile_index + 1) / tiles_per_row == row && tile_index + 1 < tiles_total {
            tiles[tile_index + 1]
        } else {
            0
        },
        south: if tile_index + tiles_per_row < tiles_total {
            tiles[tile_index + tiles_per_row]
        } else {
            0
        },
        west: if tile_index > 0 && (tile_index - 1) / tiles_per_row == row {
            tiles[tile_index - 1]
        } else {
            0
        },
    }
}

/// Pure function: recompute the cell edge flags inside one tile from the tile
/// map at the given resolution. Writes into `cells[..]` at the indices belonging
/// to this tile.
///
/// Used by [`crate::engine::LightingEngine`] to refresh its block map when a
/// tile changes.
pub fn compute_cell_details_for_tile(
    tile_idx: usize,
    tiles: &[u8],
    cells: &mut [CellDetails],
    cells_per_tile: usize,
    tiles_per_row: usize,
) {
    let neighborhood = neighborhood_of(tile_idx, tiles, tiles_per_row);
    let tile_x = tile_idx % tiles_per_row;
    let tile_y = tile_idx / tiles_per_row;
    let cells_per_row = cells_per_tile * tiles_per_row;
    let cells_total = cells.len();

    let start_x = tile_x * cells_per_tile;
    let start_y = tile_y * cells_per_tile;
    let end_x = (tile_x + 1) * cells_per_tile - 1;
    let end_y = (tile_y + 1) * cells_per_tile - 1;

    for y in start_y..=end_y {
        for x in start_x..=end_x {
            let cell_index = y * cells_per_row + x;
            if cell_index >= cells_total {
                continue;
            }
            let cell = &mut cells[cell_index];
            cell.n_blocked = y == start_y && neighborhood.tile != neighborhood.north;
            cell.e_blocked = x == end_x && neighborhood.tile != neighborhood.east;
            cell.s_blocked = y == end_y && neighborhood.tile != neighborhood.south;
            cell.w_blocked = x == start_x && neighborhood.tile != neighborhood.west;
        }
    }
}

// ------------------------------- shims ----------------------------------

/// WASM/back-compat shim. Returns a pointer to the default engine's tile array.
pub fn get_tiles() -> *const u8 {
    if let Ok(e) = DEFAULT_ENGINE.read() {
        e.tiles().as_ptr()
    } else {
        std::ptr::null()
    }
}

/// WASM/back-compat shim. Returns a copy of the default engine's tile array.
pub fn get_tiles_vec_i32() -> Vec<i32> {
    if let Ok(e) = DEFAULT_ENGINE.read() {
        e.tiles().iter().map(|&x| x as i32).collect()
    } else {
        Vec::new()
    }
}

/// WASM/back-compat shim. Returns a pointer to the default engine's block map.
pub fn get_blockmap() -> *const CellDetails {
    if let Ok(e) = DEFAULT_ENGINE.read() {
        e.block_map().as_ptr()
    } else {
        std::ptr::null()
    }
}

/// WASM/back-compat shim. Forwards to [`crate::engine::DEFAULT_ENGINE`].
pub fn set_tile(x: u32, y: u32, tile: u8) {
    if let Ok(mut e) = DEFAULT_ENGINE.write() {
        e.set_tile(x, y, tile);
    }
}

/// Force initialization of the default engine. Cheap; idempotent.
pub fn init() {
    once_cell::sync::Lazy::force(&DEFAULT_ENGINE);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::DEFAULT_TILES_PER_ROW as TILES_PER_ROW;

    #[test]
    fn test_set_tile_updates_blocking() {
        set_tile(0, 0, 1);
        set_tile(1, 0, 2);
        let blockmap = get_blockmap();
        assert!(!blockmap.is_null());
    }

    #[test]
    fn test_tile_coordinates_validation() {
        set_tile(1000, 1000, 1);
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
