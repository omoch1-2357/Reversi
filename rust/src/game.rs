use crate::board::Board;
use crate::types::{GameResult, GameState, Position};

const BOARD_WIDTH: usize = 8;
const BOARD_LEN: usize = BOARD_WIDTH * BOARD_WIDTH;
pub const PLAYER_BLACK: u8 = 1;
pub const PLAYER_WHITE: u8 = 2;

pub trait MoveSelector: Send + Sync {
    fn select_move(&self, board: &Board, is_black: bool, level: u8) -> Option<usize>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct FirstLegalMoveSelector;

impl MoveSelector for FirstLegalMoveSelector {
    fn select_move(&self, board: &Board, is_black: bool, _level: u8) -> Option<usize> {
        let legal = board.legal_moves(is_black);
        if legal == 0 {
            None
        } else {
            Some(legal.trailing_zeros() as usize)
        }
    }
}

pub struct GameInstance {
    board: Board,
    pub current_player: u8,
    pub level: u8,
    pub is_game_over: bool,
    pub is_pass: bool,
    pub flipped: Vec<u8>,
    evaluator: Box<dyn MoveSelector>,
}

impl GameInstance {
    pub fn new(level: u8, evaluator: Box<dyn MoveSelector>) -> Self {
        Self {
            board: Board::new(),
            current_player: PLAYER_BLACK,
            level,
            is_game_over: false,
            is_pass: false,
            flipped: Vec::new(),
            evaluator,
        }
    }

    pub fn new_with_default_selector(level: u8) -> Self {
        Self::new(level, Box::new(FirstLegalMoveSelector))
    }

    pub fn place(&mut self, row: u8, col: u8) -> Result<(), String> {
        if self.is_game_over {
            return Err("game is already over".to_string());
        }
        if self.current_player != PLAYER_BLACK {
            return Err("it is not the player's turn".to_string());
        }

        let pos = row_col_to_pos(row, col)?;
        self.apply_move(pos, true)
    }

    pub fn has_legal_moves_for_current(&self) -> bool {
        self.board.legal_moves(self.current_player == PLAYER_BLACK) != 0
    }

    pub fn pass(&mut self) {
        self.is_pass = true;
        self.flipped.clear();
        self.current_player = opponent_of(self.current_player);
    }

    pub fn end_game(&mut self) {
        self.is_game_over = true;
    }

    pub fn do_ai_move(&mut self) -> Result<(), String> {
        if self.is_game_over {
            return Err("game is already over".to_string());
        }
        if self.current_player != PLAYER_WHITE {
            return Err("it is not AI's turn".to_string());
        }

        let legal = self.board.legal_moves(false);
        if legal == 0 {
            return Err("AI has no legal moves".to_string());
        }

        let selected = self
            .evaluator
            .select_move(&self.board, false, self.level)
            .ok_or_else(|| "AI could not select a move".to_string())?;

        if selected >= BOARD_LEN {
            return Err("AI selected an out-of-range move".to_string());
        }
        if (legal & (1u64 << selected)) == 0 {
            return Err("AI selected an illegal move".to_string());
        }

        self.apply_move(selected, false)
    }

    pub fn get_legal_moves(&self) -> Vec<Position> {
        let legal = self.board.legal_moves(self.current_player == PLAYER_BLACK);
        bitmask_to_indices(legal)
            .into_iter()
            .map(|idx| Position {
                row: idx / BOARD_WIDTH as u8,
                col: idx % BOARD_WIDTH as u8,
            })
            .collect()
    }

    pub fn to_game_state(&self) -> GameState {
        let (black_count, white_count) = self.board.count();
        GameState {
            board: self.board.to_array().to_vec(),
            current_player: self.current_player,
            black_count,
            white_count,
            is_game_over: self.is_game_over,
            is_pass: self.is_pass,
            flipped: self.flipped.clone(),
        }
    }

    pub fn to_game_result(&self) -> GameResult {
        let (black_count, white_count) = self.board.count();
        GameResult {
            winner: if black_count > white_count {
                PLAYER_BLACK
            } else if white_count > black_count {
                PLAYER_WHITE
            } else {
                0
            },
            black_count,
            white_count,
        }
    }

