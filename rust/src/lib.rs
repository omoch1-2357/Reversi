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
        check_and_handle_pass(game);
    } else {
        game.do_ai_move().map_err(string_to_js)?;

        // F-05: auto-pass the player if they have no legal moves.
        if !game.is_game_over {
            check_and_handle_pass(game);
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

fn check_and_handle_pass(game: &mut GameInstance) {
    if !game.has_legal_moves_for_current() {
        game.pass();
        if !game.has_legal_moves_for_current() {
            game.end_game();
        }
    }
}

fn to_js_value<T: serde::Serialize>(value: &T) -> Result<JsValue, JsValue> {
    serde_wasm_bindgen::to_value(value)
        .map_err(|e| JsValue::from_str(&format!("serialization error: {e}")))
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

    use super::*;
    use crate::ai::ntuple::NTupleEvaluator;
    use crate::game::GameInstance;
    use crate::types::{GameState, Position};

    wasm_bindgen_test_configure!(run_in_browser);

    const MAX_GAME_STEPS: usize = 200;
    const AI_MOVE_LIMIT_MS: f64 = 3_000.0;
    const ERROR_GAME_NOT_INITIALIZED: &str = "game is not initialized";
    const ERROR_PLAYER_TURN: &str = "it is not the player's turn";
    const ERROR_INVALID_LEVEL: &str = "level must be in 1..=6";

    const T11_BLACK: u64 = 0xffc3_e7b9_98c8_80bf;
    const T11_WHITE: u64 = 0x003c_1846_6736_7f40;

    const T12_BLACK: u64 = 0x7a0e_123f_4981_0101;
    const T12_WHITE: u64 = 0x01f1_6d40_161e_0e1e;
    const T12_WHITE_LEGAL_MASK: u64 = 0x0400_0000_2040_0000;
    const AI_MOVE_TIMEOUT_MS: f64 = 5_000.0;
    const PERFORMANCE_SAMPLE_COUNT: usize = 100;
    const PERFORMANCE_WARMUP_COUNT: usize = 5;
    const PERFORMANCE_MOVE_MIN: usize = 20;
    const PERFORMANCE_MOVE_MAX: usize = 40;

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
    fn init_game_rejects_out_of_range_levels() {
        expect_err_message(init_game(0), ERROR_INVALID_LEVEL);
        expect_err_message(init_game(7), ERROR_INVALID_LEVEL);
    }

    #[wasm_bindgen_test]
    fn get_result_returns_error_while_game_is_active() {
        init_game(1).expect("init_game must succeed");
        expect_err_message(get_result(), "game is not over");
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
    #[ignore]
    fn ai_move_p95_meets_requirement_for_seeded_midgame_positions() {
        let positions = generate_seeded_midgame_positions(PERFORMANCE_SAMPLE_COUNT, 42);

        for sample in positions.iter().take(PERFORMANCE_WARMUP_COUNT) {
            let elapsed_ms = measure_ai_move_from_position(1, *sample);
            assert!(
                elapsed_ms < AI_MOVE_TIMEOUT_MS,
                "warm-up ai_move exceeded 5s timeout guard: {elapsed_ms} ms"
            );
        }

        for level in MIN_LEVEL..=MAX_LEVEL {
            let mut durations_ms = Vec::with_capacity(PERFORMANCE_SAMPLE_COUNT);
            for sample in &positions {
                let elapsed_ms = measure_ai_move_from_position(level, *sample);
                assert!(
                    elapsed_ms < AI_MOVE_TIMEOUT_MS,
                    "ai_move exceeded 5s timeout guard at level {level}: {elapsed_ms} ms"
                );
                durations_ms.push(elapsed_ms);
            }

            let p95_ms = percentile_95(&durations_ms);
            assert!(
                p95_ms < AI_MOVE_LIMIT_MS,
                "level {level} p95 exceeded 3s target: {p95_ms} ms"
            );
        }
    }

    #[wasm_bindgen_test]
    fn init_game_reinitializes_global_state() {
        init_game(1).expect("first init_game must succeed");
        let opening = first_internal_legal_move().expect("opening move must exist");
        place_stone(opening.row, opening.col).expect("player move must succeed");
        assert_ne!(
            snapshot_state().black_count,
            2,
            "state should change before reset"
        );

        init_game(6).expect("second init_game must succeed");
        let reset = snapshot_state();
        assert_eq!(reset.current_player, PLAYER_BLACK);
        assert_eq!(reset.black_count, 2);
        assert_eq!(reset.white_count, 2);
        assert!(!reset.is_game_over);
        assert!(!reset.is_pass);
        assert!(reset.flipped.is_empty());
    }

    #[wasm_bindgen_test]
    fn ai_move_passes_when_ai_has_no_legal_moves() {
        inject_test_position(3, T11_BLACK, T11_WHITE, PLAYER_WHITE);
        let before = snapshot_state().board;

        ai_move().expect("ai_move should handle pass position");
        let after = snapshot_state();

        assert_eq!(after.board, before, "board must stay unchanged on pass");
        assert_eq!(after.current_player, PLAYER_BLACK);
        assert!(after.is_pass);
        assert!(!after.is_game_over);
        assert!(after.flipped.is_empty());
    }

    #[wasm_bindgen_test]
    fn exact_solve_timeout_returns_fallback_move() {
        let _guard = ForcedExactSolveTimeoutGuard::new();
        inject_test_position(6, T12_BLACK, T12_WHITE, PLAYER_WHITE);
        let before = snapshot_state().board;

        ai_move().expect("ai_move should return fallback move after timeout");
        let after = snapshot_state();

        let placed =
            placed_white_index(&before, &after.board).expect("one white stone must be placed");
        assert_ne!(
            T12_WHITE_LEGAL_MASK & (1u64 << placed),
            0,
            "fallback move must still be legal"
        );
        assert!(after.white_count > 0);
    }

    #[wasm_bindgen_test]
    fn ai_move_is_deterministic_for_100_repeated_runs() {
        let expected = ai_move_index_after_opening(4);
        for _ in 0..99 {
            let mv = ai_move_index_after_opening(4);
            assert_eq!(
                mv, expected,
                "AI move must stay deterministic across 100 runs"
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

    fn ai_move_index_after_opening(level: u8) -> usize {
        init_game(level).expect("init_game must succeed");
        let opening = first_internal_legal_move().expect("opening move must exist");
        place_stone(opening.row, opening.col).expect("place_stone must succeed");
        let before = snapshot_state().board;
        ai_move().expect("ai_move must succeed");
        let after = snapshot_state().board;
        placed_white_index(&before, &after).expect("exactly one white stone should be newly placed")
    }

    fn measure_ai_move_from_position(level: u8, sample: PerformanceSample) -> f64 {
        inject_test_board(level, sample.board, sample.current_player);
        let start_ms = js_sys::Date::now();
        ai_move().expect("ai_move must succeed for generated performance position");
        js_sys::Date::now() - start_ms
    }

    fn generate_seeded_midgame_positions(sample_count: usize, seed: u64) -> Vec<PerformanceSample> {
        let mut rng = Lcg::new(seed);
        let mut positions = Vec::with_capacity(sample_count);

        for game_index in 0..sample_count {
            let sample = generate_seeded_midgame_position(&mut rng).unwrap_or_else(|| {
                panic!(
                    "random play game {game_index} did not yield a white-to-move position within plies {PERFORMANCE_MOVE_MIN}..={PERFORMANCE_MOVE_MAX}"
                )
            });
            positions.push(sample);
        }

        positions
    }

    fn generate_seeded_midgame_position(rng: &mut Lcg) -> Option<PerformanceSample> {
        let mut board = Board::new();
        let mut current_player = PLAYER_BLACK;
        let mut plies = 0usize;
        let mut candidates = Vec::new();

        for _ in 0..MAX_GAME_STEPS {
            let legal = board.legal_moves(current_player == PLAYER_BLACK);
            if legal == 0 {
                let opponent = opposing_player(current_player);
                if board.legal_moves(opponent == PLAYER_BLACK) == 0 {
                    break;
                }
                current_player = opponent;
                maybe_collect_performance_sample(&mut candidates, board, current_player, plies);
                continue;
            }

            let legal_moves = bitboard_to_positions(legal);
            let mv = legal_moves[rng.next_usize(legal_moves.len())];
            let flips = board.place(mv, current_player == PLAYER_BLACK);
            assert_ne!(flips, 0, "generated random move must stay legal");

            plies += 1;
            current_player = opposing_player(current_player);
            maybe_collect_performance_sample(&mut candidates, board, current_player, plies);

            if board.legal_moves(current_player == PLAYER_BLACK) == 0 {
                let opponent = opposing_player(current_player);
                if board.legal_moves(opponent == PLAYER_BLACK) == 0 {
                    break;
                }
                current_player = opponent;
                maybe_collect_performance_sample(&mut candidates, board, current_player, plies);
            }
        }

        if candidates.is_empty() {
            None
        } else {
            Some(candidates[rng.next_usize(candidates.len())])
        }
    }

    fn maybe_collect_performance_sample(
        candidates: &mut Vec<PerformanceSample>,
        board: Board,
        current_player: u8,
        plies: usize,
    ) {
        if !(PERFORMANCE_MOVE_MIN..=PERFORMANCE_MOVE_MAX).contains(&plies) {
            return;
        }
        if current_player != PLAYER_WHITE {
            return;
        }
        if board.legal_moves(false) == 0 {
            return;
        }

        candidates.push(PerformanceSample {
            board,
            current_player,
        });
    }

    fn inject_test_position(level: u8, black: u64, white: u64, current_player: u8) {
        inject_test_board(level, Board::from_bitboards(black, white), current_player);
    }

    fn inject_test_board(level: u8, board: Board, current_player: u8) {
        let evaluator = NTupleEvaluator::from_bytes(MODEL_BYTES)
            .expect("embedded model bytes must deserialize for wasm tests");
        let mut game = GameInstance::new(level, Box::new(SearchMoveSelector::new(evaluator)));
        game.set_board_for_test(board, current_player);
        let mut guard = GAME.lock().expect("game lock must not be poisoned");
        *guard = Some(game);
    }

    struct ForcedExactSolveTimeoutGuard;

    impl ForcedExactSolveTimeoutGuard {
        fn new() -> Self {
            crate::ai::search::set_force_exact_solve_timeout_for_test(true);
            Self
        }
    }

    impl Drop for ForcedExactSolveTimeoutGuard {
        fn drop(&mut self) {
            crate::ai::search::set_force_exact_solve_timeout_for_test(false);
        }
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
    struct PerformanceSample {
        board: Board,
        current_player: u8,
    }

    struct Lcg {
        state: u64,
    }

    impl Lcg {
        fn new(seed: u64) -> Self {
            Self { state: seed }
        }

        fn next_u32(&mut self) -> u32 {
            self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1);
            (self.state >> 32) as u32
        }

        fn next_usize(&mut self, upper_bound: usize) -> usize {
            assert!(upper_bound > 0, "upper_bound must be positive");
            (self.next_u32() as usize) % upper_bound
        }
    }

    #[derive(Debug, Clone, Copy)]
    struct TieBreakResult {
        chosen_index: usize,
        expected_index: usize,
    }

    fn run_tie_break_step() -> TieBreakResult {
        let mut game = GameInstance::new(
            1,
            Box::new(SearchMoveSelector::new(build_constant_evaluator())),
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

    fn percentile_95(samples: &[f64]) -> f64 {
        assert!(!samples.is_empty(), "samples must not be empty");

        let mut sorted = samples.to_vec();
        sorted.sort_by(f64::total_cmp);
        let rank = sorted
            .len()
            .saturating_mul(95)
            .div_ceil(100)
            .saturating_sub(1);
        sorted[rank]
    }

    fn bitboard_to_positions(mut mask: u64) -> Vec<usize> {
        let mut positions = Vec::new();

        while mask != 0 {
            let pos = mask.trailing_zeros() as usize;
            positions.push(pos);
            mask &= mask - 1;
        }

        positions
    }

    fn opposing_player(player: u8) -> u8 {
        if player == PLAYER_BLACK {
            PLAYER_WHITE
        } else {
            PLAYER_BLACK
        }
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
