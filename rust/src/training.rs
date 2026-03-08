use std::sync::LazyLock;
use std::time::Instant;

use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

use crate::board::Board;

pub type ProgressCallback<'a> = &'a mut dyn FnMut(usize, usize, f64) -> Result<(), String>;

pub const TUPLE_PATTERNS: &[&[u8]] = &[
    &[0, 1, 8, 9, 10, 17, 18, 19, 26, 27],
    &[0, 1, 8, 9, 18, 27, 36, 45, 54, 63],
    &[0, 1, 2, 3, 8, 9, 10, 16, 17, 24],
    &[0, 1, 2, 3, 4, 8, 9, 16, 24, 32],
    &[0, 1, 2, 3, 4, 5, 6, 7, 9, 14],
    &[0, 2, 3, 4, 5, 7, 10, 11, 12, 13],
    &[1, 2, 3, 4, 5, 6, 10, 11, 12, 13],
    &[0, 1, 2, 8, 9, 10, 16, 17, 18],
    &[0, 1, 10, 19, 28, 37, 46, 55, 63],
    &[8, 9, 10, 11, 12, 13, 14, 15],
    &[16, 17, 18, 19, 20, 21, 22, 23],
    &[24, 25, 26, 27, 28, 29, 30, 31],
    &[1, 2, 11, 20, 29, 38, 47, 55],
    &[3, 9, 12, 21, 30, 39, 54],
];

const MAGIC: &[u8; 4] = b"NTRV";
const VERSION: u32 = 1;
const ROTATION_COUNT: usize = 4;
const TUPLE_COUNT: usize = 14;
const MAX_TUPLE_LEN: usize = 10;

#[derive(Debug, Clone, Copy)]
struct CompiledTuple {
    len: usize,
    rotated_positions: [[u8; MAX_TUPLE_LEN]; ROTATION_COUNT],
}

static COMPILED_TUPLES: LazyLock<[CompiledTuple; TUPLE_COUNT]> = LazyLock::new(|| {
    std::array::from_fn(|tuple_idx| {
        let pattern = TUPLE_PATTERNS[tuple_idx];
        let mut rotated_positions = [[0u8; MAX_TUPLE_LEN]; ROTATION_COUNT];

        for rotation in 0..ROTATION_COUNT {
            for (cell_idx, &pos) in pattern.iter().enumerate() {
                rotated_positions[rotation][cell_idx] = rotate_pos(pos, rotation as u8) as u8;
            }
        }

        CompiledTuple {
            len: pattern.len(),
            rotated_positions,
        }
    })
});

pub trait TrainingNetwork {
    fn evaluate(&self, board: &Board, is_black: bool) -> f32;
    fn update(&mut self, board: &Board, is_black: bool, delta: f32);

    fn td_lambda_step(
        &mut self,
        board: &Board,
        is_black: bool,
        next_value: f32,
        cumulative_td: f32,
        next_player: Option<bool>,
        alpha: f32,
        lambda_: f32,
    ) -> (f32, f32) {
        let current_value = self.evaluate(board, is_black);
        let td_error = next_value - current_value;
        let next_cumulative_td = if let Some(previous_player) = next_player {
            let signed_lambda = if is_black == previous_player {
                lambda_
            } else {
                -lambda_
            };
            td_error + signed_lambda * cumulative_td
        } else {
            td_error
        };

        self.update(board, is_black, alpha * next_cumulative_td);
        (current_value, next_cumulative_td)
    }
}

#[derive(Debug, Clone)]
pub struct TrainableNTuple {
    weights: Vec<Vec<f32>>,
}

impl TrainableNTuple {
    pub fn new() -> Self {
        let weights = TUPLE_PATTERNS
            .iter()
            .map(|pattern| vec![0.0; pow3(pattern.len()).expect("tuple size must fit usize")])
            .collect();
        Self { weights }
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        if self.weights.len() != TUPLE_PATTERNS.len() {
            return Err("weights length must match tuple patterns length".to_string());
        }

        let tuple_defs_len: usize = TUPLE_PATTERNS.iter().map(|pattern| 1 + pattern.len()).sum();
        let weights_bytes: usize = self
            .weights
            .iter()
            .map(|weights| weights.len() * std::mem::size_of::<f32>())
            .sum();
        let mut data = Vec::with_capacity(tuple_defs_len + weights_bytes);

        for pattern in TUPLE_PATTERNS {
            data.push(pattern.len() as u8);
            data.extend_from_slice(pattern);
        }

        for (idx, weights) in self.weights.iter().enumerate() {
            let expected_len = pow3(TUPLE_PATTERNS[idx].len())?;
            if weights.len() != expected_len {
                return Err(format!(
                    "weights[{idx}] length must be {expected_len}, got {}",
                    weights.len()
                ));
            }
            for value in weights {
                data.extend_from_slice(&value.to_le_bytes());
            }
        }

        let crc32 = crc32fast::hash(&data);
        let mut output = Vec::with_capacity(20 + data.len());
        output.extend_from_slice(MAGIC);
        output.extend_from_slice(&VERSION.to_le_bytes());
        output.extend_from_slice(&(TUPLE_PATTERNS.len() as u32).to_le_bytes());
        output.extend_from_slice(&crc32.to_le_bytes());
        output.extend_from_slice(&0u32.to_le_bytes());
        output.extend_from_slice(&data);
        Ok(output)
    }

