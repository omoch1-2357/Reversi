use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use rand::SeedableRng;
use rand::prelude::SliceRandom;
use rand_chacha::ChaCha8Rng;
use reversi::ai::ntuple::NTupleEvaluator;
use reversi::ai::search::Searcher;
use reversi::board::Board;
use web_time::Duration as WebDuration;

const EMBEDDED_MODEL_BYTES: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/embedded_weights.bin"));
const MAX_GAME_STEPS: usize = 200;
const MIN_LEVEL: u8 = 1;
const MAX_LEVEL: u8 = 6;
const DEFAULT_WEIGHTS_TIMEOUT_MS: u64 = 250;
const DEFAULT_OPPONENT_TIMEOUT_MS: u64 = 250;
const DISABLED_TIMEOUT_SECS: u64 = 60 * 60 * 24 * 365;
const POSITION_WEIGHTS: [i32; 64] = [
    120, -20, 20, 5, 5, 20, -20, 120, -20, -40, -5, -5, -5, -5, -40, -20, 20, -5, 15, 3, 3, 15, -5,
    20, 5, -5, 3, 3, 3, 3, -5, 5, 5, -5, 3, 3, 3, 3, -5, 5, 20, -5, 15, 3, 3, 15, -5, 20, -20, -40,
    -5, -5, -5, -5, -40, -20, 120, -20, 20, 5, 5, 20, -20, 120,
];
const GOLDEN_GAMMA: u64 = 0x9E37_79B9_7F4A_7C15;

#[derive(Clone, Debug)]
struct Config {
    games_per_matchup: usize,
    level: u8,
    seed: u64,
    random_opening_plies: usize,
    weights_timeout_ms: u64,
    opponent_timeout_ms: u64,
    weights_path: Option<PathBuf>,
    opponent_weights_path: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug)]
enum Opponent<'a> {
    Random,
    PositionalSearch,
    WeightsModel(&'a NTupleEvaluator),
}

#[derive(Default)]
struct MatchStats {
    games: usize,
    weights_black_games: usize,
    weights_white_games: usize,
    wins: usize,
    losses: usize,
    draws: usize,
    final_diffs: Vec<f64>,
    weights_move_ms: Vec<f64>,
    opponent_move_ms: Vec<f64>,
}

fn main() -> Result<(), String> {
    let config = parse_args(env::args().skip(1).collect())?;
    let evaluator = load_evaluator(config.weights_path.as_ref())?;
    let opponent_evaluator = load_optional_evaluator(config.opponent_weights_path.as_ref())?;
    let primary_label = model_source_label(config.weights_path.as_ref());

    println!(
        "Benchmarking weights.bin AI: games_per_matchup={}, level={}, seed={}, random_opening_plies={}, weights_timeout_ms={}, opponent_timeout_ms={}",
        config.games_per_matchup,
        config.level,
        config.seed,
        config.random_opening_plies,
        config.weights_timeout_ms,
        config.opponent_timeout_ms
    );
    if let Some(path) = &config.weights_path {
        println!("Model source: {}", path.display());
    } else {
        println!("Model source: embedded model in current binary");
    }
    if let Some(path) = &config.opponent_weights_path {
        println!("Opponent model source: {}", path.display());
    }
    println!();

    if let Some(opponent_evaluator) = opponent_evaluator.as_ref() {
        let opponent_label = model_source_label(config.opponent_weights_path.as_ref());
        let stats = benchmark_matchup(
            &evaluator,
            Opponent::WeightsModel(opponent_evaluator),
            &config,
            config.seed.wrapping_add(GOLDEN_GAMMA),
        )?;
        print_stats(
            &format!("{primary_label} vs {opponent_label}"),
            "primary_model_move_ms",
            "opponent_model_move_ms",
            &stats,
        );
    } else {
        for (offset, (opponent, label)) in [
            (Opponent::Random, "random"),
            (Opponent::PositionalSearch, "positional-search"),
        ]
        .into_iter()
        .enumerate()
        {
            let stats = benchmark_matchup(
                &evaluator,
                opponent,
                &config,
                config
                    .seed
                    .wrapping_add(GOLDEN_GAMMA.wrapping_mul((offset as u64) + 1)),
            )?;
            print_stats(
                &format!("{primary_label} vs {label}"),
                "weights_move_ms",
                "opponent_move_ms",
                &stats,
            );
        }
    }

    Ok(())
}

