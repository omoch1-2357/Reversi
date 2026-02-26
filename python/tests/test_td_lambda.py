import numpy as np
import pytest

from board import Board
from ntuple import NTupleNetwork
from td_lambda import TDLambdaTrainer


class RecordingNTuple:
    def __init__(self, value: float = 0.0) -> None:
        self.value = value
        self.updates: list[tuple[bool, float]] = []

    def evaluate(self, _board: Board, _is_black: bool) -> float:
        return self.value

    def update(self, _board: Board, is_black: bool, delta: float) -> None:
        self.updates.append((is_black, delta))


def test_update_direction_is_toward_td_target() -> None:
    ntuple = RecordingNTuple(value=0.0)
    trainer = TDLambdaTrainer(ntuple, alpha=0.5, lambda_=0.0, epsilon=0.0, seed=7)
    history = [(Board(), True)]
    final_board = Board(black=(1 << 64) - 1, white=0)

    trainer._update_weights(history, final_board)

    assert len(ntuple.updates) == 1
    is_black, delta = ntuple.updates[0]
    assert is_black is True
    assert delta == pytest.approx(0.5)
    assert delta > 0.0


@pytest.mark.parametrize(
    ("is_black", "expected_delta"),
    [
        (True, 1.0),
        (False, -1.0),
    ],
)
def test_terminal_reward_is_reflected_per_player_perspective(
    is_black: bool, expected_delta: float
) -> None:
    ntuple = RecordingNTuple(value=0.0)
    trainer = TDLambdaTrainer(ntuple, alpha=1.0, lambda_=0.0, epsilon=0.0, seed=11)
    history = [(Board(), is_black)]
    final_board = Board(black=(1 << 64) - 1, white=0)

    trainer._update_weights(history, final_board)

    _, delta = ntuple.updates[0]
    assert delta == pytest.approx(expected_delta)


def test_update_weights_uses_lambda_return_across_multiple_steps() -> None:
    ntuple = RecordingNTuple(value=0.0)
    trainer = TDLambdaTrainer(ntuple, alpha=1.0, lambda_=0.5, epsilon=0.0, seed=13)
    history = [(Board(), True), (Board(), False)]
    final_board = Board(black=(1 << 64) - 1, white=0)

    trainer._update_weights(history, final_board)

    assert len(ntuple.updates) == 2
    # Reverse traversal: last state (white-to-move), then first state (black-to-move).
    assert ntuple.updates[0][0] is False
    assert ntuple.updates[0][1] == pytest.approx(-1.0)
    assert ntuple.updates[1][0] is True
    assert ntuple.updates[1][1] == pytest.approx(0.5)
    assert ntuple.updates[0][1] < 0.0
    assert ntuple.updates[1][1] > 0.0


def test_play_one_game_is_reproducible_with_fixed_seed() -> None:
    ntuple_a = NTupleNetwork()
    ntuple_b = NTupleNetwork()

    trainer_a = TDLambdaTrainer(
        ntuple=ntuple_a,
        alpha=0.01,
        lambda_=0.7,
        epsilon=0.3,
        seed=2026,
    )
    trainer_b = TDLambdaTrainer(
        ntuple=ntuple_b,
        alpha=0.01,
        lambda_=0.7,
        epsilon=0.3,
        seed=2026,
    )

    trainer_a._play_one_game()
    trainer_b._play_one_game()

    assert all(
        np.array_equal(a, b)
        for a, b in zip(ntuple_a.weights, ntuple_b.weights, strict=True)
    )
    assert any(np.count_nonzero(w) > 0 for w in ntuple_a.weights)
