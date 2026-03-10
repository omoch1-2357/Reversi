use std::borrow::Cow;
use std::io::Cursor;

use crate::board::Board;

const MAGIC: &[u8; 4] = b"NTRV";
const ZSTD_MAGIC: &[u8; 4] = &[0x28, 0xB5, 0x2F, 0xFD];
const ZSTD_LEVEL: i32 = 19;
const VERSION_V1: u32 = 1;
const VERSION_V2: u32 = 2;
const VERSION_V3: u32 = 3;
const HEADER_SIZE: usize = 20;
const BOARD_SIZE: usize = 8;
const BOARD_CELLS: usize = BOARD_SIZE * BOARD_SIZE;
const SYMMETRY_NORMALIZATION_ROTATIONS4: f32 = 0.25;
const SYMMETRY_NORMALIZATION_DIHEDRAL8: f32 = 0.125;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SymmetryMode {
    Rotations4,
    Dihedral8,
}

impl SymmetryMode {
    fn count(self) -> u8 {
        match self {
            Self::Rotations4 => 4,
            Self::Dihedral8 => 8,
        }
    }
}

/// Inference-time N-Tuple evaluator loaded from `weights.bin`.
#[derive(Debug, Clone)]
pub struct NTupleEvaluator {
    tuples: Vec<Vec<u8>>,
    phase_count: usize,
    weights: Vec<Vec<Vec<f32>>>,
    symmetry_mode: SymmetryMode,
}

pub fn compress_model_bytes(data: &[u8]) -> Result<Vec<u8>, String> {
    zstd::stream::encode_all(Cursor::new(data), ZSTD_LEVEL)
        .map_err(|err| format!("failed to zstd-compress weights: {err}"))
}

pub fn decompress_model_bytes(data: &[u8]) -> Result<Cow<'_, [u8]>, String> {
    if data.starts_with(ZSTD_MAGIC.as_slice()) {
        let decoded = zstd::stream::decode_all(Cursor::new(data))
            .map_err(|err| format!("failed to zstd-decompress weights: {err}"))?;
        Ok(Cow::Owned(decoded))
    } else {
        Ok(Cow::Borrowed(data))
    }
}