fn parse_args(args: Vec<String>) -> Result<Config, String> {
    let mut config = Config {
        games_per_matchup: 20,
        level: 4,
        seed: 42,
        random_opening_plies: 0,
        weights_timeout_ms: DEFAULT_WEIGHTS_TIMEOUT_MS,
        opponent_timeout_ms: DEFAULT_OPPONENT_TIMEOUT_MS,
        weights_path: None,
        opponent_weights_path: None,
    };

    let mut idx = 0usize;
    while idx < args.len() {
        match args[idx].as_str() {
            "--games" => {
                idx += 1;
                config.games_per_matchup = parse_value(&args, idx, "--games")?;
            }
            "--level" => {
                idx += 1;
                config.level = parse_value(&args, idx, "--level")?;
            }
            "--seed" => {
                idx += 1;
                config.seed = parse_value(&args, idx, "--seed")?;
            }
            "--random-opening-plies" => {
                idx += 1;
                config.random_opening_plies = parse_value(&args, idx, "--random-opening-plies")?;
            }
            "--weights-timeout-ms" => {
                idx += 1;
                config.weights_timeout_ms = parse_value(&args, idx, "--weights-timeout-ms")?;
            }
            "--opponent-timeout-ms" => {
                idx += 1;
                config.opponent_timeout_ms = parse_value(&args, idx, "--opponent-timeout-ms")?;
            }
            "--weights-path" => {
                idx += 1;
                let raw = args
                    .get(idx)
                    .ok_or_else(|| "missing value for --weights-path".to_string())?;
                config.weights_path = Some(PathBuf::from(raw));
            }
            "--opponent-weights-path" => {
                idx += 1;
                let raw = args
                    .get(idx)
                    .ok_or_else(|| "missing value for --opponent-weights-path".to_string())?;
                config.opponent_weights_path = Some(PathBuf::from(raw));
            }
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument: {other}")),
        }
        idx += 1;
    }

    if config.games_per_matchup == 0 {
        return Err("games must be greater than 0".to_string());
    }
    if !(MIN_LEVEL..=MAX_LEVEL).contains(&config.level) {
        return Err(format!("level must be in {MIN_LEVEL}..={MAX_LEVEL}"));
    }
    if config.weights_timeout_ms == 0 {
        return Err("weights-timeout-ms must be greater than 0".to_string());
    }

    Ok(config)
}

fn parse_value<T: std::str::FromStr>(args: &[String], idx: usize, flag: &str) -> Result<T, String> {
    args.get(idx)
        .ok_or_else(|| format!("missing value for {flag}"))?
        .parse::<T>()
        .map_err(|_| format!("invalid value for {flag}"))
}

fn print_usage() {
    println!(
        "Usage: cargo run --manifest-path rust/Cargo.toml --bin benchmark_matchups -- [options]\n\
         \n\
         Options:\n\
           --games <N>                 Number of games per matchup (default: 20)\n\
           --level <1-6>               Search depth level for weights AI and positional player (default: 4)\n\
           --seed <N>                  Base seed for random opponent/openings (default: 42)\n\
           --random-opening-plies <N>  Random plies applied before benchmark players take over (default: 0)\n\
           --weights-timeout-ms <N>    Per-move timeout for weights.bin AI in milliseconds (default: 250)\n\
           --opponent-timeout-ms <N>   Per-move timeout for positional-search opponent; 0 disables the limit (default: 250)\n\
           --weights-path <PATH>       Optional external weights.bin to benchmark instead of embedded model\n\
           --opponent-weights-path <PATH>\n\
                                      Optional external weights.bin for direct model-vs-model benchmark\n\
           --help                      Show this message"
    );
}

