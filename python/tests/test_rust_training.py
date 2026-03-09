from types import SimpleNamespace

import pytest

import rust_training


def test_train_to_bytes_raises_clear_error_when_extension_is_missing(
    monkeypatch,
) -> None:
    def _raise(_name: str):
        raise ModuleNotFoundError("missing extension", name="_reversi_training")

    monkeypatch.setattr(rust_training, "import_module", _raise)

    with pytest.raises(RuntimeError, match="Rust training extension is not installed"):
        rust_training.train_to_bytes(
            games=0,
            alpha=0.01,
            lambda_=0.7,
            epsilon=0.1,
            seed=42,
            threads=1,
            progress_interval=0,
        )


def test_train_to_bytes_propagates_internal_import_error(monkeypatch) -> None:
    def _raise(_name: str):
        raise ImportError("dependency ABI mismatch", name="numpy")

    monkeypatch.setattr(rust_training, "import_module", _raise)

    with pytest.raises(ImportError, match="dependency ABI mismatch"):
        rust_training.train_to_bytes(
            games=0,
            alpha=0.01,
            lambda_=0.7,
            epsilon=0.1,
            seed=42,
            threads=1,
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
        threads=0,
        progress_interval=2,
        progress_callback=progress_callback,
    )

    assert payload == b"model-bytes"
    assert captured["games"] == 3
    assert captured["alpha"] == pytest.approx(0.2)
    assert captured["lambda_"] == pytest.approx(0.8)
    assert captured["epsilon"] == pytest.approx(0.05)
    assert captured["seed"] == 99
    assert captured["threads"] == 0
    assert captured["progress_interval"] == 2
    assert captured["progress_callback"] is progress_callback
    assert callback_calls == [(1, 3, 0.25)]


def test_model_byte_helpers_delegate_to_extension(monkeypatch) -> None:
    captured: dict[str, bytes] = {}

    def _compress_model_bytes(data: bytes) -> bytes:
        captured["compress"] = data
        return b"compressed-model"

    def _decompress_model_bytes(data: bytes) -> bytes:
        captured["decompress"] = data
        return b"raw-model"

    monkeypatch.setattr(
        rust_training,
        "import_module",
        lambda _name: SimpleNamespace(
            compress_model_bytes=_compress_model_bytes,
            decompress_model_bytes=_decompress_model_bytes,
        ),
    )

    assert rust_training.compress_model_bytes(b"raw") == b"compressed-model"
    assert rust_training.decompress_model_bytes(b"compressed") == b"raw-model"
    assert captured["compress"] == b"raw"
    assert captured["decompress"] == b"compressed"


def test_train_to_bytes_falls_back_for_legacy_extension_when_threads_is_one(
    monkeypatch,
) -> None:
    captured: dict[str, object] = {}

    def _train_to_bytes(**kwargs):
        if "threads" in kwargs:
            raise TypeError(
                "train_to_bytes() got an unexpected keyword argument 'threads'"
            )
        captured.update(kwargs)
        return b"legacy-model"

    monkeypatch.setattr(
        rust_training,
        "import_module",
        lambda _name: SimpleNamespace(train_to_bytes=_train_to_bytes),
    )

    payload = rust_training.train_to_bytes(
        games=1,
        alpha=0.01,
        lambda_=0.7,
        epsilon=0.1,
        seed=42,
        threads=1,
        progress_interval=0,
    )

    assert payload == b"legacy-model"
    assert "threads" not in captured


def test_train_to_bytes_rejects_parallel_threads_with_legacy_extension(
    monkeypatch,
) -> None:
    def _train_to_bytes(**kwargs):
        raise TypeError("train_to_bytes() got an unexpected keyword argument 'threads'")

    monkeypatch.setattr(
        rust_training,
        "import_module",
        lambda _name: SimpleNamespace(train_to_bytes=_train_to_bytes),
    )

    with pytest.raises(RuntimeError, match="does not support `threads`"):
        rust_training.train_to_bytes(
            games=1,
            alpha=0.01,
            lambda_=0.7,
            epsilon=0.1,
            seed=42,
            threads=2,
            progress_interval=0,
        )
