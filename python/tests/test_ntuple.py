from unittest.mock import patch

import numpy as np
import pytest

from board import NUM_SQUARES, Board
from ntuple import NTupleNetwork


def test_weights_are_initialized_from_tuple_patterns() -> None:
    ntuple = NTupleNetwork()

    assert len(ntuple.weights) == len(ntuple.TUPLE_PATTERNS)
    for pattern, weights in zip(ntuple.TUPLE_PATTERNS, ntuple.weights, strict=True):
        assert weights.shape == (3 ** len(pattern),)
        assert weights.dtype == np.float32
        assert np.count_nonzero(weights) == 0


def test_tuple_patterns_have_expected_count_and_index_range() -> None:
    assert len(NTupleNetwork.TUPLE_PATTERNS) == 14
    for pattern in NTupleNetwork.TUPLE_PATTERNS:
        assert all(0 <= pos < NUM_SQUARES for pos in pattern)


def test_pattern_index_is_base3_encoding() -> None:
    arr = np.zeros(64, dtype=np.uint8)
    arr[0] = 2
    arr[1] = 1
    arr[2] = 0
    arr[3] = 2

    index = NTupleNetwork._pattern_index(arr, [0, 1, 2, 3])
    assert index == 65


def test_symmetries_returns_four_unique_rotations_for_asymmetric_array() -> None:
    arr = np.arange(64, dtype=np.uint8)
    symmetries = NTupleNetwork._symmetries(arr)

    assert len(symmetries) == 4
    assert np.array_equal(symmetries[0], arr)
    assert all(sym.shape == (64,) for sym in symmetries)
    assert len({tuple(sym.tolist()) for sym in symmetries}) == 4


def test_symmetries_second_entry_matches_clockwise_90_rotation() -> None:
    arr = np.arange(NUM_SQUARES, dtype=np.uint8)
    symmetries = NTupleNetwork._symmetries(arr)
    rotated = symmetries[1]

    expected = np.empty(NUM_SQUARES, dtype=np.uint8)
    for row in range(8):
        for col in range(8):
            src = row * 8 + col
            dst = col * 8 + (7 - row)
            expected[dst] = arr[src]

    assert np.array_equal(rotated, expected)


def test_symmetries_raises_value_error_for_invalid_size() -> None:
    invalid = np.arange(NUM_SQUARES - 1, dtype=np.uint8)

    with pytest.raises(ValueError):
        NTupleNetwork._symmetries(invalid)


def test_evaluate_sums_weights_of_matching_symmetry_indices() -> None:
    ntuple = NTupleNetwork()
    board = Board()
    pattern = ntuple.TUPLE_PATTERNS[0]

    indices = [
        ntuple._pattern_index(sym, pattern)
        for sym in ntuple._symmetries(board.to_array(True))
    ]
    for idx in indices:
        ntuple.weights[0][idx] += np.float32(1.0)

    expected = float(sum(ntuple.weights[0][idx] for idx in indices))
    assert ntuple.evaluate(board, True) == pytest.approx(expected)


def test_update_applies_delta_to_each_symmetry_index_for_all_patterns() -> None:
    ntuple = NTupleNetwork()
    board = Board()
    delta = 0.25

    symmetries = ntuple._symmetries(board.to_array(False))
    expected_counts: list[dict[int, int]] = []
    for pattern in ntuple.TUPLE_PATTERNS:
        counts: dict[int, int] = {}
        for sym in symmetries:
            idx = ntuple._pattern_index(sym, pattern)
            counts[idx] = counts.get(idx, 0) + 1
        expected_counts.append(counts)

    ntuple.update(board, False, delta)

    for pattern_idx, counts in enumerate(expected_counts):
        for idx, count in counts.items():
            assert ntuple.weights[pattern_idx][idx] == pytest.approx(delta * count)


def test_evaluate_uses_current_player_perspective() -> None:
    ntuple = NTupleNetwork()
    board = Board()

    ntuple.TUPLE_PATTERNS = [[27, 28, 35, 36]]
    ntuple.weights = [np.zeros(3**4, dtype=np.float32)]

    def one_symmetry(_self: NTupleNetwork, board_array: np.ndarray) -> list[np.ndarray]:
        return [board_array]

    pattern = ntuple.TUPLE_PATTERNS[0]
    black_index = ntuple._pattern_index(board.to_array(True), pattern)
    white_index = ntuple._pattern_index(board.to_array(False), pattern)
    assert black_index != white_index

    ntuple.weights[0][black_index] = np.float32(2.5)
    ntuple.weights[0][white_index] = np.float32(-1.5)

    with patch.object(NTupleNetwork, "_symmetries", one_symmetry):
        assert ntuple.evaluate(board, True) == pytest.approx(2.5)
        assert ntuple.evaluate(board, False) == pytest.approx(-1.5)