fn load_evaluator(weights_path: Option<&PathBuf>) -> Result<NTupleEvaluator, String> {
    if let Some(path) = weights_path {
        let bytes = fs::read(path)
            .map_err(|err| format!("failed to read model bytes from {}: {err}", path.display()))?;
        NTupleEvaluator::from_bytes(&bytes)
    } else {
        NTupleEvaluator::from_bytes(EMBEDDED_MODEL_BYTES)
    }
}

fn load_optional_evaluator(
    weights_path: Option<&PathBuf>,
) -> Result<Option<NTupleEvaluator>, String> {
    weights_path
        .map(|path| load_evaluator(Some(path)))
        .transpose()
}

fn model_source_label(weights_path: Option<&PathBuf>) -> String {
    weights_path
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "embedded model".to_string())
}

fn benchmark_matchup(
    evaluator: &NTupleEvaluator,
    opponent: Opponent<'_>,
    config: &Config,
    seed: u64,
) -> Result<MatchStats, String> {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let mut stats = MatchStats::default();

    for game_idx in 0..config.games_per_matchup {
        let weights_is_black = game_idx % 2 == 0;
        if weights_is_black {
            stats.weights_black_games += 1;
        } else {
            stats.weights_white_games += 1;
        }

        let outcome = play_game(
            evaluator,
            opponent,
            config.level,
            config.random_opening_plies,
            config.weights_timeout_ms,
            config.opponent_timeout_ms,
            weights_is_black,
            &mut rng,
        )?;

        stats.games += 1;
        if outcome.final_diff > 0.0 {
            stats.wins += 1;
        } else if outcome.final_diff < 0.0 {
            stats.losses += 1;
        } else {
            stats.draws += 1;
        }
        stats.final_diffs.push(outcome.final_diff);
        stats.weights_move_ms.extend(outcome.weights_move_ms);
        stats.opponent_move_ms.extend(outcome.opponent_move_ms);
    }

    Ok(stats)
}

struct GameOutcome {
    final_diff: f64,
    weights_move_ms: Vec<f64>,
    opponent_move_ms: Vec<f64>,
}

fn play_game(
    evaluator: &NTupleEvaluator,
    opponent: Opponent<'_>,
    level: u8,
    random_opening_plies: usize,
    weights_timeout_ms: u64,
    opponent_timeout_ms: u64,
    weights_is_black: bool,
    rng: &mut ChaCha8Rng,
) -> Result<GameOutcome, String> {
    let mut board = Board::new();
    let mut current_is_black = true;
    let mut placed_plies = 0usize;
    let mut steps = 0usize;
    let mut weights_move_ms = Vec::new();
    let mut opponent_move_ms = Vec::new();

    while placed_plies < random_opening_plies {
        if !play_random_ply(&mut board, &mut current_is_black, rng)? {
            break;
        }
        placed_plies += 1;
    }

    loop {
        if steps > MAX_GAME_STEPS {
            return Err(format!(
                "game exceeded {MAX_GAME_STEPS} steps without terminating"
            ));
        }

        let legal = board.legal_moves(current_is_black);
        if legal == 0 {
            let opp_legal = board.legal_moves(!current_is_black);
            if opp_legal == 0 {
                break;
            }
            current_is_black = !current_is_black;
            continue;
        }

        let weights_turn = current_is_black == weights_is_black;
        let started = Instant::now();
        let mv = if weights_turn {
            let mut searcher =
                Searcher::with_timeout(evaluator, level, model_timeout(weights_timeout_ms));
            searcher.search(&board, current_is_black)
        } else {
            match opponent {
                Opponent::Random => random_move(legal, rng)
                    .ok_or_else(|| "random opponent failed to choose move".to_string())?,
                Opponent::PositionalSearch => {
                    choose_positional_move(&board, current_is_black, level, opponent_timeout_ms)
                        .ok_or_else(|| "positional opponent failed to choose move".to_string())?
                }
                Opponent::WeightsModel(opponent_evaluator) => {
                    let mut searcher = Searcher::with_timeout(
                        opponent_evaluator,
                        level,
                        model_timeout(opponent_timeout_ms),
                    );
                    searcher.search(&board, current_is_black)
                }
            }
        };
        let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;

        let mut next = board;
        if next.place(mv, current_is_black) == 0 {
            return Err(format!("selected illegal move {mv}"));
        }
        board = next;
        current_is_black = !current_is_black;
        steps += 1;

        if weights_turn {
            weights_move_ms.push(elapsed_ms);
        } else {
            opponent_move_ms.push(elapsed_ms);
        }
    }

    let (black, white) = board.count();
    let final_diff = if weights_is_black {
        black as f64 - white as f64
    } else {
        white as f64 - black as f64
    };

    Ok(GameOutcome {
        final_diff,
        weights_move_ms,
        opponent_move_ms,
    })
}

