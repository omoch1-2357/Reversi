use std::sync::Mutex;

use once_cell::sync::Lazy;
use wasm_bindgen::prelude::*;

use crate::ai::ntuple::NTupleEvaluator;
use crate::ai::search::Searcher;
use crate::board::Board;
use crate::game::{GameInstance, MoveSelector};

pub mod ai;
pub mod board;
pub mod game;
pub mod types;

const MIN_LEVEL: u8 = 1;
const MAX_LEVEL: u8 = 6;
const PLAYER_BLACK: u8 = 1;
const PLAYER_WHITE: u8 = 2;

static MODEL_BYTES: &[u8] = include_bytes!("ai/weights.bin");
static GAME: Lazy<Mutex<Option<GameInstance>>> = Lazy::new(|| Mutex::new(None));

struct SearchMoveSelector {
    evaluator: NTupleEvaluator,
}

impl SearchMoveSelector {
    fn new(evaluator: NTupleEvaluator) -> Self {
        Self { evaluator }
    }
}

impl MoveSelector for SearchMoveSelector {
    fn select_move(&self, board: &Board, is_black: bool, level: u8) -> Option<usize> {
        let legal = board.legal_moves(is_black);
        if legal == 0 {
            return None;
        }

        let mut searcher = Searcher::new(&self.evaluator, level);
        Some(searcher.search(board, is_black))
    }
}

#[wasm_bindgen]
pub fn wasm_ready() -> bool {
    true
}

#[wasm_bindgen]
pub fn init_game(level: u8) -> Result<JsValue, JsValue> {
    if !(MIN_LEVEL..=MAX_LEVEL).contains(&level) {
        return Err(JsValue::from_str("level must be in 1..=6"));
    }

    let evaluator = NTupleEvaluator::from_bytes(MODEL_BYTES).map_err(string_to_js)?;
    let instance = GameInstance::new(level, Box::new(SearchMoveSelector::new(evaluator)));

    let mut guard = GAME
        .lock()
        .map_err(|_| JsValue::from_str("failed to lock game state"))?;
    *guard = Some(instance);
    let game = guard
        .as_ref()
        .ok_or_else(|| JsValue::from_str("failed to initialize game"))?;

    to_js_value(&game.to_game_state())
}

#[wasm_bindgen]
pub fn get_legal_moves() -> Result<JsValue, JsValue> {
    let guard = GAME
        .lock()
        .map_err(|_| JsValue::from_str("failed to lock game state"))?;
    let game = guard
        .as_ref()
        .ok_or_else(|| JsValue::from_str("game is not initialized"))?;

    to_js_value(&game.get_legal_moves())
}

#[wasm_bindgen]
pub fn place_stone(row: u8, col: u8) -> Result<JsValue, JsValue> {
    let mut guard = GAME
        .lock()
        .map_err(|_| JsValue::from_str("failed to lock game state"))?;
    let game = guard
        .as_mut()
        .ok_or_else(|| JsValue::from_str("game is not initialized"))?;

    if game.is_game_over {
        return Err(JsValue::from_str("game is already over"));
    }
    if game.current_player != PLAYER_BLACK {
        return Err(JsValue::from_str("it is not the player's turn"));
    }

    game.place(row, col).map_err(string_to_js)?;
    to_js_value(&game.to_game_state())
}

#[wasm_bindgen]
pub fn ai_move() -> Result<JsValue, JsValue> {
    let mut guard = GAME
        .lock()
        .map_err(|_| JsValue::from_str("failed to lock game state"))?;
    let game = guard
        .as_mut()
        .ok_or_else(|| JsValue::from_str("game is not initialized"))?;

    if game.is_game_over {
        return Err(JsValue::from_str("game is already over"));
    }
    if game.current_player != PLAYER_WHITE {
        return Err(JsValue::from_str("it is not AI's turn"));
    }

    // Execute exactly one AI step. Worker-side loop handles repeated calls.
    if !game.has_legal_moves_for_current() {
        game.pass();
        if !game.has_legal_moves_for_current() {
            game.end_game();
        }
    } else {
        game.do_ai_move().map_err(string_to_js)?;

        // F-05: auto-pass the player if they have no legal moves.
        if !game.is_game_over && !game.has_legal_moves_for_current() {
            game.pass();
            if !game.has_legal_moves_for_current() {
                game.end_game();
            }
        }
    }

    to_js_value(&game.to_game_state())
}

#[wasm_bindgen]
pub fn get_result() -> Result<JsValue, JsValue> {
    let guard = GAME
        .lock()
        .map_err(|_| JsValue::from_str("failed to lock game state"))?;
    let game = guard
        .as_ref()
        .ok_or_else(|| JsValue::from_str("game is not initialized"))?;
    if !game.is_game_over {
        return Err(JsValue::from_str("game is not over"));
    }

    to_js_value(&game.to_game_result())
}

fn string_to_js(message: String) -> JsValue {
    JsValue::from_str(&message)
}

fn to_js_value<T: serde::Serialize>(value: &T) -> Result<JsValue, JsValue> {
    serde_wasm_bindgen::to_value(value)
        .map_err(|e| JsValue::from_str(&format!("serialization error: {e}")))
}
