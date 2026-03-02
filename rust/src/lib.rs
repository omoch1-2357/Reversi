use wasm_bindgen::prelude::*;

pub mod ai;
pub mod board;
pub mod game;
pub mod types;

#[wasm_bindgen]
pub fn wasm_ready() -> bool {
    true
}
