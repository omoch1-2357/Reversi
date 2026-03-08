from types import SimpleNamespace

import pytest

import rust_training


def test_train_to_bytes_raises_clear_error_when_extension_is_missing(
    monkeypatch,
) -> None:
    def _raise(_name: str):
        raise ImportError("missing extension")

    monkeypatch.setattr(rust_training, "import_module", _raise)

    with pytest.raises(RuntimeError, match="Rust training extension is not installed"):
        rust_training.train_to_bytes(
            games=0,
            alpha=0.01,
            lambda_=0.7,
            epsilon=0.1,
            seed=42,
            progress_interval=0,
        )


def test_train_to_bytes_delegates_to_extension(monkeypatch) -> None:
    captured: dict[str, object] = {}
    callback_calls: list[tuple[int, int, float]] = []

    def _train_to_bytes(**kwargs):
        captured.update(kwargs)
        kwargs["progress_callback"](1, kwargs["games"], 0.25)
        return b"model-bytes"

    monkeypatch.setattr(
        rust_training,
        "import_module",
        lambda _name: SimpleNamespace(train_to_bytes=_train_to_bytes),
    )

    def progress_callback(completed: int, total: int, elapsed: float) -> None:
        callback_calls.append((completed, total, elapsed))

    payload = rust_training.train_to_bytes(
        games=3,
        alpha=0.2,
        lambda_=0.8,
        epsilon=0.05,
        seed=99,
        progress_interval=2,
        progress_callback=progress_callback,
    )

    assert payload == b"model-bytes"
    assert captured["games"] == 3
    assert captured["alpha"] == pytest.approx(0.2)
    assert captured["lambda_"] == pytest.approx(0.8)
    assert captured["epsilon"] == pytest.approx(0.05)
    assert captured["seed"] == 99
    assert captured["progress_interval"] == 2
    assert captured["progress_callback"] is progress_callback
    assert callback_calls == [(1, 3, 0.25)]
