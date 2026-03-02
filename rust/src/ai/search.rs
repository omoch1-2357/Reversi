use std::time::{Duration, Instant};

use crate::ai::ntuple::NTupleEvaluator;
use crate::board::Board;

const DEFAULT_TIMEOUT_SECS: u64 = 5;
const MIN_SCORE: f32 = f32::NEG_INFINITY;
const MAX_SCORE: f32 = f32::INFINITY;
#[cfg(test)]
const BOARD_CELLS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq)]
enum SearchResult {
    Complete(usize, f32),
    TimedOut,
}

impl SearchResult {
    fn negate(self) -> Self {
        match self {
            Self::Complete(mv, score) => Self::Complete(mv, -score),
            Self::TimedOut => Self::TimedOut,
        }
    }
}

pub struct Searcher<'a> {
    evaluator: &'a NTupleEvaluator,
    start_time: Instant,
    timeout: Duration,
    max_depth: u8,
    timed_out: bool,
}

impl<'a> Searcher<'a> {
    pub fn new(evaluator: &'a NTupleEvaluator, max_depth: u8) -> Self {
        Self::with_timeout(
            evaluator,
            max_depth,
            Duration::from_secs(DEFAULT_TIMEOUT_SECS),
        )
    }

    pub fn with_timeout(evaluator: &'a NTupleEvaluator, max_depth: u8, timeout: Duration) -> Self {
        Self {
            evaluator,
            start_time: Instant::now(),
            timeout,
            max_depth,
            timed_out: false,
        }
    }

    /// Searches the best move.
    /// Caller contract: `board` must have at least one legal move for `is_black`.
    pub fn search(&mut self, board: &Board, is_black: bool) -> usize {
        self.start_time = Instant::now();
        self.timed_out = false;

        let legal = board.legal_moves(is_black);
        let moves = bitboard_to_positions(legal);
        debug_assert!(
            !moves.is_empty(),
            "search() requires at least one legal move"
        );

        if moves.is_empty() {
            unreachable!("search() called without legal moves");
        }
        if moves.len() == 1 {
            return moves[0];
        }

        let mut best_move = moves[0];

        for depth in 1..=self.max_depth {
            match self.negaalpha(board, is_black, depth, depth, MIN_SCORE, MAX_SCORE) {
                SearchResult::Complete(mv, _score) => {
                    best_move = mv;
                }
                SearchResult::TimedOut => break,
            }
        }

        if self.should_exact_solve(board)
            && !self.timed_out
            && let SearchResult::Complete(mv, _score) = self.exact_solve(board, is_black)
        {
            best_move = mv;
        }

        best_move
    }

    pub fn timed_out(&self) -> bool {
        self.timed_out
    }

    fn negaalpha(
        &mut self,
        board: &Board,
        is_black: bool,
        depth: u8,
        root_depth: u8,
        alpha: f32,
        beta: f32,
    ) -> SearchResult {
        // Keep depth-1 search guaranteed by suppressing timeout checks at root depth 1.
        if root_depth > 1 && self.start_time.elapsed() >= self.timeout {
            self.timed_out = true;
            return SearchResult::TimedOut;
        }

        if depth == 0 {
            return SearchResult::Complete(0, self.evaluator.evaluate(board, is_black));
        }

        let legal = board.legal_moves(is_black);
        if legal == 0 {
            let opp_legal = board.legal_moves(!is_black);
            if opp_legal == 0 {
                return SearchResult::Complete(0, exact_score(board, is_black));
            }
            return self
                .negaalpha(board, !is_black, depth, root_depth, -beta, -alpha)
                .negate();
        }

        let moves = bitboard_to_sorted_moves(legal, board, is_black, self.evaluator);
        let mut best_move = moves[0];
        let mut best_score = MIN_SCORE;
        let mut alpha = alpha;

        for mv in moves {
            let mut next = *board;
            let _ = next.place(mv, is_black);
            let result = self.negaalpha(&next, !is_black, depth - 1, root_depth, -beta, -alpha);

            match result {
                SearchResult::TimedOut => return SearchResult::TimedOut,
                SearchResult::Complete(_, score) => {
                    let score = -score;
                    if is_better_move(score, mv, best_score, best_move) {
                        best_score = score;
                        best_move = mv;
                    }
                    if score > alpha {
                        alpha = score;
                    }
                    if alpha >= beta {
                        break;
                    }
                }
            }
        }

        SearchResult::Complete(best_move, best_score)
    }

