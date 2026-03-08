const BOARD_SIZE: usize = 8;
const NUM_SQUARES: usize = BOARD_SIZE * BOARD_SIZE;
const NOT_A_FILE: u64 = 0xfefefefefefefefe;
const NOT_H_FILE: u64 = 0x7f7f7f7f7f7f7f7f;

/// Reversi board state represented by two bitboards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Board {
    black: u64,
    white: u64,
}

impl Board {
    /// Creates the initial board:
    /// d4=white, e4=black, d5=black, e5=white.
    pub fn new() -> Self {
        Self {
            black: bit(28) | bit(35),
            white: bit(27) | bit(36),
        }
    }

    #[cfg(test)]
    pub(crate) fn from_bitboards(black: u64, white: u64) -> Self {
        debug_assert_eq!(black & white, 0);
        Self { black, white }
    }

    /// Returns legal move mask for the given side.
    pub fn legal_moves(&self, is_black: bool) -> u64 {
        let (me, opp) = if is_black {
            (self.black, self.white)
        } else {
            (self.white, self.black)
        };
        let empty = !(me | opp);

        legal_moves_dir(me, opp, empty, shift_east)
            | legal_moves_dir(me, opp, empty, shift_west)
            | legal_moves_dir(me, opp, empty, shift_north)
            | legal_moves_dir(me, opp, empty, shift_south)
            | legal_moves_dir(me, opp, empty, shift_north_east)
            | legal_moves_dir(me, opp, empty, shift_north_west)
            | legal_moves_dir(me, opp, empty, shift_south_east)
            | legal_moves_dir(me, opp, empty, shift_south_west)
    }

    /// Places one stone and flips captured stones.
    /// Returns flipped bit mask. Returns 0 when move is illegal.
    pub fn place(&mut self, pos: usize, is_black: bool) -> u64 {
        let (me, opp) = if is_black {
            (self.black, self.white)
        } else {
            (self.white, self.black)
        };

        let flips = Self::collect_flips(pos, me, opp);
        if flips == 0 {
            return 0;
        }

        let move_bit = bit(pos);
        let next_me = me | move_bit | flips;
        let next_opp = opp & !flips;

        if is_black {
            self.black = next_me;
            self.white = next_opp;
        } else {
            self.white = next_me;
            self.black = next_opp;
        }

        flips
    }

    /// Returns `(black_count, white_count)`.
    pub fn count(&self) -> (u8, u8) {
        (self.black.count_ones() as u8, self.white.count_ones() as u8)
    }

    /// Returns the number of empty squares.
    pub fn empty_count(&self) -> u8 {
        let (black_count, white_count) = self.count();
        NUM_SQUARES as u8 - black_count - white_count
    }

    /// Converts board to `[u8; 64]` where 0=empty, 1=black, 2=white.
    pub fn to_array(&self) -> [u8; NUM_SQUARES] {
        let mut board = [0u8; NUM_SQUARES];
        for (pos, cell) in board.iter_mut().enumerate() {
            let square = bit(pos);
            *cell = if (self.black & square) != 0 {
                1
            } else if (self.white & square) != 0 {
                2
            } else {
                0
            };
        }
        board
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn bitboards(&self) -> (u64, u64) {
        (self.black, self.white)
    }

    fn collect_flips(pos: usize, me: u64, opp: u64) -> u64 {
        if pos >= NUM_SQUARES {
            return 0;
        }

        let move_bit = bit(pos);
        if ((me | opp) & move_bit) != 0 {
            return 0;
        }

        flips_dir(move_bit, me, opp, shift_east)
            | flips_dir(move_bit, me, opp, shift_west)
            | flips_dir(move_bit, me, opp, shift_north)
            | flips_dir(move_bit, me, opp, shift_south)
            | flips_dir(move_bit, me, opp, shift_north_east)
            | flips_dir(move_bit, me, opp, shift_north_west)
            | flips_dir(move_bit, me, opp, shift_south_east)
            | flips_dir(move_bit, me, opp, shift_south_west)
    }
}

impl Default for Board {
    fn default() -> Self {
        Self::new()
    }
}

fn bit(pos: usize) -> u64 {
    if pos < NUM_SQUARES { 1u64 << pos } else { 0 }
}

fn legal_moves_dir(me: u64, opp: u64, empty: u64, shift: fn(u64) -> u64) -> u64 {
    let mut ray = shift(me) & opp;
    for _ in 0..5 {
        ray |= shift(ray) & opp;
    }
    shift(ray) & empty
}

fn flips_dir(move_bit: u64, me: u64, opp: u64, shift: fn(u64) -> u64) -> u64 {
    let mut cursor = shift(move_bit) & opp;
    let mut flips = 0u64;

    while cursor != 0 {
        flips |= cursor;
        let next = shift(cursor);
        if (next & me) != 0 {
            return flips;
        }
        cursor = next & opp;
    }

    0
}

fn shift_east(bits: u64) -> u64 {
    (bits & NOT_H_FILE) << 1
}

fn shift_west(bits: u64) -> u64 {
    (bits & NOT_A_FILE) >> 1
}

fn shift_north(bits: u64) -> u64 {
    bits >> BOARD_SIZE
}

fn shift_south(bits: u64) -> u64 {
    bits << BOARD_SIZE
}

fn shift_north_east(bits: u64) -> u64 {
    (bits & NOT_H_FILE) >> (BOARD_SIZE - 1)
}

fn shift_north_west(bits: u64) -> u64 {
    (bits & NOT_A_FILE) >> (BOARD_SIZE + 1)
}

fn shift_south_east(bits: u64) -> u64 {
    (bits & NOT_H_FILE) << (BOARD_SIZE + 1)
}

fn shift_south_west(bits: u64) -> u64 {
    (bits & NOT_A_FILE) << (BOARD_SIZE - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn idx(row: usize, col: usize) -> usize {
        row * BOARD_SIZE + col
    }

    #[test]
    fn t01_initial_black_legal_moves_are_four_expected_squares() {
        let board = Board::new();

        let expected = bit(idx(2, 3)) | bit(idx(3, 2)) | bit(idx(4, 5)) | bit(idx(5, 4)); // d3,c4,f5,e6

        assert_eq!(board.legal_moves(true), expected);
    }

    #[test]
    fn place_flips_opponent_stones_and_updates_counts() {
        let mut board = Board::new();

        let flips = board.place(idx(2, 3), true); // d3

        assert_eq!(flips, bit(idx(3, 3))); // d4
        assert_eq!(board.count(), (4, 1));
        assert_eq!(board.empty_count(), 59);

        let cells = board.to_array();
        assert_eq!(cells[idx(2, 3)], 1);
        assert_eq!(cells[idx(3, 3)], 1);
        assert_eq!(cells[idx(3, 4)], 1);
        assert_eq!(cells[idx(4, 3)], 1);
        assert_eq!(cells[idx(4, 4)], 2);
    }

    #[test]
    fn illegal_place_returns_zero_and_keeps_board_unchanged() {
        let mut board = Board::new();
        let before = board;

        let flips = board.place(idx(0, 0), true);

        assert_eq!(flips, 0);
        assert_eq!(board, before);
    }
}
