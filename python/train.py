"""CLI entry point for TD-Lambda training and model export."""

from __future__ import annotations

import argparse
import json
import math
from pathlib import Path
import struct
import sys
from time import perf_counter
from typing import Sequence
import zlib

from export_model import HEADER_SIZE, MAGIC, VERSION
from ntuple import NTupleNetwork
from rust_training import decompress_model_bytes, train_to_bytes


def build_parser() -> argparse.ArgumentParser:
    """Build the command-line interface for model training."""
    parser = argparse.ArgumentParser(description="Train Reversi NTuple weights.")
    parser.add_argument(
        "--games", type=int, default=500_000, help="Number of self-play games."
    )
    parser.add_argument("--alpha", type=float, default=0.001, help="Learning rate.")
    parser.add_argument(
        "--lambda",
        type=float,
        default=0.7,
        dest="lambda_",
        help="Eligibility trace decay.",
    )
    parser.add_argument(
        "--epsilon", type=float, default=0.1, help="Exploration rate for self-play."
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=Path("weights.bin"),
        help="Output model path.",
    )
    parser.add_argument(
        "--seed",
        type=int,
        default=42,
        help="Random seed for reproducible training runs.",
    )
    parser.add_argument(
        "--threads",
        type=int,
        default=0,
        help="Training worker threads (0 uses the maximum available CPU count).",
    )
    parser.add_argument(
        "--progress-interval",
        type=int,
        default=10_000,
        help="Emit progress every N games (0 disables periodic logs).",
    )
    parser.add_argument(
        "--random-opening-plies",
        type=int,
        default=0,
        help="Apply N random plies before learned self-play begins.",
    )
    parser.add_argument(
        "--checkpoint-interval",
        type=int,
        default=0,
        help="Save a checkpoint every N newly trained games (0 disables checkpoints).",
    )
    parser.add_argument(
        "--checkpoint-dir",
        type=Path,
        default=None,
        help="Directory used for checkpoint files. Defaults next to the output path.",
    )
    parser.add_argument(
        "--resume-from",
        type=Path,
        default=None,
        help="Optional existing weights.bin to continue training from.",
    )
    parser.add_argument(
        "--status-file",
        type=Path,
        default=None,
        help="Optional JSON file updated with the latest training status.",
    )
    parser.add_argument(
        "--verify",
        action=argparse.BooleanOptionalAction,
        default=True,
        help="Validate exported model structure and weights after writing.",
    )
    return parser


def log_training_progress(completed: int, total: int, elapsed_seconds: float) -> None:
    """Print a concise periodic progress update for long training runs."""
    progress_percent = 100.0 if total == 0 else (completed / total) * 100.0
    games_per_second = 0.0 if elapsed_seconds <= 0.0 else completed / elapsed_seconds
    print(
        f"[progress] {completed}/{total} games "
        f"({progress_percent:.1f}%) elapsed={elapsed_seconds:.1f}s "
        f"rate={games_per_second:.2f} games/s"
    )


def write_status_file(
    path: Path | None,
    *,
    state: str,
    completed_games: int,
    total_games: int,
    elapsed_seconds: float,
    output_path: Path,
    seed: int,
    threads: int,
    progress_interval: int,
    checkpoint_interval: int,
    random_opening_plies: int,
    resume_from: Path | None,
    last_checkpoint: Path | None = None,
    error: str | None = None,
) -> None:
    """Persist the latest training status in an easy-to-tail JSON file."""
    if path is None:
        return

    progress_percent = (
        100.0 if total_games == 0 else (completed_games / total_games) * 100.0
    )
    games_per_second = (
        0.0 if elapsed_seconds <= 0.0 else completed_games / elapsed_seconds
    )
    status = {
        "state": state,
        "completed_games": completed_games,
        "total_games": total_games,
        "progress_percent": round(progress_percent, 3),
        "elapsed_seconds": round(elapsed_seconds, 3),
        "games_per_second": round(games_per_second, 3),
        "output_path": str(output_path),
        "seed": seed,
        "threads": threads,
        "progress_interval": progress_interval,
        "checkpoint_interval": checkpoint_interval,
        "random_opening_plies": random_opening_plies,
        "resume_from": str(resume_from) if resume_from is not None else None,
        "last_checkpoint": str(last_checkpoint)
        if last_checkpoint is not None
        else None,
        "error": error,
    }

    path.parent.mkdir(parents=True, exist_ok=True)
    temp_path = path.with_suffix(path.suffix + ".tmp")
    temp_path.write_text(json.dumps(status, indent=2, sort_keys=True) + "\n")
    temp_path.replace(path)


