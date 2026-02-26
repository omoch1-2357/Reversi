"""CLI entry point for TD-Lambda training and model export."""

from __future__ import annotations

import argparse
from pathlib import Path
import struct
import sys
from typing import Sequence
import zlib

from export_model import HEADER_SIZE, MAGIC, VERSION, export_model
from ntuple import NTupleNetwork
from td_lambda import TDLambdaTrainer


def build_parser() -> argparse.ArgumentParser:
    """Build the command-line interface for model training."""
    parser = argparse.ArgumentParser(description="Train Reversi NTuple weights.")
    parser.add_argument(
        "--games", type=int, default=500_000, help="Number of self-play games."
    )
    parser.add_argument("--alpha", type=float, default=0.01, help="Learning rate.")
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
    return parser


def verify_exported_model(path: Path, tuple_patterns: Sequence[Sequence[int]]) -> None:
    """Validate exported bytes for header, CRC32, and deserializable layout."""
    payload = path.read_bytes()
    if len(payload) < HEADER_SIZE:
        raise ValueError(
            f"model payload too short: expected at least {HEADER_SIZE} bytes, got {len(payload)}"
        )

    magic, version, num_tuples, expected_crc32, reserved = struct.unpack(
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
    if reserved != 0:
        raise ValueError(f"reserved must be 0, got {reserved}")

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

    for idx, pattern in enumerate(tuple_patterns):
        weight_count = 3 ** len(pattern)
        required = weight_count * 4
        end = offset + required
        if end > len(data):
            raise ValueError(f"weights truncated at tuple index {idx}")

        for weight_offset in range(offset, end, 4):
            _ = struct.unpack_from("<f", data, weight_offset)
        offset = end

    if offset != len(data):
        raise ValueError(
            f"unexpected trailing bytes in model data: {len(data) - offset}"
        )


def train_and_export(
    games: int,
    alpha: float,
    lambda_: float,
    epsilon: float,
    output: Path,
    seed: int,
) -> Path:
    """Run training, export the model, and validate the resulting binary."""
    if games < 0:
        raise ValueError(f"games must be >= 0, got {games}")

    output_path = output
    output_path.parent.mkdir(parents=True, exist_ok=True)

    ntuple = NTupleNetwork()
    trainer = TDLambdaTrainer(
        ntuple=ntuple,
        alpha=alpha,
        lambda_=lambda_,
        epsilon=epsilon,
        seed=seed,
    )
    trainer.train(games)
    export_model(ntuple, output_path)
    verify_exported_model(output_path, ntuple.TUPLE_PATTERNS)
    return output_path


def main(argv: list[str] | None = None) -> int:
    """Execute CLI workflow for training and exporting weights.bin."""
    try:
        args = build_parser().parse_args(argv)

        print(
            "Training with "
            f"games={args.games}, alpha={args.alpha}, lambda={args.lambda_}, "
            f"epsilon={args.epsilon}, seed={args.seed}"
        )
        output_path = train_and_export(
            games=args.games,
            alpha=args.alpha,
            lambda_=args.lambda_,
            epsilon=args.epsilon,
            output=args.output,
            seed=args.seed,
        )
        print(f"Model exported and verified: {output_path}")
        return 0
    except Exception as exc:
        print(f"Error: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