    pub fn raw_weights(&self) -> &[Vec<f32>] {
        &self.weights
    }

    fn compute_feature_indices(
        board: &Board,
        is_black: bool,
    ) -> [[usize; TUPLE_COUNT]; ROTATION_COUNT] {
        let (black, white) = board.bitboards();
        let (me, opp) = if is_black {
            (black, white)
        } else {
            (white, black)
        };
        let mut indices = [[0usize; TUPLE_COUNT]; ROTATION_COUNT];

        for rotation in 0..ROTATION_COUNT {
            for tuple_idx in 0..TUPLE_COUNT {
                let compiled = &COMPILED_TUPLES[tuple_idx];
                indices[rotation][tuple_idx] =
                    tuple_index(&compiled.rotated_positions[rotation], compiled.len, me, opp);
            }
        }

        indices
    }

    fn sum_feature_indices(&self, indices: &[[usize; TUPLE_COUNT]; ROTATION_COUNT]) -> f32 {
        let mut score = 0.0f32;

        for tuple_indices in indices {
            for (tuple_idx, &index) in tuple_indices.iter().enumerate() {
                score += self.weights[tuple_idx][index];
            }
        }

        score
    }

    fn apply_delta(&mut self, indices: &[[usize; TUPLE_COUNT]; ROTATION_COUNT], delta: f32) {
        for tuple_indices in indices {
            for (tuple_idx, &index) in tuple_indices.iter().enumerate() {
                self.weights[tuple_idx][index] += delta;
            }
        }
    }
}

impl Default for TrainableNTuple {
    fn default() -> Self {
        Self::new()
    }
}

impl TrainingNetwork for TrainableNTuple {
    fn evaluate(&self, board: &Board, is_black: bool) -> f32 {
        let indices = Self::compute_feature_indices(board, is_black);
        self.sum_feature_indices(&indices)
    }

    fn update(&mut self, board: &Board, is_black: bool, delta: f32) {
        let indices = Self::compute_feature_indices(board, is_black);
        self.apply_delta(&indices, delta);
    }

    fn td_lambda_step(
        &mut self,
        board: &Board,
        is_black: bool,
        next_value: f32,
        cumulative_td: f32,
        next_player: Option<bool>,
        alpha: f32,
        lambda_: f32,
    ) -> (f32, f32) {
        let indices = Self::compute_feature_indices(board, is_black);
        let current_value = self.sum_feature_indices(&indices);
        let td_error = next_value - current_value;
        let next_cumulative_td = if let Some(previous_player) = next_player {
            let signed_lambda = if is_black == previous_player {
                lambda_
            } else {
                -lambda_
            };
            td_error + signed_lambda * cumulative_td
        } else {
            td_error
        };

        self.apply_delta(&indices, alpha * next_cumulative_td);
        (current_value, next_cumulative_td)
    }
}

pub struct TDLambdaTrainer<N> {
    network: N,
    alpha: f32,
    lambda_: f32,
    epsilon: f64,
    rng: ChaCha8Rng,
}

impl<N: TrainingNetwork> TDLambdaTrainer<N> {
    pub fn new(
        network: N,
        alpha: f32,
        lambda_: f32,
        epsilon: f64,
        seed: u64,
    ) -> Result<Self, String> {
        if alpha < 0.0 {
            return Err(format!("alpha must be >= 0.0, got {alpha}"));
        }
        if !(0.0..=1.0).contains(&lambda_) {
            return Err(format!("lambda_ must be in [0.0, 1.0], got {lambda_}"));
        }
        if !(0.0..=1.0).contains(&epsilon) {
            return Err(format!("epsilon must be in [0.0, 1.0], got {epsilon}"));
        }

        Ok(Self {
            network,
            alpha,
            lambda_,
            epsilon,
            rng: ChaCha8Rng::seed_from_u64(seed),
        })
    }

    pub fn train(
        &mut self,
        num_games: usize,
        progress_interval: usize,
        mut progress_callback: Option<ProgressCallback<'_>>,
    ) -> Result<(), String> {
        let start_time = Instant::now();
        for game_idx in 1..=num_games {
            self.play_one_game()?;
            if let Some(callback) = progress_callback.as_mut() {
                if progress_interval > 0 && game_idx % progress_interval == 0 {
                    callback(game_idx, num_games, start_time.elapsed().as_secs_f64())?;
                }
            }
        }

        if let Some(callback) = progress_callback.as_mut() {
            if progress_interval > 0 && num_games > 0 && num_games % progress_interval != 0 {
                callback(num_games, num_games, start_time.elapsed().as_secs_f64())?;
            }
        }

        Ok(())
    }

