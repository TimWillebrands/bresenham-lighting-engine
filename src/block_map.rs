use crate::constants::{CELLS_PER_ROW, CELLS_PER_TILE, CELLS_TOTAL, TILES_PER_ROW, TILES_TOTAL};

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct CellDetails {
    pub n_blocked: bool,
    pub e_blocked: bool,
    pub s_blocked: bool,
    pub w_blocked: bool,
}

#[derive(Default)]
struct Cell {
    tile: u8,
    north: u8,
    north_west: u8,
    west: u8,
    south_west: u8,
    south: u8,
    south_east: u8,
    east: u8,
    north_east: u8,
}

static mut CELLS: [CellDetails; CELLS_TOTAL] = [CellDetails {
    n_blocked: false,
    e_blocked: false,
    s_blocked: false,
    w_blocked: false,
}; CELLS_TOTAL];
static mut TILES: [u8; TILES_TOTAL] = [0; TILES_TOTAL];

pub fn get_tiles() -> *const u8 {
    unsafe { TILES.as_ptr() }
}

pub fn get_blockmap() -> *const CellDetails {
    unsafe { CELLS.as_ptr() }
}

pub fn set_tile(x: u32, y: u32, tile: u8) {
    let index = (x as usize) + (y as usize * TILES_PER_ROW);
    if index < TILES_TOTAL {
        unsafe {
            TILES[index] = tile;
        }
    }
    update_blockmap();
}

fn update_blockmap() {
    for i in 0..TILES_TOTAL {
        update_tile(i);
    }
}

fn update_tile(i: usize) {
    let row = i / TILES_PER_ROW;

    let cell = unsafe {
        Cell {
            tile: TILES[i],
            north: if i >= TILES_PER_ROW { TILES[i - TILES_PER_ROW] } else { 0 },
            east: if (i + 1) / TILES_PER_ROW == row && i + 1 < TILES_TOTAL { TILES[i + 1] } else { 0 },
            south: if i + TILES_PER_ROW < TILES_TOTAL { TILES[i + TILES_PER_ROW] } else { 0 },
            west: if i > 0 && (i - 1) / TILES_PER_ROW == row { TILES[i - 1] } else { 0 },
            north_east: if i >= TILES_PER_ROW && (i + 1) / TILES_PER_ROW == row && i + 1 < TILES_TOTAL { TILES[i - TILES_PER_ROW + 1] } else { 0 },
            south_east: if i + TILES_PER_ROW < TILES_TOTAL && (i + 1) / TILES_PER_ROW == row && i + 1 < TILES_TOTAL { TILES[i + TILES_PER_ROW + 1] } else { 0 },
            north_west: if i >= TILES_PER_ROW && i > 0 && (i - 1) / TILES_PER_ROW == row { TILES[i - TILES_PER_ROW - 1] } else { 0 },
            south_west: if i + TILES_PER_ROW < TILES_TOTAL && i > 0 && (i - 1) / TILES_PER_ROW == row { TILES[i + TILES_PER_ROW - 1] } else { 0 },
        }
    };
    
    update_tile_cells(i, &cell);
}

fn update_tile_cells(tile_idx: usize, cell: &Cell) {
    let tile_x = tile_idx % TILES_PER_ROW;
    let tile_y = tile_idx / TILES_PER_ROW;
    
    let start_x = tile_x * CELLS_PER_TILE;
    let start_y = tile_y * CELLS_PER_TILE;
    let end_x = (tile_x + 1) * CELLS_PER_TILE - 1;
    let end_y = (tile_y + 1) * CELLS_PER_TILE - 1;

    for y in start_y..=end_y {
        for x in start_x..=end_x {
            unsafe {
                let c = &mut CELLS[y * CELLS_PER_ROW + x];
                c.n_blocked = y == start_y && cell.tile != cell.north;
                c.e_blocked = x == end_x   && cell.tile != cell.east;
                c.s_blocked = y == end_y   && cell.tile != cell.south;
                c.w_blocked = x == start_x && cell.tile != cell.west;
            }
        }
    }
} 