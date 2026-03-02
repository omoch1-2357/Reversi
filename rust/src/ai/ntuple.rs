use crate::board::Board;

const MAGIC: &[u8; 4] = b"NTRV";
const VERSION: u32 = 1;
const HEADER_SIZE: usize = 20;
const BOARD_SIZE: usize = 8;
const BOARD_CELLS: usize = BOARD_SIZE * BOARD_SIZE;

/// Inference-time N-Tuple evaluator loaded from `weights.bin`.
#[derive(Debug, Clone)]
pub struct NTupleEvaluator {
    tuples: Vec<Vec<u8>>,
    weights: Vec<Vec<f32>>,
}

impl NTupleEvaluator {
    /// Deserialize evaluator data from `weights.bin` format.
    pub fn from_bytes(data: &[u8]) -> Result<Self, String> {
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
                "unsupported weights version: expected {VERSION}, got {version}"
            ));
        }

        let num_tuples = read_u32_le(data, 8)? as usize;
        let expected_crc = read_u32_le(data, 12)?;
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

        let mut weights = Vec::with_capacity(num_tuples);
        for (tuple_idx, tuple) in tuples.iter().enumerate() {
            let entries = pow3(tuple.len())?;
            let bytes_len = entries
                .checked_mul(4)
                .ok_or_else(|| "weights byte length overflow".to_string())?;

            if offset + bytes_len > payload.len() {
                return Err(format!(
                    "unexpected EOF while reading weights for tuple #{tuple_idx}"
                ));
            }

            let mut tuple_weights = Vec::with_capacity(entries);
            for i in 0..entries {
                let start = offset + i * 4;
                let mut chunk = [0u8; 4];
                chunk.copy_from_slice(&payload[start..start + 4]);
                tuple_weights.push(f32::from_le_bytes(chunk));
            }

            offset += bytes_len;
            weights.push(tuple_weights);
        }

        if offset != payload.len() {
            return Err("weights payload has trailing bytes".to_string());
        }

        Ok(Self { tuples, weights })
    }

    /// Evaluate from the side-to-move perspective.
    pub fn evaluate(&self, board: &Board, is_black: bool) -> f32 {
        let cells = board.to_array();
        let mut score = 0.0f32;

        for rotation in 0..4u8 {
            for (tuple, weights) in self.tuples.iter().zip(self.weights.iter()) {
                let idx = tuple.iter().fold(0usize, |acc, &pos| {
                    let rotated = rotate_pos(pos, rotation);
                    let value = map_to_player_view(cells[rotated], is_black) as usize;
                    acc * 3 + value
                });
                score += weights[idx];
            }
        }

        score
    }
}

fn rotate_pos(pos: u8, rotation: u8) -> usize {
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

    fn build_weights_blob(tuples: &[Vec<u8>], weights: &[Vec<f32>]) -> Vec<u8> {
        assert_eq!(tuples.len(), weights.len());

        let mut payload = Vec::new();
        for tuple in tuples {
            payload.push(tuple.len() as u8);
            payload.extend_from_slice(tuple);
        }
        for w in weights {
            for value in w {
                payload.extend_from_slice(&value.to_le_bytes());
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
        out
    }

    fn bit(pos: usize) -> u64 {
        1u64 << pos
    }

    #[test]
    fn from_bytes_deserializes_tuple_defs_and_weights() {
        let tuples = vec![vec![0, 1], vec![63]];
        let weights = vec![
            vec![0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0],
            vec![-1.0, 2.0, 0.25],
        ];
        let bytes = build_weights_blob(&tuples, &weights);

        let evaluator = NTupleEvaluator::from_bytes(&bytes).expect("must parse");

        assert_eq!(evaluator.tuples, tuples);
        assert_eq!(evaluator.weights, weights);
    }

    #[test]
    fn from_bytes_rejects_invalid_magic() {
        let tuples = vec![vec![0]];
        let weights = vec![vec![0.0, 1.0, -1.0]];
        let mut bytes = build_weights_blob(&tuples, &weights);
        bytes[0] = b'X';

        let err = NTupleEvaluator::from_bytes(&bytes).unwrap_err();
        assert!(err.contains("magic"));
    }

    #[test]
    fn from_bytes_rejects_unsupported_version() {
        let tuples = vec![vec![0]];
        let weights = vec![vec![0.0, 1.0, -1.0]];
        let mut bytes = build_weights_blob(&tuples, &weights);
        bytes[4..8].copy_from_slice(&2u32.to_le_bytes());

        let err = NTupleEvaluator::from_bytes(&bytes).unwrap_err();
        assert!(err.contains("version"));
    }

    #[test]
    fn from_bytes_rejects_crc_mismatch() {
        let tuples = vec![vec![0]];
        let weights = vec![vec![0.0, 1.0, -1.0]];
        let mut bytes = build_weights_blob(&tuples, &weights);
        let last = bytes.len() - 1;
        bytes[last] ^= 0x01;

        let err = NTupleEvaluator::from_bytes(&bytes).unwrap_err();
        assert!(err.contains("CRC32"));
    }

    #[test]
    fn from_bytes_rejects_truncated_weights_payload() {
        let tuples = vec![vec![0, 1]];
        let weights = vec![vec![0.0; 9]];
        let mut bytes = build_weights_blob(&tuples, &weights);
        bytes.pop();
        let recalculated_crc = crc32fast::hash(&bytes[HEADER_SIZE..]);
        bytes[12..16].copy_from_slice(&recalculated_crc.to_le_bytes());

        let err = NTupleEvaluator::from_bytes(&bytes).unwrap_err();
        assert!(err.contains("unexpected EOF while reading weights"));
    }

    #[test]
    fn evaluate_applies_rotation_symmetry_and_player_view() {
        let tuples = vec![vec![0]];
        let weights = vec![vec![0.0, 1.0, -1.0]];
        let bytes = build_weights_blob(&tuples, &weights);
        let evaluator = NTupleEvaluator::from_bytes(&bytes).expect("must parse");

        let black = bit(0) | bit(7) | bit(56) | bit(63);
        let board = Board::from_bitboards(black, 0);

        let black_score = evaluator.evaluate(&board, true);
        let white_score = evaluator.evaluate(&board, false);

        assert_eq!(black_score, 4.0);
        assert_eq!(white_score, -4.0);
    }
}
