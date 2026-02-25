"""N-tuple network evaluator for the training pipeline."""

from __future__ import annotations

import numpy as np

from board import BOARD_SIZE, NUM_SQUARES, Board


class NTupleNetwork:
    """N-Tuple Network evaluation model."""

    # Fixed tuple positions over a row-major flattened 8x8 board.
    TUPLE_PATTERNS: list[list[int]] = [
        [0, 1, 8, 9, 10, 17, 18, 19, 26, 27],
        [0, 1, 8, 9, 18, 27, 36, 45, 54, 63],
        [0, 1, 2, 3, 8, 9, 10, 16, 17, 24],
        [0, 1, 2, 3, 4, 8, 9, 16, 24, 32],
        [0, 1, 2, 3, 4, 5, 6, 7, 9, 14],
        [0, 2, 3, 4, 5, 7, 10, 11, 12, 13],
        [1, 2, 3, 4, 5, 6, 10, 11, 12, 13],
        [0, 1, 2, 8, 9, 10, 16, 17, 18],
        [0, 1, 10, 19, 28, 37, 46, 55, 63],
        [8, 9, 10, 11, 12, 13, 14, 15],
        [16, 17, 18, 19, 20, 21, 22, 23],
        [24, 25, 26, 27, 28, 29, 30, 31],
        [1, 2, 11, 20, 29, 38, 47, 55],
        [3, 9, 12, 21, 30, 39, 54],
    ]

    def __init__(self) -> None:
        """Initialize all tuple weights to zero."""
        self.weights: list[np.ndarray] = [
            np.zeros(3 ** len(pattern), dtype=np.float32)
            for pattern in self.TUPLE_PATTERNS
        ]

    def evaluate(self, board: Board, is_black: bool) -> float:
        """Evaluate a position from the current player's perspective."""
        score = 0.0
        board_array = board.to_array(is_black)
        for sym in self._symmetries(board_array):
            for i, pattern in enumerate(self.TUPLE_PATTERNS):
                index = self._pattern_index(sym, pattern)
                score += float(self.weights[i][index])
        return score

    def update(self, board: Board, is_black: bool, delta: float) -> None:
        """Apply a pre-scaled update amount to all matching tuple weights."""
        board_array = board.to_array(is_black)
        delta32 = np.float32(delta)
        for sym in self._symmetries(board_array):
            for i, pattern in enumerate(self.TUPLE_PATTERNS):
                index = self._pattern_index(sym, pattern)
                self.weights[i][index] += delta32

    @staticmethod
    def _pattern_index(board_array: np.ndarray, pattern: list[int]) -> int:
        """Convert tuple cell states into a base-3 table index."""
        index = 0
        for pos in pattern:
            index = index * 3 + int(board_array[pos])
        return index

    @staticmethod
    def _symmetries(board_array: np.ndarray) -> list[np.ndarray]:
        """Return rotational symmetries (0/90/180/270 degrees)."""
        if board_array.size != NUM_SQUARES:
            raise ValueError(
                f"board_array size must be {NUM_SQUARES}, got {board_array.size}"
            )

        board_grid = board_array.reshape(BOARD_SIZE, BOARD_SIZE)
        transformed: list[np.ndarray] = []
        for turns in range(4):
            transformed.append(np.rot90(board_grid, turns).flatten())
        return transformed
