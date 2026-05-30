//! [DEBUG-light-timeout] Bench reproducing the cold-start path that
//! `YMapgrid` walks on the JS side when a layer is first observed:
//!
//!   1. `LightingEngine::new(cells_per_tile=9, tiles_per_row=32)` — matches
//!      `LAYER_SIZE=30 + 2*ENGINE_BUFFER_TILES`.
//!   2. Bulk-set ~900 tiles with `set_tile` one at a time (the path
//!      `onTilesChanged` walks for each `insert` entry in the Yjs delta).
//!   3. Place one solid-color light and call `put_solid_color`.
//!   4. Open/close 50 door edges, each of which triggers
//!      `refresh_collision_from_tiles` + `refresh_tile_uf_from_tiles`.
//!
//! Run with `cargo run --example cold_start_bench --release` (or omit
//! `--release` to see the worse-case dev profile).
//!
//! If any single phase prints a duration >100ms native, the equivalent wasm
//! call is in the danger zone for Firefox's 10s slow-script timeout once
//! you account for the 5-10x wasm slowdown and per-frame churn.

use std::time::Instant;

use bresenham_lighting_engine::engine::LightingEngine;

const LAYER_SIZE: usize = 30;
const ENGINE_BUFFER_TILES: usize = 1;
const CELLS_PER_TILE: usize = 9;

fn main() {
    let tiles_per_row = LAYER_SIZE + 2 * ENGINE_BUFFER_TILES;
    println!(
        "engine: cells_per_tile={CELLS_PER_TILE}, tiles_per_row={tiles_per_row}, \
         cells_per_row={}, cells_total={}",
        CELLS_PER_TILE * tiles_per_row,
        (CELLS_PER_TILE * tiles_per_row).pow(2)
    );

    // Phase 1: construct.
    let t = Instant::now();
    let mut engine = LightingEngine::new(CELLS_PER_TILE, tiles_per_row);
    println!("[1] LightingEngine::new ............ {:?}", t.elapsed());

    // Phase 2: simulate `onTilesChanged` walking a freshly-loaded Yjs delta
    // one tile at a time. Real maps have a mix of wall (0) and room (1+)
    // tiles; use a chequer pattern over the inner LAYER_SIZE×LAYER_SIZE area
    // so the union-find has plenty of small components to merge each pass.
    let inner_tiles: Vec<(u32, u32, u8)> = (0..LAYER_SIZE)
        .flat_map(|y| {
            (0..LAYER_SIZE).map(move |x| {
                let val = if (x + y) % 3 == 0 { 0 } else { 1 };
                ((x + ENGINE_BUFFER_TILES) as u32, (y + ENGINE_BUFFER_TILES) as u32, val)
            })
        })
        .collect();
    let n_tiles = inner_tiles.len();

    let t = Instant::now();
    for (x, y, v) in &inner_tiles {
        engine.set_tile(*x, *y, *v);
    }
    let elapsed = t.elapsed();
    println!(
        "[2] set_tile × {n_tiles} (one-at-a-time) .. {:?}  ({:?}/tile)",
        elapsed,
        elapsed / n_tiles as u32
    );

    // Phase 2b: same payload via the bulk `set_tile_map` path that JS uses
    // on initial layer construction. Should be dramatically cheaper.
    let mut bulk_engine = LightingEngine::new(CELLS_PER_TILE, tiles_per_row);
    let mut bulk_tiles = vec![0u8; tiles_per_row * tiles_per_row];
    for (x, y, v) in &inner_tiles {
        bulk_tiles[(*y as usize) * tiles_per_row + (*x as usize)] = *v;
    }
    let t = Instant::now();
    bulk_engine.set_tile_map(bulk_tiles);
    println!("[2b] set_tile_map (one bulk call) ... {:?}", t.elapsed());

    // Phase 3: one light, centred. Engine cell coords.
    let cx = (tiles_per_row * CELLS_PER_TILE / 2) as i16;
    let cy = cx;
    let t = Instant::now();
    let _ptr = engine.update_or_add_light_with_solid_color(0, 30, cx, cy, 0);
    println!("[3] put_solid_color (r=30, centre) .. {:?}", t.elapsed());

    // Phase 4: door churn. Pick 50 adjacent tile pairs and toggle each open
    // then closed — the path the JS facade walks when door tokens change.
    let door_pairs: Vec<(usize, usize)> = (0..50)
        .map(|i| {
            let tx = (i % (LAYER_SIZE - 1)) + ENGINE_BUFFER_TILES;
            let ty = (i / (LAYER_SIZE - 1)).min(LAYER_SIZE - 1) + ENGINE_BUFFER_TILES;
            let a = ty * tiles_per_row + tx;
            let b = ty * tiles_per_row + tx + 1;
            (a, b)
        })
        .collect();

    let t = Instant::now();
    for (a, b) in &door_pairs {
        engine.set_door_edge(*a, *b, true);
    }
    println!("[4a] set_door_edge × 50 (open) ...... {:?}", t.elapsed());

    let t = Instant::now();
    for (a, b) in &door_pairs {
        engine.set_door_edge(*a, *b, false);
    }
    println!("[4b] set_door_edge × 50 (close) ..... {:?}", t.elapsed());

    // Phase 5: re-render the light after door churn (mirrors a frame where
    // the lighting system runs after a door toggle invalidated the engine).
    let t = Instant::now();
    let _ptr = engine.update_or_add_light_with_solid_color(0, 30, cx, cy, 0);
    println!("[5] put_solid_color after doors ..... {:?}", t.elapsed());
}
