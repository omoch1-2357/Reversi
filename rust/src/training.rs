use std::sync::LazyLock;
use std::sync::mpsc;
use std::time::Instant;

use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

use crate::ai::ntuple::{compress_model_bytes, decompress_model_bytes};
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
const VERSION: u32 = 3;
const HEADER_SIZE: usize = 20;
const SYMMETRY_COUNT: usize = 8;
const TUPLE_COUNT: usize = 14;
const MAX_TUPLE_LEN: usize = 10;
const TRAINING_SEARCH_DEPTH: u8 = 2;
pub const PHASE_COUNT: usize = 30;

#[derive(Debug, Clone, Copy)]
struct CompiledTuple {
    len: usize,
    transformed_positions: [[u8; MAX_TUPLE_LEN]; SYMMETRY_COUNT],
}

static COMPILED_TUPLES: LazyLock<[CompiledTuple; TUPLE_COUNT]> = LazyLock::new(|| {
    std::array::from_fn(|tuple_idx| {
        let pattern = TUPLE_PATTERNS[tuple_idx];
        let mut transformed_positions = [[0u8; MAX_TUPLE_LEN]; SYMMETRY_COUNT];

        for symmetry in 0..SYMMETRY_COUNT {
            for (cell_idx, &pos) in pattern.iter().enumerate() {
                transformed_positions[symmetry][cell_idx] =
                    transform_pos(pos, symmetry as u8) as u8;
            }
        }

        CompiledTuple {
            len: pattern.len(),
            transformed_positions,
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
    phase_count: usize,
    weights: Vec<Vec<Vec<f32>>>,
}

impl TrainableNTuple {
    pub fn new() -> Self {
        let template: Vec<Vec<f32>> = TUPLE_PATTERNS
            .iter()
            .map(|pattern| vec![0.0; pow3(pattern.len()).expect("tuple size must fit usize")])
            .collect();
        Self {
            phase_count: PHASE_COUNT,
            weights: vec![template; PHASE_COUNT],
        }
    }

    pub fn from_bytes(data: &[u8]) -> Result<Self, String> {
        let bytes = decompress_model_bytes(data)?;
        Self::from_uncompressed_bytes(bytes.as_ref())
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        if self.phase_count == 0 {
            return Err("phase_count must be greater than 0".to_string());
        }
        if self.weights.len() != self.phase_count {
            return Err(format!(
                "weights phase length must match phase_count: expected {}, got {}",
                self.phase_count,
                self.weights.len()
            ));
        }

        let tuple_defs_len: usize = TUPLE_PATTERNS.iter().map(|pattern| 1 + pattern.len()).sum();
        let weights_bytes: usize = self
            .weights
            .iter()
            .map(|phase_weights| {
                phase_weights
                    .iter()
                    .map(|weights| weights.len() * std::mem::size_of::<f32>())
                    .sum::<usize>()
            })
            .sum();
        let mut data = Vec::with_capacity(tuple_defs_len + weights_bytes);

        for pattern in TUPLE_PATTERNS {
            data.push(pattern.len() as u8);
            data.extend_from_slice(pattern);
        }

        for (phase_idx, phase_weights) in self.weights.iter().enumerate() {
            if phase_weights.len() != TUPLE_PATTERNS.len() {
                return Err(format!(
                    "weights[{phase_idx}] tuple length must match tuple patterns length"
                ));
            }

            for (tuple_idx, weights) in phase_weights.iter().enumerate() {
                let expected_len = pow3(TUPLE_PATTERNS[tuple_idx].len())?;
                if weights.len() != expected_len {
                    return Err(format!(
                        "weights[{phase_idx}][{tuple_idx}] length must be {expected_len}, got {}",
                        weights.len()
                    ));
                }
                for value in weights {
                    data.extend_from_slice(&value.to_le_bytes());
                }
            }
        }

        let crc32 = crc32fast::hash(&data);
        let mut output = Vec::with_capacity(20 + data.len());
        output.extend_from_slice(MAGIC);
        output.extend_from_slice(&VERSION.to_le_bytes());
        output.extend_from_slice(&(TUPLE_PATTERNS.len() as u32).to_le_bytes());
        output.extend_from_slice(&crc32.to_le_bytes());
        output.extend_from_slice(&(self.phase_count as u32).to_le_bytes());
        output.extend_from_slice(&data);
        compress_model_bytes(&output)
    }

    pub fn raw_weights(&self) -> &[Vec<Vec<f32>>] {
        &self.weights
    }

    fn merge_weighted(
        workers: &[(TrainableNTuple, usize)],
        total_games: usize,
    ) -> Result<Self, String> {
        let mut merged = Self::new();
        if total_games == 0 {
            return Ok(merged);
        }

        for (network, games) in workers {
            if *games == 0 {
                continue;
            }
            merged.accumulate_scaled_from(network, (*games as f32) / (total_games as f32))?;
        }

        Ok(merged)
    }

    fn phase_index(&self, board: &Board) -> usize {
        phase_index_for_board(board, self.phase_count)
    }

    fn from_uncompressed_bytes(data: &[u8]) -> Result<Self, String> {
        if data.len() < HEADER_SIZE {
            return Err(format!(
                "weights data too short: expected at least {HEADER_SIZE} bytes, got {}",
                data.len()
            ));
        }

        if &data[0..4] != MAGIC {
            return Err("invalid weights magic (expected NTRV)".to_string());
        }

        let version = read_u32_le(data, 4)?;
        if version != VERSION {
            return Err(format!(
                "unsupported training weights version: expected {VERSION}, got {version}"
            ));
        }

        let num_tuples = read_u32_le(data, 8)? as usize;
        if num_tuples != TUPLE_PATTERNS.len() {
            return Err(format!(
                "tuple count mismatch: expected {}, got {}",
                TUPLE_PATTERNS.len(),
                num_tuples
            ));
        }

        let expected_crc = read_u32_le(data, 12)?;
        let phase_count = read_u32_le(data, 16)? as usize;
        if phase_count != PHASE_COUNT {
            return Err(format!(
                "phase_count mismatch: expected {PHASE_COUNT}, got {phase_count}"
            ));
        }

        let payload = &data[HEADER_SIZE..];
        let actual_crc = crc32fast::hash(payload);
        if actual_crc != expected_crc {
            return Err(format!(
                "CRC32 mismatch: expected {expected_crc:#010x}, got {actual_crc:#010x}"
            ));
        }

        let mut offset = 0usize;
        for (tuple_idx, pattern) in TUPLE_PATTERNS.iter().enumerate() {
            if offset >= payload.len() {
                return Err(format!(
                    "unexpected EOF while reading tuple definition #{tuple_idx}"
                ));
            }

            let tuple_size = payload[offset] as usize;
            offset += 1;
            if tuple_size != pattern.len() {
                return Err(format!(
                    "tuple_size mismatch at index {tuple_idx}: expected {}, got {tuple_size}",
                    pattern.len()
                ));
            }

            let end = offset + tuple_size;
            if end > payload.len() {
                return Err(format!(
                    "unexpected EOF while reading tuple positions #{tuple_idx}"
                ));
            }

            if &payload[offset..end] != *pattern {
                return Err(format!(
                    "tuple positions mismatch at index {tuple_idx}: expected {:?}, got {:?}",
                    pattern,
                    &payload[offset..end]
                ));
            }
            offset = end;
        }

        let mut weights = Vec::with_capacity(PHASE_COUNT);
        for phase_idx in 0..PHASE_COUNT {
            let mut phase_weights = Vec::with_capacity(TUPLE_PATTERNS.len());
            for (tuple_idx, pattern) in TUPLE_PATTERNS.iter().enumerate() {
                let entries = pow3(pattern.len())?;
                let bytes_len = entries
                    .checked_mul(std::mem::size_of::<f32>())
                    .ok_or_else(|| "weights byte length overflow".to_string())?;
                if offset + bytes_len > payload.len() {
                    return Err(format!(
                        "unexpected EOF while reading weights for phase #{phase_idx}, tuple #{tuple_idx}"
                    ));
                }

                let mut tuple_weights = Vec::with_capacity(entries);
                for entry_idx in 0..entries {
                    let start = offset + entry_idx * std::mem::size_of::<f32>();
                    let mut chunk = [0u8; 4];
                    chunk.copy_from_slice(&payload[start..start + 4]);
                    tuple_weights.push(f32::from_le_bytes(chunk));
                }

                phase_weights.push(tuple_weights);
                offset += bytes_len;
            }
            weights.push(phase_weights);
        }

        if offset != payload.len() {
            return Err("weights payload has trailing bytes".to_string());
        }

        Ok(Self {
            phase_count,
            weights,
        })
    }

    fn compute_feature_indices(
        board: &Board,
        is_black: bool,
    ) -> [[usize; TUPLE_COUNT]; SYMMETRY_COUNT] {
        let (black, white) = board.bitboards();
        let (me, opp) = if is_black {
            (black, white)
        } else {
            (white, black)
        };
        let mut indices = [[0usize; TUPLE_COUNT]; SYMMETRY_COUNT];

        for symmetry in 0..SYMMETRY_COUNT {
            for tuple_idx in 0..TUPLE_COUNT {
                let compiled = &COMPILED_TUPLES[tuple_idx];
                indices[symmetry][tuple_idx] = tuple_index(
                    &compiled.transformed_positions[symmetry],
                    compiled.len,
                    me,
                    opp,
                );
            }
        }

        indices
    }

    fn sum_feature_indices(
        &self,
        phase_idx: usize,
        indices: &[[usize; TUPLE_COUNT]; SYMMETRY_COUNT],
    ) -> f32 {
        let mut score = 0.0f32;
        let phase_weights = &self.weights[phase_idx];

        for tuple_indices in indices {
            for (tuple_idx, &index) in tuple_indices.iter().enumerate() {
                score += phase_weights[tuple_idx][index];
            }
        }

        score
    }

    fn apply_delta(
        &mut self,
        phase_idx: usize,
        indices: &[[usize; TUPLE_COUNT]; SYMMETRY_COUNT],
        delta: f32,
    ) {
        let phase_weights = &mut self.weights[phase_idx];
        for tuple_indices in indices {
            for (tuple_idx, &index) in tuple_indices.iter().enumerate() {
                phase_weights[tuple_idx][index] += delta;
            }
        }
    }

    fn accumulate_scaled_from(&mut self, other: &Self, scale: f32) -> Result<(), String> {
        if self.phase_count != other.phase_count {
            return Err(format!(
                "phase_count mismatch while merging networks: {} vs {}",
                self.phase_count, other.phase_count
            ));
        }

        for (phase_idx, (target_phase, source_phase)) in self
            .weights
            .iter_mut()
            .zip(other.weights.iter())
            .enumerate()
        {
            if target_phase.len() != source_phase.len() {
                return Err(format!(
                    "tuple count mismatch while merging phase {phase_idx}: {} vs {}",
                    target_phase.len(),
                    source_phase.len()
                ));
            }

            for (tuple_idx, (target_weights, source_weights)) in
                target_phase.iter_mut().zip(source_phase.iter()).enumerate()
            {
                if target_weights.len() != source_weights.len() {
                    return Err(format!(
                        "weight length mismatch while merging phase {phase_idx}, tuple {tuple_idx}: {} vs {}",
                        target_weights.len(),
                        source_weights.len()
                    ));
                }

                for (target, source) in target_weights.iter_mut().zip(source_weights.iter()) {
                    *target += source * scale;
                }
            }
        }

        Ok(())
    }
}

impl Default for TrainableNTuple {
    fn default() -> Self {
        Self::new()
    }
}

impl TrainingNetwork for TrainableNTuple {
    fn evaluate(&self, board: &Board, is_black: bool) -> f32 {
        let phase_idx = self.phase_index(board);
        let indices = Self::compute_feature_indices(board, is_black);
        self.sum_feature_indices(phase_idx, &indices)
    }

    fn update(&mut self, board: &Board, is_black: bool, delta: f32) {
        let phase_idx = self.phase_index(board);
        let indices = Self::compute_feature_indices(board, is_black);
        self.apply_delta(phase_idx, &indices, delta);
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
        let phase_idx = self.phase_index(board);
        let indices = Self::compute_feature_indices(board, is_black);
        let current_value = self.sum_feature_indices(phase_idx, &indices);
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

        self.apply_delta(phase_idx, &indices, alpha * next_cumulative_td);
        (current_value, next_cumulative_td)
    }
}

pub struct TDLambdaTrainer<N> {
    network: N,
    alpha: f32,
    lambda_: f32,
    epsilon: f64,
    random_opening_plies: usize,
    rng: ChaCha8Rng,
}

#[derive(Debug, Clone, Copy)]
enum WorkerMessage {
    Progress(usize),
    Done,
}

impl<N: TrainingNetwork> TDLambdaTrainer<N> {
    pub fn new(
        network: N,
        alpha: f32,
        lambda_: f32,
        epsilon: f64,
        seed: u64,
        random_opening_plies: usize,
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
            random_opening_plies,
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

        self.apply_random_opening(
            &mut board,
            &mut is_black,
            &mut consecutive_passes,
            &mut history,
        )?;

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

    fn apply_random_opening(
        &mut self,
        board: &mut Board,
        is_black: &mut bool,
        consecutive_passes: &mut usize,
        history: &mut Vec<(Board, bool)>,
    ) -> Result<(), String> {
        let mut applied_plies = 0usize;
        while applied_plies < self.random_opening_plies && *consecutive_passes < 2 {
            let legal = board.legal_moves(*is_black);
            if legal == 0 {
                *consecutive_passes += 1;
                *is_black = !*is_black;
                continue;
            }

            *consecutive_passes = 0;
            let choice = self.rng.gen_range(0..legal.count_ones());
            let mv = nth_move_from_mask(legal, choice);
            history.push((*board, *is_black));
            let flipped = board.place(mv, *is_black);
            if flipped == 0 {
                return Err(format!("selected illegal random opening move: {mv}"));
            }
            *is_black = !*is_black;
            applied_plies += 1;
        }

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

        let moves = ordered_moves(board, is_black, legal, |next_board, next_player| {
            -self.network.evaluate(next_board, next_player)
        });
        let mut best_move = moves[0];
        let mut best_score = f32::NEG_INFINITY;

        for mv in moves {
            let mut next_board = *board;
            let flipped = next_board.place(mv, is_black);
            if flipped == 0 {
                return Err(format!("selected illegal move: {mv}"));
            }
            let score = -self.search_training_position(
                &next_board,
                !is_black,
                TRAINING_SEARCH_DEPTH - 1,
            )?;
            if is_better_move(score, mv, best_score, best_move) {
                best_score = score;
                best_move = mv;
            }
        }

        Ok(best_move)
    }

    fn search_training_position(
        &self,
        board: &Board,
        is_black: bool,
        depth: u8,
    ) -> Result<f32, String> {
        if depth == 0 {
            return Ok(self.network.evaluate(board, is_black));
        }

        let legal = board.legal_moves(is_black);
        if legal == 0 {
            let opp_legal = board.legal_moves(!is_black);
            if opp_legal == 0 {
                return Ok(terminal_training_score(board, is_black));
            }
            return Ok(-self.search_training_position(board, !is_black, depth)?);
        }

        let moves = ordered_moves(board, is_black, legal, |next_board, next_player| {
            -self.network.evaluate(next_board, next_player)
        });
        let mut best_move = moves[0];
        let mut best_score = f32::NEG_INFINITY;

        for mv in moves {
            let mut next_board = *board;
            let flipped = next_board.place(mv, is_black);
            if flipped == 0 {
                return Err(format!("selected illegal move: {mv}"));
            }

            let score = -self.search_training_position(&next_board, !is_black, depth - 1)?;
            if is_better_move(score, mv, best_score, best_move) {
                best_score = score;
                best_move = mv;
            }
        }

        Ok(best_score)
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
    threads: usize,
    initial_model: Option<&[u8]>,
    random_opening_plies: usize,
    progress_interval: usize,
    progress_callback: Option<ProgressCallback<'_>>,
) -> Result<Vec<u8>, String> {
    let network = train_network(
        games,
        alpha,
        lambda_,
        epsilon,
        seed,
        threads,
        initial_model,
        random_opening_plies,
        progress_interval,
        progress_callback,
    )?;
    network.to_bytes()
}

fn train_network(
    games: usize,
    alpha: f32,
    lambda_: f32,
    epsilon: f64,
    seed: u64,
    threads: usize,
    initial_model: Option<&[u8]>,
    random_opening_plies: usize,
    progress_interval: usize,
    progress_callback: Option<ProgressCallback<'_>>,
) -> Result<TrainableNTuple, String> {
    let base_network = if let Some(bytes) = initial_model {
        TrainableNTuple::from_bytes(bytes)?
    } else {
        TrainableNTuple::new()
    };
    let resolved_threads = resolve_thread_count(threads);
    let active_threads = resolved_threads.min(games.max(1));

    if active_threads <= 1 || games == 0 {
        return train_network_sequential(
            base_network,
            games,
            alpha,
            lambda_,
            epsilon,
            seed,
            random_opening_plies,
            progress_interval,
            progress_callback,
        );
    }

    train_network_parallel(
        base_network,
        games,
        alpha,
        lambda_,
        epsilon,
        seed,
        active_threads,
        random_opening_plies,
        progress_interval,
        progress_callback,
    )
}

fn train_network_sequential(
    network: TrainableNTuple,
    games: usize,
    alpha: f32,
    lambda_: f32,
    epsilon: f64,
    seed: u64,
    random_opening_plies: usize,
    progress_interval: usize,
    progress_callback: Option<ProgressCallback<'_>>,
) -> Result<TrainableNTuple, String> {
    let mut trainer =
        TDLambdaTrainer::new(network, alpha, lambda_, epsilon, seed, random_opening_plies)?;
    trainer.train(games, progress_interval, progress_callback)?;
    Ok(trainer.into_network())
}

fn train_network_parallel(
    base_network: TrainableNTuple,
    games: usize,
    alpha: f32,
    lambda_: f32,
    epsilon: f64,
    seed: u64,
    threads: usize,
    random_opening_plies: usize,
    progress_interval: usize,
    mut progress_callback: Option<ProgressCallback<'_>>,
) -> Result<TrainableNTuple, String> {
    let worker_game_counts = split_games(games, threads);
    let worker_progress_interval = if progress_interval == 0 {
        0
    } else {
        (progress_interval / threads).max(1)
    };
    let start_time = Instant::now();
    let (tx, rx) = mpsc::channel::<WorkerMessage>();

    std::thread::scope(|scope| -> Result<TrainableNTuple, String> {
        let mut handles = Vec::with_capacity(worker_game_counts.len());
        for (worker_idx, worker_games) in worker_game_counts.iter().copied().enumerate() {
            let worker_tx = tx.clone();
            let worker_network = base_network.clone();
            handles.push(
                scope.spawn(move || -> Result<(TrainableNTuple, usize), String> {
                    let result = train_worker_network(
                        worker_network,
                        worker_games,
                        alpha,
                        lambda_,
                        epsilon,
                        worker_seed(seed, worker_idx),
                        random_opening_plies,
                        worker_progress_interval,
                        worker_tx.clone(),
                    );
                    let _ = worker_tx.send(WorkerMessage::Done);
                    result.map(|network| (network, worker_games))
                }),
            );
        }
        drop(tx);

        let mut completed_games = 0usize;
        let mut last_reported = 0usize;
        let mut finished_workers = 0usize;
        while finished_workers < worker_game_counts.len() {
            match rx
                .recv()
                .map_err(|_| "training worker progress channel closed unexpectedly".to_string())?
            {
                WorkerMessage::Progress(delta) => {
                    completed_games = completed_games.saturating_add(delta);
                    if let Some(callback) = progress_callback.as_mut()
                        && progress_interval > 0
                        && (completed_games - last_reported >= progress_interval
                            || completed_games == games)
                    {
                        last_reported = completed_games;
                        callback(completed_games, games, start_time.elapsed().as_secs_f64())?;
                    }
                }
                WorkerMessage::Done => finished_workers += 1,
            }
        }

        if let Some(callback) = progress_callback.as_mut()
            && progress_interval > 0
            && last_reported != games
        {
            callback(games, games, start_time.elapsed().as_secs_f64())?;
        }

        let mut workers = Vec::with_capacity(handles.len());
        for handle in handles {
            let worker = handle
                .join()
                .map_err(|_| "training worker thread panicked".to_string())??;
            workers.push(worker);
        }
        TrainableNTuple::merge_weighted(&workers, games)
    })
}

fn train_worker_network(
    network: TrainableNTuple,
    games: usize,
    alpha: f32,
    lambda_: f32,
    epsilon: f64,
    seed: u64,
    random_opening_plies: usize,
    progress_interval: usize,
    progress_tx: mpsc::Sender<WorkerMessage>,
) -> Result<TrainableNTuple, String> {
    let mut trainer =
        TDLambdaTrainer::new(network, alpha, lambda_, epsilon, seed, random_opening_plies)?;

    if progress_interval > 0 {
        let mut reported = 0usize;
        let mut progress = |completed: usize, _total: usize, _elapsed: f64| -> Result<(), String> {
            let delta = completed.saturating_sub(reported);
            reported = completed;
            if delta > 0 {
                progress_tx
                    .send(WorkerMessage::Progress(delta))
                    .map_err(|_| "failed to send worker progress".to_string())?;
            }
            Ok(())
        };
        trainer.train(games, progress_interval, Some(&mut progress))?;
    } else {
        trainer.train(games, 0, None)?;
    }

    Ok(trainer.into_network())
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

fn ordered_moves<F>(board: &Board, is_black: bool, legal: u64, mut score_move: F) -> Vec<usize>
where
    F: FnMut(&Board, bool) -> f32,
{
    let mut scored_moves: Vec<(usize, f32)> = bitboard_to_positions(legal)
        .into_iter()
        .map(|mv| {
            let mut next = *board;
            let flipped = next.place(mv, is_black);
            debug_assert_ne!(flipped, 0, "ordered_moves must only score legal moves");
            let score = score_move(&next, !is_black);
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

fn bitboard_to_positions(mut mask: u64) -> Vec<usize> {
    let mut out = Vec::new();
    while mask != 0 {
        let mv = mask.trailing_zeros() as usize;
        out.push(mv);
        mask &= mask - 1;
    }
    out
}

fn is_better_move(score: f32, mv: usize, best_score: f32, best_move: usize) -> bool {
    score > best_score || (score == best_score && mv < best_move)
}

fn terminal_training_score(board: &Board, is_black: bool) -> f32 {
    let (black_count, white_count) = board.count();
    if is_black {
        black_count as f32 - white_count as f32
    } else {
        white_count as f32 - black_count as f32
    }
}

fn resolve_thread_count(threads: usize) -> usize {
    if threads == 0 {
        std::thread::available_parallelism()
            .map(|parallelism| parallelism.get())
            .unwrap_or(1)
    } else {
        threads
    }
}

fn split_games(games: usize, threads: usize) -> Vec<usize> {
    let base = games / threads;
    let remainder = games % threads;
    (0..threads)
        .map(|idx| base + usize::from(idx < remainder))
        .collect()
}

fn worker_seed(base_seed: u64, worker_idx: usize) -> u64 {
    const GOLDEN_GAMMA: u64 = 0x9E37_79B9_7F4A_7C15;
    base_seed.wrapping_add(GOLDEN_GAMMA.wrapping_mul((worker_idx as u64) + 1))
}

fn phase_index_for_board(board: &Board, phase_count: usize) -> usize {
    let plies = 60usize.saturating_sub(board.empty_count() as usize);
    (plies / 2).min(phase_count.saturating_sub(1))
}

fn transform_pos(pos: u8, symmetry: u8) -> usize {
    const BOARD_SIZE: usize = 8;
    let row = (pos as usize) / BOARD_SIZE;
    let col = (pos as usize) % BOARD_SIZE;

    let (nr, nc) = match symmetry {
        0 => (row, col),
        1 => (col, BOARD_SIZE - 1 - row),
        2 => (BOARD_SIZE - 1 - row, BOARD_SIZE - 1 - col),
        3 => (BOARD_SIZE - 1 - col, row),
        4 => (row, BOARD_SIZE - 1 - col),
        5 => (BOARD_SIZE - 1 - col, BOARD_SIZE - 1 - row),
        6 => (BOARD_SIZE - 1 - row, col),
        _ => (col, row),
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

fn read_u32_le(data: &[u8], offset: usize) -> Result<u32, String> {
    if offset + 4 > data.len() {
        return Err("unexpected EOF while reading u32".to_string());
    }

    let mut bytes = [0u8; 4];
    bytes.copy_from_slice(&data[offset..offset + 4]);
    Ok(u32::from_le_bytes(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::ntuple::{NTupleEvaluator, decompress_model_bytes};

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

    fn board_with_empty_count(empty: u8) -> Board {
        let occupied = 64usize - empty as usize;
        let black = if occupied == 64 {
            u64::MAX
        } else {
            (1u64 << occupied) - 1
        };
        Board::from_bitboards(black, 0)
    }

    #[test]
    fn update_direction_is_toward_td_target() {
        let network = RecordingNetwork {
            value: 0.0,
            updates: Vec::new(),
        };
        let mut trainer = TDLambdaTrainer::new(network, 0.5, 0.0, 0.0, 7, 0).unwrap();
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
            let mut trainer = TDLambdaTrainer::new(network, 1.0, 0.0, 0.0, 11, 0).unwrap();
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
        let mut trainer = TDLambdaTrainer::new(network, 1.0, 0.5, 0.0, 13, 0).unwrap();
        let history = vec![(Board::new(), true), (Board::new(), false)];
        let final_board = Board::from_bitboards(u64::MAX, 0);

        trainer.update_weights(&history, &final_board);

        assert_eq!(trainer.network.updates.len(), 2);
        assert_eq!(trainer.network.updates[0], (false, -1.0));
        assert_eq!(trainer.network.updates[1], (true, 0.5));
    }

    #[test]
    fn phase_index_uses_two_plies_per_phase() {
        assert_eq!(phase_index_for_board(&Board::new(), PHASE_COUNT), 0);
        assert_eq!(
            phase_index_for_board(&board_with_empty_count(59), PHASE_COUNT),
            0
        );
        assert_eq!(
            phase_index_for_board(&board_with_empty_count(58), PHASE_COUNT),
            1
        );
        assert_eq!(
            phase_index_for_board(&board_with_empty_count(1), PHASE_COUNT),
            29
        );
        assert_eq!(
            phase_index_for_board(&board_with_empty_count(0), PHASE_COUNT),
            29
        );
    }

    #[test]
    fn evaluate_reads_only_current_phase_weights() {
        let mut network = TrainableNTuple::new();
        let phase0_board = board_with_empty_count(60);
        let phase1_board = board_with_empty_count(58);
        let phase0_indices = TrainableNTuple::compute_feature_indices(&phase0_board, true);
        let phase1_indices = TrainableNTuple::compute_feature_indices(&phase1_board, true);

        for symmetry in 0..SYMMETRY_COUNT {
            network.weights[0][0][phase0_indices[symmetry][0]] = 1.0;
            network.weights[1][0][phase1_indices[symmetry][0]] = 2.0;
        }

        assert_eq!(network.evaluate(&phase0_board, true), 8.0);
        assert_eq!(network.evaluate(&phase1_board, true), 16.0);
    }

    #[test]
    fn update_only_touches_active_phase() {
        let mut network = TrainableNTuple::new();
        let board = board_with_empty_count(58);
        let indices = TrainableNTuple::compute_feature_indices(&board, true);

        network.update(&board, true, 0.25);

        assert_eq!(network.weights[0][0][indices[0][0]], 0.0);
        assert!(network.weights[1][0][indices[0][0]] > 0.0);
    }

    #[test]
    fn play_one_game_is_reproducible_with_fixed_seed() {
        let mut trainer_a =
            TDLambdaTrainer::new(TrainableNTuple::new(), 0.01, 0.7, 0.3, 2026, 0).unwrap();
        let mut trainer_b =
            TDLambdaTrainer::new(TrainableNTuple::new(), 0.01, 0.7, 0.3, 2026, 0).unwrap();

        trainer_a.play_one_game().unwrap();
        trainer_b.play_one_game().unwrap();

        assert_eq!(
            trainer_a.network.raw_weights(),
            trainer_b.network.raw_weights()
        );
        assert!(trainer_a.network.raw_weights().iter().any(|phase| {
            phase
                .iter()
                .any(|weights| weights.iter().any(|value| *value != 0.0))
        }));
    }

    #[test]
    fn play_one_game_with_random_opening_is_reproducible_with_fixed_seed() {
        let mut trainer_a =
            TDLambdaTrainer::new(TrainableNTuple::new(), 0.01, 0.7, 0.3, 2026, 4).unwrap();
        let mut trainer_b =
            TDLambdaTrainer::new(TrainableNTuple::new(), 0.01, 0.7, 0.3, 2026, 4).unwrap();

        trainer_a.play_one_game().unwrap();
        trainer_b.play_one_game().unwrap();

        assert_eq!(
            trainer_a.network.raw_weights(),
            trainer_b.network.raw_weights()
        );
    }

    #[test]
    fn train_reports_progress_at_interval_and_completion() {
        let mut trainer =
            TDLambdaTrainer::new(TrainableNTuple::new(), 0.0, 0.0, 0.0, 5, 0).unwrap();
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
    fn train_to_bytes_writes_v3_header_with_phase_count() {
        let bytes = train_to_bytes(0, 0.01, 0.7, 0.1, 42, 1, None, 0, 0, None).unwrap();
        let bytes = decompress_model_bytes(&bytes).unwrap();
        assert_eq!(&bytes[0..4], MAGIC);
        assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), VERSION);
        assert_eq!(
            u32::from_le_bytes(bytes[16..20].try_into().unwrap()),
            PHASE_COUNT as u32
        );
    }

    #[test]
    fn exported_bytes_are_readable_by_inference_evaluator() {
        let bytes = train_to_bytes(0, 0.01, 0.7, 0.1, 42, 1, None, 0, 0, None).unwrap();
        let evaluator = NTupleEvaluator::from_bytes(&bytes).unwrap();
        let score = evaluator.evaluate(&Board::new(), true);
        assert_eq!(score, 0.0);
    }

    #[test]
    fn split_games_distributes_remainder_to_earliest_workers() {
        assert_eq!(split_games(10, 3), vec![4, 3, 3]);
        assert_eq!(split_games(2, 4), vec![1, 1, 0, 0]);
    }

    #[test]
    fn resolve_thread_count_zero_uses_available_parallelism() {
        assert!(resolve_thread_count(0) >= 1);
        assert_eq!(resolve_thread_count(3), 3);
    }

    #[test]
    fn parallel_training_is_reproducible_for_fixed_seed_and_thread_count() {
        let first = train_to_bytes(8, 0.01, 0.7, 0.1, 42, 2, None, 0, 0, None).unwrap();
        let second = train_to_bytes(8, 0.01, 0.7, 0.1, 42, 2, None, 0, 0, None).unwrap();

        assert_eq!(first, second);
    }

    #[test]
    fn train_to_bytes_supports_resuming_from_zero_checkpoint() {
        let checkpoint = train_to_bytes(0, 0.01, 0.7, 0.1, 42, 1, None, 0, 0, None).unwrap();
        let resumed =
            train_to_bytes(8, 0.01, 0.7, 0.1, 42, 1, Some(&checkpoint), 0, 0, None).unwrap();
        let fresh = train_to_bytes(8, 0.01, 0.7, 0.1, 42, 1, None, 0, 0, None).unwrap();

        assert_eq!(resumed, fresh);
    }

    #[test]
    fn random_opening_training_is_reproducible_with_fixed_seed() {
        let first = train_to_bytes(8, 0.01, 0.7, 0.1, 42, 2, None, 4, 0, None).unwrap();
        let second = train_to_bytes(8, 0.01, 0.7, 0.1, 42, 2, None, 4, 0, None).unwrap();

        assert_eq!(first, second);
    }

    struct CountingNetwork {
        evaluations: std::cell::Cell<usize>,
    }

    impl CountingNetwork {
        fn new() -> Self {
            Self {
                evaluations: std::cell::Cell::new(0),
            }
        }
    }

    impl TrainingNetwork for CountingNetwork {
        fn evaluate(&self, _board: &Board, _is_black: bool) -> f32 {
            self.evaluations.set(self.evaluations.get() + 1);
            0.0
        }

        fn update(&mut self, _board: &Board, _is_black: bool, _delta: f32) {}
    }

    #[test]
    fn select_move_uses_shallow_search_not_single_ply_greedy() {
        let network = CountingNetwork::new();
        let mut trainer = TDLambdaTrainer::new(network, 0.01, 0.7, 0.0, 17, 0).unwrap();
        let board = Board::new();
        let legal = board.legal_moves(true);

        let mv = trainer.select_move(&board, true, legal).unwrap();

        assert_ne!(legal & (1u64 << mv), 0, "selected move must stay legal");
        assert!(
            trainer.network.evaluations.get() > legal.count_ones() as usize,
            "depth-2 self-play search should evaluate beyond immediate children"
        );
    }
}
