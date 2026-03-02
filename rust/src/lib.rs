use std::sync::Mutex;

use once_cell::sync::Lazy;
use wasm_bindgen::prelude::*;

use crate::ai::ntuple::NTupleEvaluator;
use crate::ai::search::Searcher;
use crate::board::Board;
use crate::game::{GameInstance, MoveSelector};
pub use crate::game::{PLAYER_BLACK, PLAYER_WHITE};

pub mod ai;
pub mod board;
pub mod game;
pub mod types;

const MIN_LEVEL: u8 = 1;
const MAX_LEVEL: u8 = 6;

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
    let state = instance.to_game_state();
    *guard = Some(instance);
    to_js_value(&state)
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

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use wasm_bindgen_test::wasm_bindgen_test;

    use super::*;
    use crate::types::GameState;

    const MAX_GAME_STEPS: usize = 200;
    const AI_MOVE_LIMIT_MS: f64 = 3_000.0;

    #[wasm_bindgen_test]
    fn api_flow_init_place_ai_get_result_works_end_to_end() {
        init_game(1).expect("init_game must succeed");
        assert!(
            get_legal_moves().is_ok(),
            "get_legal_moves must succeed right after init"
        );
        assert!(
            get_result().is_err(),
            "get_result must fail before game over"
        );

        let opening = first_internal_legal_move().expect("player should have an opening move");
        place_stone(opening.row, opening.col).expect("place_stone must succeed on legal move");

        loop_until_game_over();

        assert!(
            get_result().is_ok(),
            "get_result must succeed after game over"
        );
    }

    #[wasm_bindgen_test]
    fn ai_move_is_deterministic_for_same_level_and_position() {
        let first = play_one_opening_and_ai_step(3);
        let second = play_one_opening_and_ai_step(3);

        assert_eq!(
            first, second,
            "same level and position must yield same AI step"
        );
    }

    #[wasm_bindgen_test]
    fn ai_move_smoke_meets_level_performance_target() {
        for level in MIN_LEVEL..=MAX_LEVEL {
            init_game(level).expect("init_game must succeed");
            let opening = first_internal_legal_move().expect("opening move must exist");
            place_stone(opening.row, opening.col).expect("place_stone must succeed");

            let start_ms = js_sys::Date::now();
            ai_move().expect("ai_move must succeed");
            let elapsed_ms = js_sys::Date::now() - start_ms;
            assert!(
                elapsed_ms < AI_MOVE_LIMIT_MS,
                "ai_move exceeded 3s target at level {level}: {elapsed_ms} ms"
            );
        }
    }

    fn play_one_opening_and_ai_step(level: u8) -> GameState {
        init_game(level).expect("init_game must succeed");
        let opening = first_internal_legal_move().expect("opening move must exist");
        place_stone(opening.row, opening.col).expect("place_stone must succeed");
        ai_move().expect("ai_move must succeed");
        snapshot_state()
    }

    fn loop_until_game_over() {
        for _ in 0..MAX_GAME_STEPS {
            let (is_game_over, current_player) = current_state_markers();
            if is_game_over {
                return;
            }

            assert!(
                get_legal_moves().is_ok(),
                "get_legal_moves must succeed while game is active"
            );

            if current_player == PLAYER_BLACK {
                let mv = first_internal_legal_move().expect(
                    "player turn must have legal move (auto-pass is AI side responsibility)",
                );
                place_stone(mv.row, mv.col).expect("player legal move must succeed");
            } else {
                let start_ms = js_sys::Date::now();
                ai_move().expect("ai_move must succeed on AI turn");
                let elapsed_ms = js_sys::Date::now() - start_ms;
                assert!(
                    elapsed_ms < AI_MOVE_LIMIT_MS,
                    "ai_move exceeded 3s target during flow: {elapsed_ms} ms"
                );
            }
        }

        panic!("game did not finish within {MAX_GAME_STEPS} steps");
    }

    fn first_internal_legal_move() -> Option<crate::types::Position> {
        let guard = GAME.lock().expect("game lock must not be poisoned");
        guard.as_ref()?.get_legal_moves().into_iter().next()
    }

    fn current_state_markers() -> (bool, u8) {
        let guard = GAME.lock().expect("game lock must not be poisoned");
        let game = guard
            .as_ref()
            .expect("game must be initialized before checking state");
        (game.is_game_over, game.current_player)
    }

    fn snapshot_state() -> GameState {
        let guard = GAME.lock().expect("game lock must not be poisoned");
        guard
            .as_ref()
            .expect("game must be initialized before snapshot")
            .to_game_state()
    }
}