    fn apply_move(&mut self, pos: usize, is_black: bool) -> Result<(), String> {
        let legal = self.board.legal_moves(is_black);
        if (legal & (1u64 << pos)) == 0 {
            return Err("illegal move".to_string());
        }

        let flips = self.board.place(pos, is_black);
        if flips == 0 {
            return Err("illegal move".to_string());
        }

        self.is_pass = false;
        self.flipped = bitmask_to_indices(flips);
        self.current_player = if is_black { PLAYER_WHITE } else { PLAYER_BLACK };

        if self.board.empty_count() == 0 {
            self.end_game();
        }

        Ok(())
    }

    #[cfg(test)]
    fn set_board_for_test(&mut self, board: Board, current_player: u8) {
        self.board = board;
        self.current_player = current_player;
        self.is_game_over = false;
        self.is_pass = false;
        self.flipped.clear();
    }
}

fn row_col_to_pos(row: u8, col: u8) -> Result<usize, String> {
    if row >= BOARD_WIDTH as u8 || col >= BOARD_WIDTH as u8 {
        return Err("row/col out of range".to_string());
    }
    Ok((row as usize) * BOARD_WIDTH + col as usize)
}

fn bitmask_to_indices(mask: u64) -> Vec<u8> {
    let mut bits = mask;
    let mut out = Vec::new();

    while bits != 0 {
        let idx = bits.trailing_zeros() as u8;
        out.push(idx);
        bits &= bits - 1;
    }

    out
}

fn opponent_of(player: u8) -> u8 {
    match player {
        PLAYER_BLACK => PLAYER_WHITE,
        PLAYER_WHITE => PLAYER_BLACK,
        _ => unreachable!("invalid player value: {}", player),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FULL_BOARD: u64 = u64::MAX;

    struct FixedMoveSelector {
        mv: usize,
    }

    impl MoveSelector for FixedMoveSelector {
        fn select_move(&self, _board: &Board, _is_black: bool, _level: u8) -> Option<usize> {
            Some(self.mv)
        }
    }

    fn bit(row: usize, col: usize) -> u64 {
        1u64 << (row * BOARD_WIDTH + col)
    }

    #[test]
    fn initial_state_is_correct() {
        let game = GameInstance::new_with_default_selector(3);
        let state = game.to_game_state();

        assert_eq!(state.current_player, PLAYER_BLACK);
        assert_eq!(state.black_count, 2);
        assert_eq!(state.white_count, 2);
        assert!(!state.is_game_over);
        assert!(!state.is_pass);
        assert!(state.flipped.is_empty());
        assert_eq!(game.get_legal_moves().len(), 4);
    }

    #[test]
    fn t02_illegal_player_move_returns_error() {
        let mut game = GameInstance::new_with_default_selector(1);
        let err = game.place(0, 0).unwrap_err();

        assert!(err.contains("illegal move"));
    }

    #[test]
    fn t03_pass_occurrence_switches_turn() {
        let mut game = GameInstance::new_with_default_selector(1);
        let black = bit(0, 1);
        let white = FULL_BOARD ^ bit(0, 0) ^ black;
        game.set_board_for_test(Board::from_bitboards(black, white), PLAYER_BLACK);

        assert!(!game.has_legal_moves_for_current());
        game.pass();

        assert_eq!(game.current_player, PLAYER_WHITE);
        assert!(game.is_pass);
        assert!(game.flipped.is_empty());
        assert!(!game.is_game_over);
        assert!(game.has_legal_moves_for_current());
    }

    #[test]
    fn t04_both_passes_end_game() {
        let mut game = GameInstance::new_with_default_selector(1);
        let black = FULL_BOARD ^ bit(0, 0);
        game.set_board_for_test(Board::from_bitboards(black, 0), PLAYER_BLACK);

        assert!(!game.has_legal_moves_for_current());
        game.pass();
        assert_eq!(game.current_player, PLAYER_WHITE);
        assert!(!game.has_legal_moves_for_current());

        game.end_game();
        assert!(game.is_game_over);
    }

    #[test]
    fn t05_full_board_after_move_sets_game_over() {
        let mut game = GameInstance::new(1, Box::new(FixedMoveSelector { mv: 0 }));
        let black = bit(0, 1);
        let white = FULL_BOARD ^ bit(0, 0) ^ black;
        game.set_board_for_test(Board::from_bitboards(black, white), PLAYER_WHITE);

        game.do_ai_move().unwrap();
        let state = game.to_game_state();

        assert!(state.is_game_over);
        assert_eq!(state.current_player, PLAYER_BLACK);
        assert_eq!(state.black_count, 0);
        assert_eq!(state.white_count, 64);
        assert_eq!(state.flipped, vec![1]);
    }
}
