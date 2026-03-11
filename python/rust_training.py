"""Thin Python wrapper around the Rust-backed training extension."""

from __future__ import annotations

from collections.abc import Callable
from importlib.machinery import ExtensionFileLoader
from importlib import import_module
from importlib.util import module_from_spec, spec_from_file_location
from pathlib import Path
import sys
from types import ModuleType

ProgressCallback = Callable[[int, int, float], None]

_MODULE_NAME = "_reversi_training"
_MODULE_DIR = Path(__file__).resolve().parent
_IMPORT_ERROR_MESSAGE = (
    "Rust training extension is not installed. "
    "Build it with `maturin build --manifest-path python/rust_training_ext/Cargo.toml "
    "--out python/dist --interpreter python` and install the wheel before running training."
)


def _candidate_extension_paths() -> tuple[Path, ...]:
    suffixes = (".pyd", ".dll")
    base_dirs = (
        _MODULE_DIR / "rust_training_ext" / "target" / "release",
        _MODULE_DIR / "rust_training_ext" / "target" / "release" / "maturin",
    )
    return tuple(
        base_dir / f"{_MODULE_NAME}{suffix}"
        for base_dir in base_dirs
        for suffix in suffixes
    )


def _load_extension_from_path(path: Path) -> ModuleType:
    loader = ExtensionFileLoader(_MODULE_NAME, str(path))
    spec = spec_from_file_location(_MODULE_NAME, str(path), loader=loader)
    if spec is None or spec.loader is None:
        raise ImportError(f"Could not load extension spec from {path}")

    previous = sys.modules.pop(_MODULE_NAME, None)
    try:
        module = module_from_spec(spec)
        sys.modules[_MODULE_NAME] = module
        spec.loader.exec_module(module)
        return module
    except Exception:
        if previous is not None:
            sys.modules[_MODULE_NAME] = previous
        else:
            sys.modules.pop(_MODULE_NAME, None)
        raise


def _load_extension() -> ModuleType:
    for candidate in _candidate_extension_paths():
        if candidate.exists():
            try:
                return _load_extension_from_path(candidate)
            except ImportError:
                continue
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
    threads: int,
    initial_model: bytes | None,
    random_opening_plies: int,
    progress_interval: int,
    progress_callback: ProgressCallback | None = None,
) -> bytes:
    module = _load_extension()
    kwargs = dict(
        games=games,
        alpha=alpha,
        lambda_=lambda_,
        epsilon=epsilon,
        seed=seed,
        threads=threads,
        progress_interval=progress_interval,
        progress_callback=progress_callback,
    )
    if initial_model is not None:
        kwargs["initial_model"] = bytes(initial_model)
    if random_opening_plies != 0:
        kwargs["random_opening_plies"] = random_opening_plies
    try:
        return bytes(module.train_to_bytes(**kwargs))
    except TypeError as exc:
        if "threads" not in str(exc):
            raise
        if threads != 1:
            raise RuntimeError(
                "Installed Rust training extension does not support `threads`. "
                "Rebuild and reinstall the extension before using parallel training."
            ) from exc

        kwargs.pop("threads")
        return bytes(module.train_to_bytes(**kwargs))


def compress_model_bytes(data: bytes) -> bytes:
    module = _load_extension()
    return bytes(module.compress_model_bytes(bytes(data)))


def decompress_model_bytes(data: bytes) -> bytes:
    module = _load_extension()
    return bytes(module.decompress_model_bytes(bytes(data)))
