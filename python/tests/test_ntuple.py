import numpy as np
import pytest

from board import Board
from ntuple import NTupleNetwork


def test_weights_are_initialized_from_tuple_patterns() -> None:
    ntuple = NTupleNetwork()

    assert len(ntuple.weights) == len(ntuple.TUPLE_PATTERNS)
    for pattern, weights in zip(ntuple.TUPLE_PATTERNS, ntuple.weights, strict=True):
        assert weights.shape == (3 ** len(pattern),)
        assert weights.dtype == np.float32
        assert np.count_nonzero(weights) == 0


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


def test_update_applies_delta_to_each_symmetry_index() -> None:
    ntuple = NTupleNetwork()
    board = Board()
    pattern = ntuple.TUPLE_PATTERNS[0]
    delta = 0.25

    indices = [
        ntuple._pattern_index(sym, pattern)
        for sym in ntuple._symmetries(board.to_array(False))
    ]
    counts: dict[int, int] = {}
    for idx in indices:
        counts[idx] = counts.get(idx, 0) + 1

    ntuple.update(board, False, delta)

    for idx, count in counts.items():
        assert ntuple.weights[0][idx] == pytest.approx(delta * count)


def test_evaluate_uses_current_player_perspective() -> None:
    ntuple = NTupleNetwork()
    board = Board()

    ntuple.TUPLE_PATTERNS = [[27, 28, 35, 36]]
    ntuple.weights = [np.zeros(3**4, dtype=np.float32)]
    ntuple._symmetries = lambda board_array: [board_array]  # type: ignore[method-assign]

    pattern = ntuple.TUPLE_PATTERNS[0]
    black_index = ntuple._pattern_index(board.to_array(True), pattern)
    white_index = ntuple._pattern_index(board.to_array(False), pattern)
    assert black_index != white_index

    ntuple.weights[0][black_index] = np.float32(2.5)
    ntuple.weights[0][white_index] = np.float32(-1.5)

    assert ntuple.evaluate(board, True) == pytest.approx(2.5)
    assert ntuple.evaluate(board, False) == pytest.approx(-1.5)