    fn should_exact_solve(&self, board: &Board) -> bool {
        let empty = board.empty_count();
        match self.max_depth {
            // REQUIREMENTS.md 2.3: Level 1-2 do not use exact solving.
            1 | 2 => false,
            3 => empty <= 10,
            4 => empty <= 12,
            5 => empty <= 14,
            6 => empty <= 16,
            _ => false,
        }
    }

    fn exact_solve(&mut self, board: &Board, is_black: bool) -> SearchResult {
        self.negaalpha_exact(board, is_black, board.empty_count(), MIN_SCORE, MAX_SCORE)
    }

    fn negaalpha_exact(
        &mut self,
        board: &Board,
        is_black: bool,
        empties: u8,
        alpha: f32,
        beta: f32,
    ) -> SearchResult {
        if self.start_time.elapsed() >= self.timeout {
            self.timed_out = true;
            return SearchResult::TimedOut;
        }

        if empties == 0 {
            return SearchResult::Complete(0, exact_score(board, is_black));
        }

        let legal = board.legal_moves(is_black);
        if legal == 0 {
            let opp_legal = board.legal_moves(!is_black);
            if opp_legal == 0 {
                return SearchResult::Complete(0, exact_score(board, is_black));
            }
            return self
                .negaalpha_exact(board, !is_black, empties, -beta, -alpha)
                .negate();
        }

        let moves = bitboard_to_sorted_moves(legal, board, is_black, self.evaluator);
        let mut best_move = moves[0];
        let mut best_score = MIN_SCORE;
        let mut alpha = alpha;

        for mv in moves {
            let mut next = *board;
            let _ = next.place(mv, is_black);
            let result = self.negaalpha_exact(&next, !is_black, empties - 1, -beta, -alpha);

            match result {
                SearchResult::TimedOut => return SearchResult::TimedOut,
                SearchResult::Complete(_, score) => {
                    let score = -score;
                    if is_better_move(score, mv, best_score, best_move) {
                        best_score = score;
                        best_move = mv;
                    }
                    if score > alpha {
                        alpha = score;
                    }
                    if alpha >= beta {
                        break;
                    }
                }
            }
        }

        SearchResult::Complete(best_move, best_score)
    }
}

fn is_better_move(score: f32, mv: usize, best_score: f32, best_move: usize) -> bool {
    score > best_score || (score == best_score && mv < best_move)
}

fn exact_score(board: &Board, is_black: bool) -> f32 {
    let (black, white) = board.count();
    if is_black {
        black as f32 - white as f32
    } else {
        white as f32 - black as f32
    }
}

fn bitboard_to_positions(mut mask: u64) -> Vec<usize> {
    let mut out = Vec::new();
    while mask != 0 {
        let mv = mask.trailing_zeros() as usize;
        out.push(mv);
        mask &= mask - 1;
    }
    out
}

