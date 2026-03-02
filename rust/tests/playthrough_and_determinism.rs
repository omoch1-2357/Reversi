use reversi::ai::ntuple::NTupleEvaluator;
use reversi::ai::search::Searcher;
use reversi::board::Board;
use reversi::game::{GameInstance, MoveSelector, PLAYER_BLACK, PLAYER_WHITE};
use reversi::types::Position;

const MODEL_BYTES: &[u8] = include_bytes!("../src/ai/weights.bin");
const MAX_GAME_STEPS: usize = 200;

#[derive(Clone)]
struct SearchBackedSelector {
    evaluator: NTupleEvaluator,
}

impl SearchBackedSelector {
    fn from_embedded_model() -> Self {
        let evaluator = NTupleEvaluator::from_bytes(MODEL_BYTES)
            .expect("embedded model bytes must deserialize for integration tests");
        Self { evaluator }
    }
}

impl MoveSelector for SearchBackedSelector {
    fn select_move(&self, board: &Board, is_black: bool, level: u8) -> Option<usize> {
        let legal = board.legal_moves(is_black);
        if legal == 0 {
            return None;
        }

        let mut searcher = Searcher::new(&self.evaluator, level);
        Some(searcher.search(board, is_black))
    }
}

fn first_legal_move(game: &GameInstance) -> Option<Position> {
    game.get_legal_moves().into_iter().next()
}

fn handle_pass_if_needed(game: &mut GameInstance) {
    if !game.has_legal_moves_for_current() {
        game.pass();
        if !game.has_legal_moves_for_current() {
            game.end_game();
        }
    }
}

fn assert_winner_consistency(black: u8, white: u8, winner: u8) {
    if black > white {
        assert_eq!(winner, PLAYER_BLACK);
    } else if white > black {
        assert_eq!(winner, PLAYER_WHITE);
    } else {
        assert_eq!(winner, 0);
    }
}

#[test]
fn playthrough_init_place_ai_get_result_flow_works() {
    let mut game = GameInstance::new(3, Box::new(SearchBackedSelector::from_embedded_model()));

    // init
    assert_eq!(game.current_player, PLAYER_BLACK);
    assert!(!game.is_game_over);

    // place
    let opening = first_legal_move(&game).expect("initial player move must exist");
    game.place(opening.row, opening.col)
        .expect("opening player move must succeed");
    assert_eq!(game.current_player, PLAYER_WHITE);

    // ai_move
    game.do_ai_move().expect("first AI move must succeed");

    // Continue until game over, mirroring the Rust-side pass handling behavior.
    for _ in 0..MAX_GAME_STEPS {
        if game.is_game_over {
            break;
        }

        if game.current_player == PLAYER_BLACK {
            if let Some(mv) = first_legal_move(&game) {
                game.place(mv.row, mv.col)
                    .expect("player legal move should always succeed");
            } else {
                handle_pass_if_needed(&mut game);
            }
        } else {
            if game.has_legal_moves_for_current() {
                game.do_ai_move().expect("AI legal move should succeed");
            } else {
                handle_pass_if_needed(&mut game);
            }
            if !game.is_game_over {
                handle_pass_if_needed(&mut game);
            }
        }
    }

    assert!(
        game.is_game_over,
        "game did not finish within {MAX_GAME_STEPS} steps"
    );

    // get_result
    let state = game.to_game_state();
    let result = game.to_game_result();
    assert_eq!(state.black_count, result.black_count);
    assert_eq!(state.white_count, result.white_count);
    assert_winner_consistency(result.black_count, result.white_count, result.winner);
}

#[test]
fn search_is_deterministic_with_embedded_model() {
    let evaluator = NTupleEvaluator::from_bytes(MODEL_BYTES)
        .expect("embedded model bytes must deserialize for integration tests");
    let board = Board::new();
    let legal = board.legal_moves(true);

    let mut expected = None;
    for _ in 0..16 {
        let mut searcher = Searcher::new(&evaluator, 4);
        let mv = searcher.search(&board, true);
        assert_ne!(legal & (1u64 << mv), 0, "search must return a legal move");
        assert!(!searcher.timed_out(), "determinism test must not timeout");
        if let Some(first) = expected {
            assert_eq!(mv, first, "search must be deterministic");
        } else {
            expected = Some(mv);
        }
    }
}