fn play_random_ply(
    board: &mut Board,
    current_is_black: &mut bool,
    rng: &mut ChaCha8Rng,
) -> Result<bool, String> {
    loop {
        let legal = board.legal_moves(*current_is_black);
        if legal == 0 {
            let opp_legal = board.legal_moves(!*current_is_black);
            if opp_legal == 0 {
                return Ok(false);
            }
            *current_is_black = !*current_is_black;
            continue;
        }

        let mv = random_move(legal, rng)
            .ok_or_else(|| "failed to sample random opening move".to_string())?;
        if board.place(mv, *current_is_black) == 0 {
            return Err(format!("random opening selected illegal move {mv}"));
        }
        *current_is_black = !*current_is_black;
        return Ok(true);
    }
}

fn random_move(legal: u64, rng: &mut ChaCha8Rng) -> Option<usize> {
    let mut moves = bitboard_to_positions(legal);
    moves.shuffle(rng);
    moves.into_iter().next()
}

fn choose_positional_move(
    board: &Board,
    is_black: bool,
    depth: u8,
    timeout_ms: u64,
) -> Option<usize> {
    let legal = board.legal_moves(is_black);
    if legal == 0 {
        return None;
    }

    let deadline = if timeout_ms == 0 {
        None
    } else {
        Some(Instant::now() + Duration::from_millis(timeout_ms))
    };
    let moves = sort_moves_for_positional_search(legal, board, is_black);
    let mut best_move = moves[0];
    let mut best_score = f32::NEG_INFINITY;
    let mut alpha = f32::NEG_INFINITY;
    let beta = f32::INFINITY;

    for mv in moves {
        if deadline.is_some_and(|limit| Instant::now() >= limit) {
            break;
        }
        let mut next = *board;
        let _ = next.place(mv, is_black);
        let score = -positional_negamax(
            &next,
            !is_black,
            depth.saturating_sub(1),
            -beta,
            -alpha,
            deadline,
        );
        if is_better_move(board, score, mv, best_score, best_move) {
            best_score = score;
            best_move = mv;
        }
        if score > alpha {
            alpha = score;
        }
    }

    Some(best_move)
}

fn positional_negamax(
    board: &Board,
    is_black: bool,
    depth: u8,
    alpha: f32,
    beta: f32,
    deadline: Option<Instant>,
) -> f32 {
    if deadline.is_some_and(|limit| Instant::now() >= limit) {
        return positional_eval(board, is_black);
    }
    if depth == 0 {
        return positional_eval(board, is_black);
    }

    let legal = board.legal_moves(is_black);
    if legal == 0 {
        let opp_legal = board.legal_moves(!is_black);
        if opp_legal == 0 {
            return terminal_score(board, is_black);
        }
        return -positional_negamax(board, !is_black, depth, -beta, -alpha, deadline);
    }

    let moves = sort_moves_for_positional_search(legal, board, is_black);
    let mut alpha = alpha;
    let mut best = f32::NEG_INFINITY;
    let mut best_move = moves[0];

    for mv in moves {
        if deadline.is_some_and(|limit| Instant::now() >= limit) {
            break;
        }
        let mut next = *board;
        let _ = next.place(mv, is_black);
        let score = -positional_negamax(&next, !is_black, depth - 1, -beta, -alpha, deadline);
        if is_better_move(board, score, mv, best, best_move) {
            best = score;
            best_move = mv;
        }
        if score > alpha {
            alpha = score;
        }
        if alpha >= beta {
            break;
        }
    }

    best
}

