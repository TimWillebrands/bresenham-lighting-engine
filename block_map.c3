module blockmap;

import constants;
import std::io;

struct CellDetails// : char
{
    bool n_blocked;
    bool e_blocked;
    bool s_blocked;
    bool w_blocked;
}

struct Cell
{
    char tile;
    char north;
    char north_west;
    char west;
    char south_west;
    char south;
    char south_east;
    char east;
    char north_east;
}

CellDetails[constants::CELLS_TOTAL] cells; 
char[constants::TILES_TOTAL] tiles; 

const int TIR = constants::TILES_PER_ROW;
const int TT = constants::TILES_TOTAL;

fn char* get_tiles() @extern("getTiles") @wasm {
    return &tiles[0];
}

fn CellDetails* get_blockmap() @extern("getBlockmap") @wasm {
    return &cells[0];
}

fn void set_tile(uint x, uint y, char tile) @extern("setTile") @wasm {
    int index = x + (y * constants::TILES_PER_ROW);
    tiles[index] = tile;
}

fn void update_blockmap () {
    for(int i; i < constants::TILES_TOTAL; i++) {
        Cell tile_n_neighbours = update_tile(1);
    }
}

macro check_north(i)     { return i - TIR >= 0; }
macro check_east(i, row) { return (i + 1) / TIR == row && i + 1 < TT; }
macro check_south(i)     { return i + TIR < TT; }
macro check_west(i, row) { return (i - 1) / TIR == row && i - 1 >= 0; }

fn Cell update_tile (int i) {
    int col = i % TIR;
    int row = i / TIR;

    Cell cell;
    cell.tile  = tiles[i];
    cell.north = check_north(i)     ? tiles[i - TIR] : 0;
    cell.east  = check_east(i, row) ? tiles[i + 1] : 0;
    cell.south = check_south(i)     ? tiles[i + TIR] : 0;
    cell.west  = check_west(i, row) ? tiles[i - 1] : 0;

    cell.north_east = check_north(i) && check_east(i, row) ? tiles[i - TIR + 1] : 0;
    cell.south_east = check_south(i) && check_east(i, row) ? tiles[i + TIR + 1] : 0;
    cell.north_west = check_north(i) && check_west(i, row) ? tiles[i - TIR - 1] : 0;
    cell.south_west = check_south(i) && check_west(i, row) ? tiles[i + TIR - 1] : 0;

    update_tile_cells(i, cell);

    return cell;
}

fn void update_tile_cells (int tile_idx, Cell cell) {
    int tile_x = tile_idx % constants::TILES_PER_ROW;
    int tile_y = tile_idx / constants::TILES_PER_ROW;
    int ratio = constants::CELLS_PER_TILE;

    int start_x = tile_x * ratio;
    int start_y = tile_y * ratio;
    int end_x = (tile_x + 1) * ratio - 1;
    int end_y = (tile_y + 1) * ratio - 1;

    // Iterate through all grid2 cells in the calculated range
    // and set the borders of the subgrid accordingly
    for (int x = start_x; x <= end_x; x++) {
        for (int y = start_y; y <= end_y; y++) {
            CellDetails* c = &cells[y * constants::CELLS_PER_ROW + x];
            c.n_blocked = y == start_y && cell.tile != cell.north;
            c.e_blocked = x == end_x   && cell.tile != cell.east;
            c.s_blocked = y == end_y   && cell.tile != cell.south;
            c.w_blocked = x == start_x && cell.tile != cell.west;
        }
    }
}

fn void assert_cell(int x, int y, bool n, bool e, bool s, bool w) {
    CellDetails* c = &cells[y * constants::CELLS_PER_ROW + x];
    assert(c.n_blocked == n, "North blocked mismatch at %d,%d", x, y);
    assert(c.e_blocked == e, "East blocked mismatch at %d,%d", x, y);
    assert(c.s_blocked == s, "South blocked mismatch at %d,%d", x, y);
    assert(c.w_blocked == w, "West blocked mismatch at %d,%d", x, y);
}

fn void test_update_tile() @test
{
    tiles[0] = 1;
    tiles[1] = 2;

    Cell tile = update_tile(0);
    tile = update_tile(1);
    assert(tile.tile == 2, "Tile is 2");
    assert(tile.north == 0, "North is empty");
    assert(tile.west == 1, "West is 1");
    assert(tile.north_west == 0, "Northwest is empty");

    assert_cell(0, 0, true,  false, false, true);
    assert_cell(0, 1, false, false, false, true);
    assert_cell(5, 1, false, true,  false, false);
    assert_cell(5, 5, false, true,  true,  false);
    assert_cell(0, 5, false, false, true,  true);
    assert_cell(3, 3, false, false, false, false);
}

fn void test_update_tile_connected() @test
{
    tiles[0] = 2;
    tiles[1] = 2;

    Cell tile = update_tile(0);
    tile = update_tile(1);
    assert(tile.tile == 2, "Tile is 2");
    assert(tile.north == 0, "North is empty");
    assert(tile.west == 2, "West is 2");
    assert(tile.north_west == 0, "Northwest is empty");

    assert_cell(0, 0, true,  false, false, true);
    assert_cell(0, 1, false, false, false, true);
    assert_cell(5, 1, false, false, false, false);
    assert_cell(5, 5, false, false, true,  false);
    assert_cell(0, 5, false, false, true,  true);
    assert_cell(3, 3, false, false, false, false);
    assert_cell(6, 1, false, false, false, false);
    assert_cell(6, 5, false, false, true,  false);
    assert_cell(11, 1, false, true, false, false);
    assert_cell(11, 5, false, true, true,  false);
}

fn void test_update_tile_bottom() @test
{
    tiles[constants::TILES_TOTAL-1] = 1;
    tiles[constants::TILES_TOTAL-2] = 2;
    tiles[constants::TILES_TOTAL-constants::TILES_PER_ROW-1] = 3;
    tiles[constants::TILES_TOTAL-constants::TILES_PER_ROW-2] = 4;

    Cell tile = update_tile(constants::TILES_TOTAL-2);
    tile = update_tile(constants::TILES_TOTAL-1);
    assert(tile.west == 2, "West must be 2, but was %d", tile.west);
    assert(tile.north == 3, "North must be 3, but was %d", tile.north);
    assert(tile.north_west == 4, "Northwest must be 4, but was %d", tile.north_west);
}


// fn count_distincts (tiles: char[]) {
//     const stuff: Record<number, number> = {}
// 
//     for (let i = 0; i < tiles.length; ++i) {
//         const tile = tiles[i] ?? MAGIC_UNDEFINED_IDENTIFIER
//         stuff[tile] !== undefined ? stuff[tile]++ : (stuff[tile] = 1)
//     }
// 
//     return Object.entries(stuff).reduce((acc, [key, value]) => {
//         acc.distincts++
//         if (value > acc.largestCount) {
//             acc.largestGroup = Number.parseInt(key)
//             acc.largestCount = value
//         }
// 
//         return acc
//     }, {
//         distincts: 0,
//         largestCount: 0,
//         largestGroup: 0
//     })
// }