    pub fn into_network(self) -> N {
        self.network
    }

    fn play_one_game(&mut self) -> Result<(), String> {
        let mut board = Board::new();
        let mut is_black = true;
        let mut consecutive_passes = 0usize;
        let mut history: Vec<(Board, bool)> = Vec::with_capacity(60);

        while consecutive_passes < 2 {
            let legal = board.legal_moves(is_black);
            if legal == 0 {
                consecutive_passes += 1;
                is_black = !is_black;
                continue;
            }

            consecutive_passes = 0;
            let mv = self.select_move(&board, is_black, legal)?;
            history.push((board, is_black));
            let flipped = board.place(mv, is_black);
            if flipped == 0 {
                return Err(format!("selected illegal move: {mv}"));
            }
            is_black = !is_black;
        }

        self.update_weights(&history, &board);
        Ok(())
    }

    fn select_move(&mut self, board: &Board, is_black: bool, legal: u64) -> Result<usize, String> {
        if legal == 0 {
            return Err("legal move mask contains no moves".to_string());
        }

        let move_count = legal.count_ones();
        if self.rng.gen_bool(self.epsilon) {
            let choice = self.rng.gen_range(0..move_count);
            return Ok(nth_move_from_mask(legal, choice));
        }

        let mut remaining = legal;
        let mut best_move = remaining.trailing_zeros() as usize;
        let mut best_score = f32::NEG_INFINITY;

        while remaining != 0 {
            let mv = remaining.trailing_zeros() as usize;
            remaining &= remaining - 1;

            let mut next_board = *board;
            let flipped = next_board.place(mv, is_black);
            if flipped == 0 {
                return Err(format!("selected illegal move: {mv}"));
            }
            let score = self.network.evaluate(&next_board, is_black);
            if score > best_score {
                best_score = score;
                best_move = mv;
            }
        }

        Ok(best_move)
    }

    fn update_weights(&mut self, history: &[(Board, bool)], final_board: &Board) {
        if history.is_empty() {
            return;
        }

        let (black_count, white_count) = final_board.count();
        let reward = if black_count > white_count {
            1.0
        } else if black_count < white_count {
            -1.0
        } else {
            0.0
        };

        let mut next_value = if history.last().expect("history must be non-empty").1 {
            reward
        } else {
            -reward
        };
        let mut cumulative_td = 0.0f32;
        let mut next_player: Option<bool> = None;

        for &(board, is_black) in history.iter().rev() {
            let (current_value, next_cumulative_td) = self.network.td_lambda_step(
                &board,
                is_black,
                next_value,
                cumulative_td,
                next_player,
                self.alpha,
                self.lambda_,
            );
            cumulative_td = next_cumulative_td;
            next_value = -current_value;
            next_player = Some(is_black);
        }
    }
}

pub fn train_to_bytes(
    games: usize,
    alpha: f32,
    lambda_: f32,
    epsilon: f64,
    seed: u64,
    progress_interval: usize,
    progress_callback: Option<ProgressCallback<'_>>,
) -> Result<Vec<u8>, String> {
    let network = TrainableNTuple::new();
    let mut trainer = TDLambdaTrainer::new(network, alpha, lambda_, epsilon, seed)?;
    trainer.train(games, progress_interval, progress_callback)?;
    trainer.into_network().to_bytes()
}

fn nth_move_from_mask(mask: u64, target: u32) -> usize {
    let mut remaining = mask;
    let mut skip = target;

    loop {
        let idx = remaining.trailing_zeros() as usize;
        if skip == 0 {
            return idx;
        }
        remaining &= remaining - 1;
        skip -= 1;
    }
}

fn rotate_pos(pos: u8, rotation: u8) -> usize {
    const BOARD_SIZE: usize = 8;
    let row = (pos as usize) / BOARD_SIZE;
    let col = (pos as usize) % BOARD_SIZE;

    let (nr, nc) = match rotation % 4 {
        0 => (row, col),
        1 => (col, BOARD_SIZE - 1 - row),
        2 => (BOARD_SIZE - 1 - row, BOARD_SIZE - 1 - col),
        _ => (BOARD_SIZE - 1 - col, row),
    };

    nr * BOARD_SIZE + nc
}

fn tuple_index(positions: &[u8; MAX_TUPLE_LEN], len: usize, me: u64, opp: u64) -> usize {
    let mut index = 0usize;

    for &pos in positions.iter().take(len) {
        index = index * 3 + cell_state(me, opp, pos as usize);
    }

    index
}

