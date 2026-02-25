"""Reversi bitboard implementation for the training pipeline."""

from __future__ import annotations

from dataclasses import dataclass

import numpy as np


BOARD_SIZE = 8
NUM_SQUARES = BOARD_SIZE * BOARD_SIZE
DIRECTIONS: tuple[tuple[int, int], ...] = (
    (-1, -1),
    (-1, 0),
    (-1, 1),
    (0, -1),
    (0, 1),
    (1, -1),
    (1, 0),
    (1, 1),
)


def _bit(pos: int) -> int:
    if 0 <= pos < NUM_SQUARES:
        return 1 << pos
    return 0


def _in_bounds(row: int, col: int) -> bool:
    return 0 <= row < BOARD_SIZE and 0 <= col < BOARD_SIZE


@dataclass
class Board:
    """Bitboard state aligned with the Rust implementation."""

    black: int = 0x0000000810000000  # d5, e4
    white: int = 0x0000001008000000  # d4, e5

    @staticmethod
    def _collect_flips(pos: int, me: int, opp: int) -> int:
        if pos < 0 or pos >= NUM_SQUARES:
            return 0

        move_bit = _bit(pos)
        if ((me | opp) & move_bit) != 0:
            return 0

        row, col = divmod(pos, BOARD_SIZE)
        flips = 0

        for dr, dc in DIRECTIONS:
            r = row + dr
            c = col + dc
            line = 0
            has_opponent = False

            while _in_bounds(r, c):
                square = _bit(r * BOARD_SIZE + c)
                if (opp & square) != 0:
                    has_opponent = True
                    line |= square
                elif (me & square) != 0:
                    if has_opponent:
                        flips |= line
                    break
                else:
                    break
                r += dr
                c += dc

        return flips

    def legal_moves(self, is_black: bool) -> int:
        me, opp = (self.black, self.white) if is_black else (self.white, self.black)
        occupied = me | opp
        legal = 0

        for pos in range(NUM_SQUARES):
            move_bit = _bit(pos)
            if (occupied & move_bit) != 0:
                continue
            if self._collect_flips(pos, me, opp) != 0:
                legal |= move_bit

        return legal

    def place(self, pos: int, is_black: bool) -> int:
        me, opp = (self.black, self.white) if is_black else (self.white, self.black)
        flips = self._collect_flips(pos, me, opp)
        if flips == 0:
            return 0

        move_bit = _bit(pos)
        next_me = me | move_bit | flips
        next_opp = opp & ~flips

        if is_black:
            self.black = next_me
            self.white = next_opp
        else:
            self.white = next_me
            self.black = next_opp

        return flips

    def count(self) -> tuple[int, int]:
        return self.black.bit_count(), self.white.bit_count()

    def empty_count(self) -> int:
        black_count, white_count = self.count()
        return NUM_SQUARES - black_count - white_count

    def to_array(self, is_black: bool) -> np.ndarray:
        arr = np.zeros(NUM_SQUARES, dtype=np.uint8)
        me, opp = (self.black, self.white) if is_black else (self.white, self.black)
        for pos in range(NUM_SQUARES):
            square = _bit(pos)
            if (me & square) != 0:
                arr[pos] = 1
            elif (opp & square) != 0:
                arr[pos] = 2
        return arr

    def copy(self) -> Board:
        return Board(black=self.black, white=self.white)
