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
    use crate::ai::ntuple::NTupleEvaluator;
    use crate::ai::search::Searcher;
    use crate::board::Board;
    use crate::game::{GameInstance, MoveSelector};
    use crate::types::{GameState, Position};

    const MAX_GAME_STEPS: usize = 200;
    const AI_MOVE_LIMIT_MS: f64 = 3_000.0;
    const ERROR_GAME_NOT_INITIALIZED: &str = "game is not initialized";
    const ERROR_PLAYER_TURN: &str = "it is not the player's turn";

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

    #[wasm_bindgen_test]
    fn api_returns_uninitialized_error_before_init_game() {
        clear_game();

        expect_err_message(get_result(), ERROR_GAME_NOT_INITIALIZED);
        expect_err_message(get_legal_moves(), ERROR_GAME_NOT_INITIALIZED);
        expect_err_message(place_stone(2, 3), ERROR_GAME_NOT_INITIALIZED);
        expect_err_message(ai_move(), ERROR_GAME_NOT_INITIALIZED);
    }

    #[wasm_bindgen_test]
    fn place_stone_rejects_wrong_player_turn() {
        init_game(1).expect("init_game must succeed");
        let opening = first_internal_legal_move().expect("opening move must exist");
        place_stone(opening.row, opening.col).expect("first player move must succeed");

        expect_err_message(place_stone(opening.row, opening.col), ERROR_PLAYER_TURN);
    }

    #[wasm_bindgen_test]
    fn ai_tie_break_prefers_smallest_index_and_is_deterministic() {
        let first = run_tie_break_step();
        let second = run_tie_break_step();

        assert_eq!(first.expected_index, second.expected_index);
        assert_eq!(first.chosen_index, first.expected_index);
        assert_eq!(second.chosen_index, second.expected_index);
        assert_eq!(first.chosen_index, second.chosen_index);
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

    fn clear_game() {
        let mut guard = GAME.lock().expect("game lock must not be poisoned");
        *guard = None;
    }

    fn expect_err_message(result: Result<JsValue, JsValue>, expected: &str) {
        let err = result.expect_err("operation should fail");
        let message = err
            .as_string()
            .expect("error should be representable as string");
        assert!(
            message.contains(expected),
            "expected error to contain '{expected}', got '{message}'"
        );
    }

    #[derive(Debug, Clone, Copy)]
    struct TieBreakResult {
        chosen_index: usize,
        expected_index: usize,
    }

    struct TieBreakSelector {
        evaluator: NTupleEvaluator,
    }

    impl MoveSelector for TieBreakSelector {
        fn select_move(&self, board: &Board, is_black: bool, level: u8) -> Option<usize> {
            let legal = board.legal_moves(is_black);
            if legal == 0 {
                return None;
            }

            let mut searcher = Searcher::new(&self.evaluator, level);
            Some(searcher.search(board, is_black))
        }
    }

    fn run_tie_break_step() -> TieBreakResult {
        let mut game = GameInstance::new(
            1,
            Box::new(TieBreakSelector {
                evaluator: build_constant_evaluator(),
            }),
        );
        game.place(2, 3)
            .expect("fixed opening move should be legal");

        let legal = game.get_legal_moves();
        assert!(
            legal.len() > 1,
            "tie-break scenario requires at least two legal AI moves"
        );
        let expected_index = legal
            .iter()
            .map(position_to_index)
            .min()
            .expect("expected at least one legal move");

        let before = game.to_game_state().board;
        {
            let mut guard = GAME.lock().expect("game lock must not be poisoned");
            *guard = Some(game);
        }

        ai_move().expect("ai_move must succeed");
        let after = snapshot_state().board;
        let chosen_index = placed_white_index(&before, &after)
            .expect("exactly one newly placed white stone should exist");

        TieBreakResult {
            chosen_index,
            expected_index,
        }
    }

    fn build_constant_evaluator() -> NTupleEvaluator {
        let mut payload = Vec::new();
        payload.push(1u8);
        payload.push(0u8);
        for _ in 0..3 {
            payload.extend_from_slice(&0.0f32.to_le_bytes());
        }

        let crc = crc32fast::hash(&payload);
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"NTRV");
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&crc.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&payload);

        NTupleEvaluator::from_bytes(&bytes).expect("constant evaluator must deserialize")
    }

    fn position_to_index(pos: &Position) -> usize {
        (pos.row as usize) * 8 + pos.col as usize
    }

    fn placed_white_index(before: &[u8], after: &[u8]) -> Option<usize> {
        let mut placed = None;
        for (idx, (&prev, &next)) in before.iter().zip(after.iter()).enumerate() {
            if prev == 0 && next == PLAYER_WHITE {
                if placed.is_some() {
                    return None;
                }
                placed = Some(idx);
            }
        }
        placed
    }
}
