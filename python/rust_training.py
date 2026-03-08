"""Thin Python wrapper around the Rust-backed training extension."""

from __future__ import annotations

from collections.abc import Callable
from importlib import import_module
from types import ModuleType

ProgressCallback = Callable[[int, int, float], None]

_MODULE_NAME = "_reversi_training"
_IMPORT_ERROR_MESSAGE = (
    "Rust training extension is not installed. "
    "Build it with `maturin build --manifest-path python/rust_training_ext/Cargo.toml "
    "--out python/dist --interpreter python` and install the wheel before running training."
)


def _load_extension() -> ModuleType:
    try:
        return import_module(_MODULE_NAME)
    except ImportError as exc:  # pragma: no cover - exercised via monkeypatch
        if getattr(exc, "name", None) == _MODULE_NAME:
            raise RuntimeError(_IMPORT_ERROR_MESSAGE) from exc
        raise


def train_to_bytes(
    games: int,
    alpha: float,
    lambda_: float,
    epsilon: float,
    seed: int,
    progress_interval: int,
    progress_callback: ProgressCallback | None = None,
) -> bytes:
    module = _load_extension()
    return bytes(
        module.train_to_bytes(
            games=games,
            alpha=alpha,
            lambda_=lambda_,
            epsilon=epsilon,
            seed=seed,
            progress_interval=progress_interval,
            progress_callback=progress_callback,
        )
    )


def compress_model_bytes(data: bytes) -> bytes:
    module = _load_extension()
    return bytes(module.compress_model_bytes(bytes(data)))


def decompress_model_bytes(data: bytes) -> bytes:
    module = _load_extension()
    return bytes(module.decompress_model_bytes(bytes(data)))