def verify_exported_model(path: Path, tuple_patterns: Sequence[Sequence[int]]) -> None:
    """Validate exported bytes for header, CRC32, and deserializable layout."""
    payload = decompress_model_bytes(path.read_bytes())
    if len(payload) < HEADER_SIZE:
        raise ValueError(
            f"model payload too short: expected at least {HEADER_SIZE} bytes, got {len(payload)}"
        )

    magic, version, num_tuples, expected_crc32, phase_count = struct.unpack(
        "<4sIIII", payload[:HEADER_SIZE]
    )
    if magic != MAGIC:
        raise ValueError(f"invalid magic: expected {MAGIC!r}, got {magic!r}")
    if version != VERSION:
        raise ValueError(f"unsupported version: expected {VERSION}, got {version}")
    if num_tuples != len(tuple_patterns):
        raise ValueError(
            f"num_tuples mismatch: expected {len(tuple_patterns)}, got {num_tuples}"
        )
    if phase_count != NTupleNetwork.PHASE_COUNT:
        raise ValueError(
            f"phase_count mismatch: expected {NTupleNetwork.PHASE_COUNT}, got {phase_count}"
        )

    data = payload[HEADER_SIZE:]
    actual_crc32 = zlib.crc32(data) & 0xFFFFFFFF
    if actual_crc32 != expected_crc32:
        raise ValueError(
            f"CRC32 mismatch: expected {expected_crc32:#010x}, got {actual_crc32:#010x}"
        )

    offset = 0
    for idx, pattern in enumerate(tuple_patterns):
        if offset >= len(data):
            raise ValueError(f"missing tuple definition at index {idx}")

        tuple_size = data[offset]
        offset += 1
        if tuple_size != len(pattern):
            raise ValueError(
                f"tuple_size mismatch at index {idx}: "
                f"expected {len(pattern)}, got {tuple_size}"
            )

        end = offset + tuple_size
        if end > len(data):
            raise ValueError(f"tuple definition truncated at index {idx}")

        positions = list(data[offset:end])
        if positions != list(pattern):
            raise ValueError(
                f"tuple positions mismatch at index {idx}: "
                f"expected {list(pattern)}, got {positions}"
            )
        offset = end

    for phase_idx in range(phase_count):
        for tuple_idx, pattern in enumerate(tuple_patterns):
            weight_count = 3 ** len(pattern)
            required = weight_count * 4
            end = offset + required
            if end > len(data):
                raise ValueError(
                    f"weights truncated at phase {phase_idx}, tuple index {tuple_idx}"
                )

            for weight_offset in range(offset, end, 4):
                (value,) = struct.unpack_from("<f", data, weight_offset)
                if not math.isfinite(value):
                    raise ValueError(
                        f"non-finite weight at phase {phase_idx}, tuple index {tuple_idx}"
                    )
            offset = end

    if offset != len(data):
        raise ValueError(
            f"unexpected trailing bytes in model data: {len(data) - offset}"
        )


def checkpoint_path_for(
    output: Path, checkpoint_dir: Path, completed_games: int
) -> Path:
    """Build a stable checkpoint filename for the current training chunk."""
    suffix = "".join(output.suffixes) or ".bin"
    stem = output.name[: -len(suffix)] if output.name.endswith(suffix) else output.stem
    filename = f"{stem}.checkpoint-{completed_games:07d}{suffix}"
    return checkpoint_dir / filename