fn sort_moves_for_positional_search(legal: u64, board: &Board, is_black: bool) -> Vec<usize> {
    let tie_break_symmetry = canonical_symmetry(board);
    let mut scored_moves: Vec<(usize, f32)> = bitboard_to_positions(legal)
        .into_iter()
        .map(|mv| {
            let mut next = *board;
            let _ = next.place(mv, is_black);
            (mv, -positional_eval(&next, !is_black))
        })
        .collect();

    scored_moves.sort_by(|(left_mv, left_score), (right_mv, right_score)| {
        right_score.total_cmp(left_score).then_with(|| {
            transform_pos(*left_mv as u8, tie_break_symmetry)
                .cmp(&transform_pos(*right_mv as u8, tie_break_symmetry))
        })
    });

    scored_moves.into_iter().map(|(mv, _)| mv).collect()
}

fn positional_eval(board: &Board, is_black: bool) -> f32 {
    let cells = board.to_array();
    let mut black_positional = 0i32;
    let mut white_positional = 0i32;

    for (idx, cell) in cells.iter().enumerate() {
        match *cell {
            1 => black_positional += POSITION_WEIGHTS[idx],
            2 => white_positional += POSITION_WEIGHTS[idx],
            _ => {}
        }
    }

    let black_mobility = board.legal_moves(true).count_ones() as i32;
    let white_mobility = board.legal_moves(false).count_ones() as i32;
    let black_corners = corners_taken(&cells, 1) as i32;
    let white_corners = corners_taken(&cells, 2) as i32;
    let (black_count, white_count) = board.count();
    let black_score = (black_positional - white_positional)
        + 5 * (black_mobility - white_mobility)
        + 25 * (black_corners - white_corners)
        + (black_count as i32 - white_count as i32);

    let score = if is_black { black_score } else { -black_score };
    score as f32
}

fn corners_taken(cells: &[u8; 64], stone: u8) -> usize {
    [0usize, 7, 56, 63]
        .into_iter()
        .filter(|&idx| cells[idx] == stone)
        .count()
}