impl NTupleEvaluator {
    /// Deserialize evaluator data from `weights.bin` format.
    pub fn from_bytes(data: &[u8]) -> Result<Self, String> {
        let bytes = decompress_model_bytes(data)?;
        Self::from_uncompressed_bytes(bytes.as_ref())
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
        let num_tuples = read_u32_le(data, 8)? as usize;
        let expected_crc = read_u32_le(data, 12)?;
        let (phase_count, symmetry_mode) = match version {
            VERSION_V1 => (1, SymmetryMode::Rotations4),
            VERSION_V2 => {
                let count = read_u32_le(data, 16)? as usize;
                if count == 0 {
                    return Err("phase_count must be greater than 0".to_string());
                }
                (count, SymmetryMode::Rotations4)
            }
            VERSION_V3 => {
                let count = read_u32_le(data, 16)? as usize;
                if count == 0 {
                    return Err("phase_count must be greater than 0".to_string());
                }
                (count, SymmetryMode::Dihedral8)
            }
            _ => {
                return Err(format!(
                    "unsupported weights version: expected {VERSION_V1}, {VERSION_V2}, or {VERSION_V3}, got {version}"
                ));
            }
        };
        let payload = &data[HEADER_SIZE..];

        let actual_crc = crc32fast::hash(payload);
        if actual_crc != expected_crc {
            return Err(format!(
                "CRC32 mismatch: expected {expected_crc:#010x}, got {actual_crc:#010x}"
            ));
        }

        let mut offset = 0usize;
        let mut tuples = Vec::with_capacity(num_tuples);
        for tuple_idx in 0..num_tuples {
            if offset >= payload.len() {
                return Err(format!(
                    "unexpected EOF while reading tuple definition #{tuple_idx}"
                ));
            }

            let tuple_size = payload[offset] as usize;
            offset += 1;

            if offset + tuple_size > payload.len() {
                return Err(format!(
                    "unexpected EOF while reading tuple positions #{tuple_idx}"
                ));
            }

            let tuple = payload[offset..offset + tuple_size].to_vec();
            if tuple.iter().any(|&pos| pos as usize >= BOARD_CELLS) {
                return Err(format!(
                    "tuple #{tuple_idx} contains out-of-range board position"
                ));
            }
            offset += tuple_size;
            tuples.push(tuple);
        }

        let mut weights = Vec::with_capacity(phase_count);
        for phase_idx in 0..phase_count {
            let mut phase_weights = Vec::with_capacity(num_tuples);
            for (tuple_idx, tuple) in tuples.iter().enumerate() {
                let entries = pow3(tuple.len())?;
                let bytes_len = entries
                    .checked_mul(4)
                    .ok_or_else(|| "weights byte length overflow".to_string())?;

                if offset + bytes_len > payload.len() {
                    return Err(format!(
                        "unexpected EOF while reading weights for phase #{phase_idx}, tuple #{tuple_idx}"
                    ));
                }

                let mut tuple_weights = Vec::with_capacity(entries);
                for i in 0..entries {
                    let start = offset + i * 4;
                    let mut chunk = [0u8; 4];
                    chunk.copy_from_slice(&payload[start..start + 4]);
                    let value = f32::from_le_bytes(chunk);
                    if !value.is_finite() {
                        return Err(format!(
                            "non-finite weight at phase #{phase_idx}, tuple #{tuple_idx}, entry #{i}"
                        ));
                    }
                    tuple_weights.push(value);
                }

                offset += bytes_len;
                phase_weights.push(tuple_weights);
            }
            weights.push(phase_weights);
        }

        if offset != payload.len() {
            return Err("weights payload has trailing bytes".to_string());
        }

        Ok(Self {
            tuples,
            phase_count,
            weights,
            symmetry_mode,
        })
    }

    /// Evaluate from the side-to-move perspective.
    pub fn evaluate(&self, board: &Board, is_black: bool) -> f32 {
        let cells = board.to_array();
        let phase_idx = phase_index_for_board(board, self.phase_count);
        let phase_weights = &self.weights[phase_idx];
        let mut score = 0.0f32;

        for symmetry in 0..self.symmetry_mode.count() {
            for (tuple, weights) in self.tuples.iter().zip(phase_weights.iter()) {
                let idx = tuple.iter().fold(0usize, |acc, &pos| {
                    let transformed = transform_pos(pos, symmetry);
                    let value = map_to_player_view(cells[transformed], is_black) as usize;
                    acc * 3 + value
                });
                score += weights[idx];
            }
        }

        score
            * match self.symmetry_mode {
                SymmetryMode::Rotations4 => SYMMETRY_NORMALIZATION_ROTATIONS4,
                SymmetryMode::Dihedral8 => SYMMETRY_NORMALIZATION_DIHEDRAL8,
            }
    }
}

fn phase_index_for_board(board: &Board, phase_count: usize) -> usize {
    let plies = 60usize.saturating_sub(board.empty_count() as usize);
    (plies / 2).min(phase_count.saturating_sub(1))
}

