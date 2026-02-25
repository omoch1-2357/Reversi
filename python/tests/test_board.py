from board import Board


def idx(row: int, col: int) -> int:
    return row * 8 + col


def bit(pos: int) -> int:
    return 1 << pos


def test_initial_black_legal_moves_are_four_expected_squares() -> None:
    board = Board()
    expected = bit(idx(2, 3)) | bit(idx(3, 2)) | bit(idx(4, 5)) | bit(idx(5, 4))
    assert board.legal_moves(True) == expected


def test_initial_white_legal_moves_are_four_expected_squares() -> None:
    board = Board()
    expected = bit(idx(2, 4)) | bit(idx(3, 5)) | bit(idx(4, 2)) | bit(idx(5, 3))
    assert board.legal_moves(False) == expected


def test_place_flips_and_updates_counts_and_empty() -> None:
    board = Board()
    flips = board.place(idx(2, 3), True)

    assert flips == bit(idx(3, 3))
    assert board.count() == (4, 1)
    assert board.empty_count() == 59


def test_illegal_place_returns_zero_and_keeps_state() -> None:
    board = Board()
    before_black = board.black
    before_white = board.white

    flips = board.place(idx(0, 0), True)

    assert flips == 0
    assert board.black == before_black
    assert board.white == before_white


def test_out_of_range_place_returns_zero_and_keeps_state() -> None:
    board = Board()
    before_black = board.black
    before_white = board.white

    assert board.place(-1, False) == 0
    assert board.place(64, False) == 0
    assert board.black == before_black
    assert board.white == before_white


def test_to_array_is_current_player_perspective() -> None:
    board = Board()
    board.place(idx(2, 3), True)

    black_view = board.to_array(True)
    white_view = board.to_array(False)

    assert black_view[idx(2, 3)] == 1
    assert black_view[idx(4, 4)] == 2
    assert white_view[idx(2, 3)] == 2
    assert white_view[idx(4, 4)] == 1


def test_copy_returns_deep_copy() -> None:
    board = Board()
    cloned = board.copy()

    assert cloned is not board
    assert cloned.black == board.black
    assert cloned.white == board.white

    cloned.place(idx(2, 3), True)
    assert (cloned.black, cloned.white) != (board.black, board.white)
