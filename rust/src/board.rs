const BOARD_SIZE: usize = 8;
const NUM_SQUARES: usize = BOARD_SIZE * BOARD_SIZE;
const DIRECTIONS: [(i32, i32); 8] = [
    (-1, -1),
    (-1, 0),
    (-1, 1),
    (0, -1),
    (0, 1),
    (1, -1),
    (1, 0),
    (1, 1),
];

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

    /// Returns legal move mask for the given side.
    pub fn legal_moves(&self, is_black: bool) -> u64 {
        let (me, opp) = if is_black {
            (self.black, self.white)
        } else {
            (self.white, self.black)
        };

        let occupied = me | opp;
        let mut legal = 0u64;

        for pos in 0..NUM_SQUARES {
            let move_bit = bit(pos);
            if (occupied & move_bit) != 0 {
                continue;
            }
            if Self::collect_flips(pos, me, opp) != 0 {
                legal |= move_bit;
            }
        }

        legal
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

    fn collect_flips(pos: usize, me: u64, opp: u64) -> u64 {
        if pos >= NUM_SQUARES {
            return 0;
        }

        let move_bit = bit(pos);
        if ((me | opp) & move_bit) != 0 {
            return 0;
        }

        let (row, col) = pos_to_row_col(pos);
        let mut flips = 0u64;

        for (dr, dc) in DIRECTIONS {
            let mut r = row + dr;
            let mut c = col + dc;
            let mut line = 0u64;
            let mut has_opponent = false;

            while in_bounds(r, c) {
                let square = bit((r as usize) * BOARD_SIZE + c as usize);
                if (opp & square) != 0 {
                    has_opponent = true;
                    line |= square;
                } else if (me & square) != 0 {
                    if has_opponent {
                        flips |= line;
                    }
                    break;
                } else {
                    break;
                }

                r += dr;
                c += dc;
            }
        }

        flips
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

fn pos_to_row_col(pos: usize) -> (i32, i32) {
    ((pos / BOARD_SIZE) as i32, (pos % BOARD_SIZE) as i32)
}

fn in_bounds(row: i32, col: i32) -> bool {
    (0..BOARD_SIZE as i32).contains(&row) && (0..BOARD_SIZE as i32).contains(&col)
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