fn cell_state(me: u64, opp: u64, pos: usize) -> usize {
    let square = 1u64 << pos;
    if (me & square) != 0 {
        1
    } else if (opp & square) != 0 {
        2
    } else {
        0
    }
}

fn pow3(exp: usize) -> Result<usize, String> {
    let mut out = 1usize;
    for _ in 0..exp {
        out = out
            .checked_mul(3)
            .ok_or_else(|| "3^tuple_size overflow".to_string())?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::ntuple::NTupleEvaluator;

    struct RecordingNetwork {
        value: f32,
        updates: Vec<(bool, f32)>,
    }

    impl TrainingNetwork for RecordingNetwork {
        fn evaluate(&self, _board: &Board, _is_black: bool) -> f32 {
            self.value
        }

        fn update(&mut self, _board: &Board, is_black: bool, delta: f32) {
            self.updates.push((is_black, delta));
        }
    }

    #[test]
    fn update_direction_is_toward_td_target() {
        let network = RecordingNetwork {
            value: 0.0,
            updates: Vec::new(),
        };
        let mut trainer = TDLambdaTrainer::new(network, 0.5, 0.0, 0.0, 7).unwrap();
        let history = vec![(Board::new(), true)];
        let final_board = Board::from_bitboards(u64::MAX, 0);

        trainer.update_weights(&history, &final_board);

        assert_eq!(trainer.network.updates.len(), 1);
        assert_eq!(trainer.network.updates[0], (true, 0.5));
    }

    #[test]
    fn terminal_reward_is_reflected_per_player_perspective() {
        for (is_black, expected) in [(true, 1.0f32), (false, -1.0f32)] {
            let network = RecordingNetwork {
                value: 0.0,
                updates: Vec::new(),
            };
            let mut trainer = TDLambdaTrainer::new(network, 1.0, 0.0, 0.0, 11).unwrap();
            let history = vec![(Board::new(), is_black)];
            let final_board = Board::from_bitboards(u64::MAX, 0);

            trainer.update_weights(&history, &final_board);

            assert_eq!(trainer.network.updates[0], (is_black, expected));
        }
    }

    #[test]
    fn update_weights_uses_lambda_return_across_multiple_steps() {
        let network = RecordingNetwork {
            value: 0.0,
            updates: Vec::new(),
        };
        let mut trainer = TDLambdaTrainer::new(network, 1.0, 0.5, 0.0, 13).unwrap();
        let history = vec![(Board::new(), true), (Board::new(), false)];
        let final_board = Board::from_bitboards(u64::MAX, 0);

        trainer.update_weights(&history, &final_board);

        assert_eq!(trainer.network.updates.len(), 2);
        assert_eq!(trainer.network.updates[0], (false, -1.0));
        assert_eq!(trainer.network.updates[1], (true, 0.5));
    }

    #[test]
    fn play_one_game_is_reproducible_with_fixed_seed() {
        let mut trainer_a =
            TDLambdaTrainer::new(TrainableNTuple::new(), 0.01, 0.7, 0.3, 2026).unwrap();
        let mut trainer_b =
            TDLambdaTrainer::new(TrainableNTuple::new(), 0.01, 0.7, 0.3, 2026).unwrap();

        trainer_a.play_one_game().unwrap();
        trainer_b.play_one_game().unwrap();

        assert_eq!(
            trainer_a.network.raw_weights(),
            trainer_b.network.raw_weights()
        );
        assert!(
            trainer_a
                .network
                .raw_weights()
                .iter()
                .any(|weights| weights.iter().any(|value| *value != 0.0))
        );
    }

    #[test]
    fn train_reports_progress_at_interval_and_completion() {
        let mut trainer = TDLambdaTrainer::new(TrainableNTuple::new(), 0.0, 0.0, 0.0, 5).unwrap();
        let mut updates = Vec::new();
        let mut callback = |done: usize, total: usize, elapsed: f64| {
            updates.push((done, total, elapsed));
            Ok(())
        };

        trainer.train(5, 2, Some(&mut callback)).unwrap();

        assert_eq!(updates.len(), 3);
        assert_eq!(updates[0].0, 2);
        assert_eq!(updates[1].0, 4);
        assert_eq!(updates[2].0, 5);
        assert!(updates.iter().all(|(_, total, _)| *total == 5));
        assert!(updates.iter().all(|(_, _, elapsed)| *elapsed >= 0.0));
    }

    #[test]
    fn exported_bytes_are_readable_by_inference_evaluator() {
        let bytes = train_to_bytes(0, 0.01, 0.7, 0.1, 42, 0, None).unwrap();
        let evaluator = NTupleEvaluator::from_bytes(&bytes).unwrap();
        let score = evaluator.evaluate(&Board::new(), true);
        assert_eq!(score, 0.0);
    }
}