fn transform_pos(pos: u8, symmetry: u8) -> usize {
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

fn map_to_player_view(cell: u8, is_black: bool) -> u8 {
    match (cell, is_black) {
        (0, _) => 0,
        (1, true) | (2, false) => 1,
        (2, true) | (1, false) => 2,
        _ => 0,
    }
}

fn read_u32_le(data: &[u8], offset: usize) -> Result<u32, String> {
    if offset + 4 > data.len() {
        return Err("unexpected EOF while reading u32".to_string());
    }
    let mut bytes = [0u8; 4];
    bytes.copy_from_slice(&data[offset..offset + 4]);
    Ok(u32::from_le_bytes(bytes))
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

    fn build_weights_blob_v1(tuples: &[Vec<u8>], weights: &[Vec<f32>]) -> Vec<u8> {
        let phase_weights = vec![weights.to_vec()];
        build_weights_blob(VERSION_V1, tuples, &phase_weights, 0)
    }

    fn build_weights_blob_v2(
        tuples: &[Vec<u8>],
        phase_weights: &[Vec<Vec<f32>>],
        phase_count: u32,
    ) -> Vec<u8> {
        build_weights_blob(VERSION_V2, tuples, phase_weights, phase_count)
    }

    fn build_weights_blob_v3(
        tuples: &[Vec<u8>],
        phase_weights: &[Vec<Vec<f32>>],
        phase_count: u32,
    ) -> Vec<u8> {
        build_weights_blob(VERSION_V3, tuples, phase_weights, phase_count)
    }

    fn build_weights_blob(
        version: u32,
        tuples: &[Vec<u8>],
        phase_weights: &[Vec<Vec<f32>>],
        phase_count: u32,
    ) -> Vec<u8> {
        let expected_phases = if version == VERSION_V1 {
            1usize
        } else {
            phase_count as usize
        };
        assert_eq!(phase_weights.len(), expected_phases);
        assert!(expected_phases > 0);
        for weights in phase_weights {
            assert_eq!(tuples.len(), weights.len());
        }

        let mut payload = Vec::new();
        for tuple in tuples {
            payload.push(tuple.len() as u8);
            payload.extend_from_slice(tuple);
        }
        for weights in phase_weights {
            for tuple_weights in weights {
                for value in tuple_weights {
                    payload.extend_from_slice(&value.to_le_bytes());
                }
            }
        }

        let crc = crc32fast::hash(&payload);
        let mut out = Vec::new();
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&version.to_le_bytes());
        out.extend_from_slice(&(tuples.len() as u32).to_le_bytes());
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&phase_count.to_le_bytes());
        out.extend_from_slice(&payload);
        out
    }

    fn bit(pos: usize) -> u64 {
        1u64 << pos
    }

    fn board_with_empty_count(empty: u8) -> Board {
        let occupied = BOARD_CELLS - empty as usize;
        let mut black = bit(0) | bit(7) | bit(56) | bit(63);
        let mut pos = 1usize;

        while (black.count_ones() as usize) < occupied {
            if pos < BOARD_CELLS {
                let square = bit(pos);
                if (black & square) == 0 {
                    black |= square;
                }
                pos += 1;
            } else {
                break;
            }
        }

        Board::from_bitboards(black, 0)
    }

    #[test]
    fn from_bytes_deserializes_v2_tuple_defs_and_weights() {
        let tuples = vec![vec![0, 1], vec![63]];
        let phase_weights = vec![
            vec![
                vec![0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0],
                vec![-1.0, 2.0, 0.25],
            ],
            vec![
                vec![10.0, 10.5, 11.0, 11.5, 12.0, 12.5, 13.0, 13.5, 14.0],
                vec![3.0, 4.0, 5.0],
            ],
        ];
        let bytes = build_weights_blob_v2(&tuples, &phase_weights, 2);

        let evaluator = NTupleEvaluator::from_bytes(&bytes).expect("must parse");

        assert_eq!(evaluator.tuples, tuples);
        assert_eq!(evaluator.phase_count, 2);
        assert_eq!(evaluator.weights, phase_weights);
        assert_eq!(evaluator.symmetry_mode, SymmetryMode::Rotations4);
    }

    #[test]
    fn from_bytes_deserializes_v3_tuple_defs_and_weights() {
        let tuples = vec![vec![0, 1]];
        let phase_weights = vec![
            vec![vec![0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0]],
            vec![vec![10.0, 10.5, 11.0, 11.5, 12.0, 12.5, 13.0, 13.5, 14.0]],
        ];
        let bytes = build_weights_blob_v3(&tuples, &phase_weights, 2);

        let evaluator = NTupleEvaluator::from_bytes(&bytes).expect("must parse");

        assert_eq!(evaluator.tuples, tuples);
        assert_eq!(evaluator.phase_count, 2);
        assert_eq!(evaluator.weights, phase_weights);
        assert_eq!(evaluator.symmetry_mode, SymmetryMode::Dihedral8);
    }

    #[test]
    fn from_bytes_normalizes_v1_to_single_phase() {
        let tuples = vec![vec![0]];
        let weights = vec![vec![0.0, 1.0, -1.0]];
        let bytes = build_weights_blob_v1(&tuples, &weights);

        let evaluator = NTupleEvaluator::from_bytes(&bytes).expect("must parse");

        assert_eq!(evaluator.tuples, tuples);
        assert_eq!(evaluator.phase_count, 1);
        assert_eq!(evaluator.weights, vec![weights]);
        assert_eq!(evaluator.symmetry_mode, SymmetryMode::Rotations4);
    }

    #[test]
    fn from_bytes_rejects_invalid_magic() {
        let tuples = vec![vec![0]];
        let weights = vec![vec![0.0, 1.0, -1.0]];
        let mut bytes = build_weights_blob_v1(&tuples, &weights);
        bytes[0] = b'X';

        let err = NTupleEvaluator::from_bytes(&bytes).unwrap_err();
        assert!(err.contains("magic"));
    }

    #[test]
    fn from_bytes_rejects_unsupported_version() {
        let tuples = vec![vec![0]];
        let weights = vec![vec![vec![0.0, 1.0, -1.0]]];
        let mut bytes = build_weights_blob(99, &tuples, &weights, 1);
        bytes[4..8].copy_from_slice(&99u32.to_le_bytes());

        let err = NTupleEvaluator::from_bytes(&bytes).unwrap_err();
        assert!(err.contains("version"));
    }

    #[test]
    fn from_bytes_rejects_crc_mismatch() {
        let tuples = vec![vec![0]];
        let weights = vec![vec![vec![0.0, 1.0, -1.0]]];
        let mut bytes = build_weights_blob_v2(&tuples, &weights, 1);
        let last = bytes.len() - 1;
        bytes[last] ^= 0x01;

        let err = NTupleEvaluator::from_bytes(&bytes).unwrap_err();
        assert!(err.contains("CRC32"));
    }

    #[test]
    fn from_bytes_rejects_zero_phase_count_for_v2() {
        let tuples = vec![vec![0]];
        let weights = vec![vec![vec![0.0, 1.0, -1.0]]];
        let mut bytes = build_weights_blob(VERSION_V2, &tuples, &weights, 1);
        bytes[16..20].copy_from_slice(&0u32.to_le_bytes());

        let err = NTupleEvaluator::from_bytes(&bytes).unwrap_err();
        assert!(err.contains("phase_count"));
    }

    #[test]
    fn from_bytes_rejects_truncated_weights_payload() {
        let tuples = vec![vec![0, 1]];
        let weights = vec![vec![vec![0.0; 9]], vec![vec![1.0; 9]]];
        let mut bytes = build_weights_blob_v2(&tuples, &weights, 2);
        bytes.pop();
        let recalculated_crc = crc32fast::hash(&bytes[HEADER_SIZE..]);
        bytes[12..16].copy_from_slice(&recalculated_crc.to_le_bytes());

        let err = NTupleEvaluator::from_bytes(&bytes).unwrap_err();
        assert!(err.contains("unexpected EOF while reading weights"));
    }

    #[test]
    fn from_bytes_rejects_non_finite_weights() {
        let tuples = vec![vec![0]];
        let weights = vec![vec![vec![0.0, f32::NAN, -1.0]]];
        let bytes = build_weights_blob_v2(&tuples, &weights, 1);

        let err = NTupleEvaluator::from_bytes(&bytes).unwrap_err();
        assert!(err.contains("non-finite weight"));
    }

    #[test]
    fn from_bytes_accepts_zstd_compressed_payload() {
        let tuples = vec![vec![0, 1]];
        let weights = vec![vec![vec![0.0; 9]], vec![vec![1.0; 9]]];
        let bytes = build_weights_blob_v2(&tuples, &weights, 2);
        let compressed = compress_model_bytes(&bytes).expect("must compress");

        let evaluator = NTupleEvaluator::from_bytes(&compressed).expect("must parse");

        assert_eq!(evaluator.tuples, tuples);
        assert_eq!(evaluator.phase_count, 2);
        assert_eq!(evaluator.weights, weights);
    }

    #[test]
    fn evaluate_applies_rotation_symmetry_and_player_view() {
        let tuples = vec![vec![0]];
        let weights = vec![vec![0.0, 1.0, -1.0]];
        let bytes = build_weights_blob_v1(&tuples, &weights);
        let evaluator = NTupleEvaluator::from_bytes(&bytes).expect("must parse");

        let black = bit(0) | bit(7) | bit(56) | bit(63);
        let board = Board::from_bitboards(black, 0);

        let black_score = evaluator.evaluate(&board, true);
        let white_score = evaluator.evaluate(&board, false);

        assert_eq!(black_score, 1.0);
        assert_eq!(white_score, -1.0);
    }

    #[test]
    fn evaluate_v3_applies_dihedral_symmetry_and_player_view() {
        let tuples = vec![vec![0]];
        let phase_weights = vec![vec![vec![0.0, 1.0, -1.0]]];
        let bytes = build_weights_blob_v3(&tuples, &phase_weights, 1);
        let evaluator = NTupleEvaluator::from_bytes(&bytes).expect("must parse");

        let black = bit(0) | bit(7) | bit(56) | bit(63);
        let board = Board::from_bitboards(black, 0);

        let black_score = evaluator.evaluate(&board, true);
        let white_score = evaluator.evaluate(&board, false);

        assert_eq!(black_score, 1.0);
        assert_eq!(white_score, -1.0);
    }

    #[test]
    fn evaluate_v3_is_reflection_invariant_for_opening_positions() {
        let tuples = vec![vec![0, 1]];
        let phase_weights = vec![vec![vec![0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0]]];
        let bytes = build_weights_blob_v3(&tuples, &phase_weights, 1);
        let evaluator = NTupleEvaluator::from_bytes(&bytes).expect("must parse");

        let mut d3_board = Board::new();
        let mut c4_board = Board::new();
        assert_ne!(d3_board.place(19, true), 0);
        assert_ne!(c4_board.place(26, true), 0);

        assert_eq!(
            evaluator.evaluate(&d3_board, false),
            evaluator.evaluate(&c4_board, false)
        );
    }

    #[test]
    fn evaluate_selects_weights_for_current_phase() {
        let tuples = vec![vec![0]];
        let phase_weights = vec![vec![vec![0.0, 1.0, 0.0]], vec![vec![0.0, 2.0, 0.0]]];
        let bytes = build_weights_blob_v2(&tuples, &phase_weights, 2);
        let evaluator = NTupleEvaluator::from_bytes(&bytes).expect("must parse");

        let phase0_board = board_with_empty_count(60);
        let phase1_board = board_with_empty_count(58);

        assert_eq!(evaluator.evaluate(&phase0_board, true), 1.0);
        assert_eq!(evaluator.evaluate(&phase1_board, true), 2.0);
    }

    #[test]
    fn v1_weights_are_reused_for_all_later_phases() {
        let tuples = vec![vec![0]];
        let weights = vec![vec![0.0, 1.0, 0.0]];
        let bytes = build_weights_blob_v1(&tuples, &weights);
        let evaluator = NTupleEvaluator::from_bytes(&bytes).expect("must parse");
        let late_board = board_with_empty_count(0);

        assert_eq!(evaluator.evaluate(&late_board, true), 1.0);
    }
}