fn bitboard_to_sorted_moves(
    legal: u64,
    board: &Board,
    is_black: bool,
    evaluator: &NTupleEvaluator,
) -> Vec<usize> {
    let mut scored_moves: Vec<(usize, f32)> = bitboard_to_positions(legal)
        .into_iter()
        .map(|mv| {
            let mut next = *board;
            let _ = next.place(mv, is_black);
            // Move ordering heuristic from the current player's perspective.
            let score = -evaluator.evaluate(&next, !is_black);
            (mv, score)
        })
        .collect();

    scored_moves.sort_by(|(left_mv, left_score), (right_mv, right_score)| {
        right_score
            .total_cmp(left_score)
            .then_with(|| left_mv.cmp(right_mv))
    });

    scored_moves.into_iter().map(|(mv, _)| mv).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAGIC: &[u8; 4] = b"NTRV";
    const VERSION: u32 = 1;
    const HEADER_SIZE: usize = 20;
    const FULL_BOARD: u64 = u64::MAX;

    fn bit(pos: usize) -> u64 {
        1u64 << pos
    }

    fn build_weights_blob(tuples: &[Vec<u8>], weights: &[Vec<f32>]) -> Vec<u8> {
        assert_eq!(tuples.len(), weights.len());

        let mut payload = Vec::new();
        for tuple in tuples {
            payload.push(tuple.len() as u8);
            payload.extend_from_slice(tuple);
        }
        for ws in weights {
            for w in ws {
                payload.extend_from_slice(&w.to_le_bytes());
            }
        }

        let crc = crc32fast::hash(&payload);
        let mut out = Vec::new();
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&VERSION.to_le_bytes());
        out.extend_from_slice(&(tuples.len() as u32).to_le_bytes());
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&payload);
        debug_assert_eq!(out.len(), HEADER_SIZE + payload.len());
        out
    }

    fn build_constant_evaluator() -> NTupleEvaluator {
        let tuples = vec![vec![0]];
        let weights = vec![vec![0.0, 0.0, 0.0]];
        let bytes = build_weights_blob(&tuples, &weights);
        NTupleEvaluator::from_bytes(&bytes).expect("constant evaluator must deserialize")
    }

    fn board_with_empty_count(empty: u8) -> Board {
        let occupied = BOARD_CELLS - empty as usize;
        let black = if occupied == BOARD_CELLS {
            u64::MAX
        } else {
            (1u64 << occupied) - 1
        };
        Board::from_bitboards(black, 0)
    }

    #[test]
    fn search_returns_single_legal_move_immediately() {
        let evaluator = build_constant_evaluator();
        let mut searcher = Searcher::new(&evaluator, 6);

        let black = bit(1);
        let white = FULL_BOARD ^ bit(0) ^ black;
        let board = Board::from_bitboards(black, white);

        assert_eq!(searcher.search(&board, false), 0);
        assert!(!searcher.timed_out());
    }

    #[test]
    fn search_tie_breaks_to_smallest_index_when_scores_equal() {
        let evaluator = build_constant_evaluator();
        let mut searcher = Searcher::new(&evaluator, 1);
        let board = Board::new();

        // Initial legal moves are [19, 26, 37, 44].
        assert_eq!(searcher.search(&board, true), 19);
    }

    #[test]
    fn search_depth_one_completes_before_timeout_cutoff() {
        let evaluator = build_constant_evaluator();
        let mut searcher = Searcher::with_timeout(&evaluator, 6, Duration::from_nanos(1));
        let board = Board::new();

        let mv = searcher.search(&board, true);
        let legal = board.legal_moves(true);

        assert_ne!(legal & (1u64 << mv), 0);
        assert!(searcher.timed_out());
    }

    #[test]
    fn should_exact_solve_threshold_matches_level_table() {
        let evaluator = build_constant_evaluator();
        let board_10 = board_with_empty_count(10);
        let board_11 = board_with_empty_count(11);
        let board_12 = board_with_empty_count(12);
        let board_13 = board_with_empty_count(13);
        let board_14 = board_with_empty_count(14);
        let board_15 = board_with_empty_count(15);
        let board_16 = board_with_empty_count(16);
        let board_17 = board_with_empty_count(17);

        assert!(Searcher::new(&evaluator, 3).should_exact_solve(&board_10));
        assert!(!Searcher::new(&evaluator, 3).should_exact_solve(&board_11));
        assert!(Searcher::new(&evaluator, 4).should_exact_solve(&board_12));
        assert!(!Searcher::new(&evaluator, 4).should_exact_solve(&board_13));
        assert!(Searcher::new(&evaluator, 5).should_exact_solve(&board_14));
        assert!(!Searcher::new(&evaluator, 5).should_exact_solve(&board_15));
        assert!(Searcher::new(&evaluator, 6).should_exact_solve(&board_16));
        assert!(!Searcher::new(&evaluator, 6).should_exact_solve(&board_17));
    }

    #[test]
    fn exact_solve_stops_when_deadline_is_already_exceeded() {
        let evaluator = build_constant_evaluator();
        let mut searcher = Searcher::with_timeout(&evaluator, 6, Duration::ZERO);
        searcher.start_time = Instant::now() - Duration::from_millis(1);

        let result = searcher.exact_solve(&Board::new(), true);

        assert_eq!(result, SearchResult::TimedOut);
        assert!(searcher.timed_out());
    }
}