fn terminal_score(board: &Board, is_black: bool) -> f32 {
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

fn is_better_move(board: &Board, score: f32, mv: usize, best_score: f32, best_move: usize) -> bool {
    let tie_break_symmetry = canonical_symmetry(board);
    let move_key = transform_pos(mv as u8, tie_break_symmetry);
    let best_key = transform_pos(best_move as u8, tie_break_symmetry);

    score > best_score || (score == best_score && move_key < best_key)
}

fn canonical_symmetry(board: &Board) -> u8 {
    let cells = board.to_array();
    let mut best = None;

    for symmetry in 0..8u8 {
        let transformed = transform_cells(&cells, symmetry);
        if best
            .as_ref()
            .is_none_or(|(current, current_sym): &(Vec<u8>, u8)| {
                transformed < *current || (transformed == *current && symmetry < *current_sym)
            })
        {
            best = Some((transformed, symmetry));
        }
    }

    best.expect("at least one symmetry must exist").1
}

fn transform_cells(cells: &[u8; 64], symmetry: u8) -> Vec<u8> {
    let mut transformed = vec![0u8; 64];
    for (pos, value) in cells.iter().copied().enumerate() {
        transformed[transform_pos(pos as u8, symmetry)] = value;
    }
    transformed
}

fn transform_pos(pos: u8, symmetry: u8) -> usize {
    let row = (pos as usize) / 8;
    let col = (pos as usize) % 8;

    let (nr, nc) = match symmetry {
        0 => (row, col),
        1 => (col, 7 - row),
        2 => (7 - row, 7 - col),
        3 => (7 - col, row),
        4 => (row, 7 - col),
        5 => (7 - col, 7 - row),
        6 => (7 - row, col),
        _ => (col, row),
    };

    nr * 8 + nc
}

fn print_stats(
    label: &str,
    primary_move_label: &str,
    opponent_move_label: &str,
    stats: &MatchStats,
) {
    println!("{label}");
    println!(
        "  games={} (black={}, white={})",
        stats.games, stats.weights_black_games, stats.weights_white_games
    );
    println!(
        "  record={}W-{}L-{}D  win_rate={:.1}% draw_rate={:.1}%",
        stats.wins,
        stats.losses,
        stats.draws,
        percentage(stats.wins, stats.games),
        percentage(stats.draws, stats.games)
    );
    println!(
        "  final_diff(avg/p50/p95/min/max) = {:.2} / {:.2} / {:.2} / {:.2} / {:.2}",
        mean(&stats.final_diffs),
        percentile(&stats.final_diffs, 50),
        percentile(&stats.final_diffs, 95),
        min_value(&stats.final_diffs),
        max_value(&stats.final_diffs)
    );
    println!(
        "  {primary_move_label}(avg/p95/max) = {:.2} / {:.2} / {:.2}",
        mean(&stats.weights_move_ms),
        percentile(&stats.weights_move_ms, 95),
        max_value(&stats.weights_move_ms)
    );
    println!(
        "  {opponent_move_label}(avg/p95/max) = {:.2} / {:.2} / {:.2}",
        mean(&stats.opponent_move_ms),
        percentile(&stats.opponent_move_ms, 95),
        max_value(&stats.opponent_move_ms)
    );
    println!();
}

fn model_timeout(timeout_ms: u64) -> WebDuration {
    if timeout_ms == 0 {
        WebDuration::from_secs(DISABLED_TIMEOUT_SECS)
    } else {
        WebDuration::from_millis(timeout_ms)
    }
}

fn percentage(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        (numerator as f64) * 100.0 / (denominator as f64)
    }
}

fn mean(samples: &[f64]) -> f64 {
    if samples.is_empty() {
        0.0
    } else {
        samples.iter().sum::<f64>() / (samples.len() as f64)
    }
}

fn percentile(samples: &[f64], pct: usize) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }

    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);
    let rank = sorted
        .len()
        .saturating_mul(pct)
        .div_ceil(100)
        .saturating_sub(1);
    sorted[rank]
}

fn min_value(samples: &[f64]) -> f64 {
    samples
        .iter()
        .copied()
        .min_by(f64::total_cmp)
        .unwrap_or(0.0)
}

fn max_value(samples: &[f64]) -> f64 {
    samples
        .iter()
        .copied()
        .max_by(f64::total_cmp)
        .unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::parse_args;

    #[test]
    fn parse_args_supports_direct_model_matchup() {
        let config = parse_args(vec![
            "--games".to_string(),
            "40".to_string(),
            "--weights-path".to_string(),
            "models/current.bin".to_string(),
            "--opponent-weights-path".to_string(),
            "models/challenger.bin".to_string(),
        ])
        .expect("args should parse");

        assert_eq!(config.games_per_matchup, 40);
        assert_eq!(
            config
                .weights_path
                .as_deref()
                .and_then(|path| path.to_str()),
            Some("models/current.bin")
        );
        assert_eq!(
            config
                .opponent_weights_path
                .as_deref()
                .and_then(|path| path.to_str()),
            Some("models/challenger.bin")
        );
    }

    #[test]
    fn parse_args_rejects_missing_opponent_weights_path_value() {
        let err = parse_args(vec!["--opponent-weights-path".to_string()])
            .expect_err("missing value should fail");

        assert!(err.contains("missing value for --opponent-weights-path"));
    }
}
