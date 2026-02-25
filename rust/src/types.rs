use serde::Serialize;

/// A board coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Position {
    pub row: u8,
    pub col: u8,
}

/// Public game state returned from WASM APIs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GameState {
    pub board: Vec<u8>,
    pub current_player: u8,
    pub black_count: u8,
    pub white_count: u8,
    pub is_game_over: bool,
    /// Contract:
    /// - `true` when the previous action was a pass.
    /// - `false` when the previous action was a normal move.
    pub is_pass: bool,
    /// Contract:
    /// - Normal move: list of flipped positions (0..=63).
    /// - Pass: must be an empty list.
    pub flipped: Vec<u8>,
}

/// Final result after game over.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct GameResult {
    pub winner: u8,
    pub black_count: u8,
    pub white_count: u8,
}
