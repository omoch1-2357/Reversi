"""TD-Lambda self-play trainer for the Reversi N-tuple model."""

from __future__ import annotations

import random

from board import Board
from ntuple import NTupleNetwork


class TDLambdaTrainer:
    """Train an N-tuple network by epsilon-greedy self-play."""

    def __init__(
        self,
        ntuple: NTupleNetwork,
        alpha: float = 0.01,
        lambda_: float = 0.7,
        epsilon: float = 0.1,
        seed: int | None = None,
    ) -> None:
        if alpha < 0.0:
            raise ValueError(f"alpha must be >= 0.0, got {alpha}")
        if not 0.0 <= lambda_ <= 1.0:
            raise ValueError(f"lambda_ must be in [0.0, 1.0], got {lambda_}")
        if not 0.0 <= epsilon <= 1.0:
            raise ValueError(f"epsilon must be in [0.0, 1.0], got {epsilon}")

        self.ntuple = ntuple
        self.alpha = alpha
        self.lambda_ = lambda_
        self.epsilon = epsilon
        self._rng = random.Random(seed)

    def train(self, num_games: int) -> None:
        """Run repeated self-play games and update weights after each game."""
        if num_games < 0:
            raise ValueError(f"num_games must be >= 0, got {num_games}")
        for _ in range(num_games):
            self._play_one_game()

    def _play_one_game(self) -> None:
        """Play one self-play game and apply TD-Lambda backward updates."""
        board = Board()
        is_black = True
        consecutive_passes = 0
        history: list[tuple[Board, bool]] = []

        while consecutive_passes < 2:
            legal = board.legal_moves(is_black)
            if legal == 0:
                consecutive_passes += 1
                is_black = not is_black
                continue

            consecutive_passes = 0
            move = self._select_move(board, is_black, legal)
            history.append((board.copy(), is_black))
            flipped = board.place(move, is_black)
            if flipped == 0:
                raise RuntimeError(f"selected illegal move: {move}")
            is_black = not is_black

        self._update_weights(history, board)

    def _select_move(self, board: Board, is_black: bool, legal: int) -> int:
        """Select one legal move by epsilon-greedy policy."""
        moves = self._legal_moves_from_mask(legal)
        if not moves:
            raise ValueError("legal move mask contains no moves")

        if self._rng.random() < self.epsilon:
            return self._rng.choice(moves)

        best_move = moves[0]
        best_score = float("-inf")
        for move in moves:
            next_board = board.copy()
            next_board.place(move, is_black)
            score = self.ntuple.evaluate(next_board, is_black)
            if score > best_score:
                best_score = score
                best_move = move
        return best_move

    def _update_weights(
        self, history: list[tuple[Board, bool]], final_board: Board
    ) -> None:
        """Apply TD-Lambda updates in reverse order after game termination."""
        if not history:
            return

        black_count, white_count = final_board.count()
        if black_count > white_count:
            reward = 1.0
        elif black_count < white_count:
            reward = -1.0
        else:
            reward = 0.0

        # Align terminal reward with the side to move in the last recorded state.
        next_value = reward if history[-1][1] else -reward
        cumulative_td = 0.0

        for board, is_black in reversed(history):
            current_value = self.ntuple.evaluate(board, is_black)
            td_error = next_value - current_value
            cumulative_td = td_error + self.lambda_ * cumulative_td
            delta = self.alpha * cumulative_td
            self.ntuple.update(board, is_black, delta)
            next_value = -current_value

    @staticmethod
    def _legal_moves_from_mask(mask: int) -> list[int]:
        """Decode a legal-move bitmask into ascending board indices."""
        moves: list[int] = []
        remaining = mask
        while remaining:
            lsb = remaining & -remaining
            moves.append(lsb.bit_length() - 1)
            remaining &= remaining - 1
        return moves
