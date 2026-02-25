use wasm_bindgen::prelude::*;

pub mod board;
pub mod types;

#[wasm_bindgen]
pub fn wasm_ready() -> bool {
    true
}
