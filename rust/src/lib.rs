use wasm_bindgen::prelude::*;

pub mod board;

#[wasm_bindgen]
pub fn wasm_ready() -> bool {
    true
}
