use wasm_bindgen::prelude::*;

mod arctan;
mod block_map;
mod constants;
mod lighting;
mod ray;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);

    #[wasm_bindgen(js_name = IsBlocked)]
    fn is_blocked(x0: i16, y0: i16, x1: i16, y1: i16) -> bool;
}

#[wasm_bindgen(start)]
pub fn start() {
    lighting::init();
}

#[wasm_bindgen]
pub fn put(id: u8, r: i16, x: i16, y: i16) -> *const lighting::Color {
    lighting::update_or_add_light(id, r, x, y)
}

#[wasm_bindgen]
pub fn get_tiles() -> *const u8 {
    block_map::get_tiles()
}

#[wasm_bindgen]
pub fn get_blockmap() -> *const block_map::CellDetails {
    block_map::get_blockmap()
}

#[wasm_bindgen]
pub fn set_tile(x: u32, y: u32, tile: u8) {
    block_map::set_tile(x, y, tile);
} 