def train_and_export(
    games: int,
    alpha: float,
    lambda_: float,
    epsilon: float,
    output: Path,
    seed: int,
    threads: int,
    random_opening_plies: int,
    progress_interval: int,
    checkpoint_interval: int,
    checkpoint_dir: Path | None,
    resume_from: Path | None,
    status_file: Path | None,
    verify: bool,
) -> Path:
    """Run training, export the model, and validate the resulting binary."""
    if games < 0:
        raise ValueError(f"games must be >= 0, got {games}")
    if threads < 0:
        raise ValueError(f"threads must be >= 0, got {threads}")
    if random_opening_plies < 0:
        raise ValueError(
            f"random_opening_plies must be >= 0, got {random_opening_plies}"
        )
    if progress_interval < 0:
        raise ValueError(f"progress_interval must be >= 0, got {progress_interval}")
    if checkpoint_interval < 0:
        raise ValueError(f"checkpoint_interval must be >= 0, got {checkpoint_interval}")

    output_path = output
    output_path.parent.mkdir(parents=True, exist_ok=True)
    current_model = None
    started_at = perf_counter()
    last_checkpoint_path: Path | None = None

    def emit_status(
        completed_games: int,
        *,
        state: str,
        error: str | None = None,
    ) -> None:
        write_status_file(
            status_file,
            state=state,
            completed_games=completed_games,
            total_games=games,
            elapsed_seconds=perf_counter() - started_at,
            output_path=output_path,
            seed=seed,
            threads=threads,
            progress_interval=progress_interval,
            checkpoint_interval=checkpoint_interval,
            random_opening_plies=random_opening_plies,
            resume_from=resume_from,
            last_checkpoint=last_checkpoint_path,
            error=error,
        )

    if resume_from is not None:
        if verify:
            verify_exported_model(resume_from, NTupleNetwork.TUPLE_PATTERNS)
        current_model = resume_from.read_bytes()

    emit_status(0, state="starting")

    def on_progress(completed: int, total: int, elapsed_seconds: float) -> None:
        log_training_progress(completed, total, elapsed_seconds)
        emit_status(completed, state="running")

    if checkpoint_interval == 0 or games == 0:
        model_bytes = train_to_bytes(
            games=games,
            alpha=alpha,
            lambda_=lambda_,
            epsilon=epsilon,
            seed=seed,
            threads=threads,
            initial_model=current_model,
            random_opening_plies=random_opening_plies,
            progress_interval=progress_interval,
            progress_callback=on_progress,
        )
        output_path.write_bytes(model_bytes)
        if verify:
            verify_exported_model(output_path, NTupleNetwork.TUPLE_PATTERNS)
        emit_status(games, state="completed")
        return output_path

    resolved_checkpoint_dir = checkpoint_dir
    if resolved_checkpoint_dir is None:
        resolved_checkpoint_dir = output_path.parent / f"{output_path.stem}.checkpoints"
    resolved_checkpoint_dir.mkdir(parents=True, exist_ok=True)

    completed_games = 0
    while completed_games < games:
        chunk_games = min(checkpoint_interval, games - completed_games)

        def on_progress(done: int, _total: int, _elapsed: float) -> None:
            log_training_progress(
                completed_games + done,
                games,
                perf_counter() - started_at,
            )
            emit_status(completed_games + done, state="running")

        current_model = train_to_bytes(
            games=chunk_games,
            alpha=alpha,
            lambda_=lambda_,
            epsilon=epsilon,
            seed=seed + completed_games,
            threads=threads,
            initial_model=current_model,
            random_opening_plies=random_opening_plies,
            progress_interval=min(progress_interval, chunk_games)
            if progress_interval > 0
            else 0,
            progress_callback=on_progress,
        )
        completed_games += chunk_games

        checkpoint_path = checkpoint_path_for(
            output_path,
            resolved_checkpoint_dir,
            completed_games,
        )
        checkpoint_path.write_bytes(current_model)
        last_checkpoint_path = checkpoint_path
        if verify:
            verify_exported_model(checkpoint_path, NTupleNetwork.TUPLE_PATTERNS)
        emit_status(completed_games, state="running")

    output_path.write_bytes(current_model)
    if verify:
        verify_exported_model(output_path, NTupleNetwork.TUPLE_PATTERNS)
    emit_status(games, state="completed")
    return output_path


def main(argv: list[str] | None = None) -> int:
    """Execute CLI workflow for training and exporting weights.bin."""
    args = build_parser().parse_args(argv)
    try:
        print(
            "Training with "
            f"games={args.games}, alpha={args.alpha}, lambda={args.lambda_}, "
            f"epsilon={args.epsilon}, seed={args.seed}, threads={args.threads}, "
            f"progress_interval={args.progress_interval}, "
            f"random_opening_plies={args.random_opening_plies}, "
            f"checkpoint_interval={args.checkpoint_interval}, "
            f"resume_from={args.resume_from}, status_file={args.status_file}, "
            f"verify={args.verify}"
        )
        start_time = perf_counter()
        output_path = train_and_export(
            games=args.games,
            alpha=args.alpha,
            lambda_=args.lambda_,
            epsilon=args.epsilon,
            output=args.output,
            seed=args.seed,
            threads=args.threads,
            random_opening_plies=args.random_opening_plies,
            progress_interval=args.progress_interval,
            checkpoint_interval=args.checkpoint_interval,
            checkpoint_dir=args.checkpoint_dir,
            resume_from=args.resume_from,
            status_file=args.status_file,
            verify=args.verify,
        )
        print(
            f"Model exported{(' and verified' if args.verify else '')}: {output_path} "
            f"(elapsed={perf_counter() - start_time:.1f}s)"
        )
        return 0
    except Exception as exc:
        write_status_file(
            args.status_file,
            state="failed",
            completed_games=0,
            total_games=args.games,
            elapsed_seconds=0.0,
            output_path=args.output,
            seed=args.seed,
            threads=args.threads,
            progress_interval=args.progress_interval,
            checkpoint_interval=args.checkpoint_interval,
            random_opening_plies=args.random_opening_plies,
            resume_from=args.resume_from,
            error=str(exc),
        )
        print(f"Error: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
