"""Convert legacy Reversi weights.bin models to the latest version."""

from __future__ import annotations

import argparse
from pathlib import Path
import struct
import zlib

from export_model import HEADER_SIZE, MAGIC, VERSION
from ntuple import NTupleNetwork
from rust_training import compress_model_bytes, decompress_model_bytes


LEGACY_SCALE = 0.5


def _read_header(payload: bytes) -> tuple[int, int, int]:
    if len(payload) < HEADER_SIZE:
        raise ValueError(
            f"model payload too short: expected at least {HEADER_SIZE} bytes, got {len(payload)}"
        )

    magic, version, num_tuples, expected_crc32, reserved = struct.unpack(
        "<4sIIII", payload[:HEADER_SIZE]
    )
    if magic != MAGIC:
        raise ValueError(f"invalid magic: expected {MAGIC!r}, got {magic!r}")

    actual_crc32 = zlib.crc32(payload[HEADER_SIZE:]) & 0xFFFFFFFF
    if actual_crc32 != expected_crc32:
        raise ValueError(
            f"CRC32 mismatch: expected {expected_crc32:#010x}, got {actual_crc32:#010x}"
        )

    return version, num_tuples, reserved


def convert_model_to_v3(input_path: str | Path, output_path: str | Path) -> Path:
    """Convert a version 1/2/3 weights.bin file into the latest version."""
    payload = decompress_model_bytes(Path(input_path).read_bytes())
    version, num_tuples, reserved = _read_header(payload)

    if version == VERSION:
        Path(output_path).write_bytes(compress_model_bytes(payload))
        return Path(output_path)
    if version not in (1, 2, 3):
        raise ValueError(f"unsupported legacy version: {version}")
    if num_tuples != len(NTupleNetwork.TUPLE_PATTERNS):
        raise ValueError(
            f"num_tuples mismatch: expected {len(NTupleNetwork.TUPLE_PATTERNS)}, got {num_tuples}"
        )

    phase_count = 1 if version == 1 else reserved
    if phase_count <= 0:
        raise ValueError(
            f"phase_count must be > 0 for version {version}, got {phase_count}"
        )

    data = bytearray()
    offset = HEADER_SIZE
    for pattern in NTupleNetwork.TUPLE_PATTERNS:
        tuple_size = payload[offset]
        offset += 1
        if tuple_size != len(pattern):
            raise ValueError(
                f"tuple_size mismatch: expected {len(pattern)}, got {tuple_size}"
            )
        positions = bytes(payload[offset : offset + tuple_size])
        if list(positions) != pattern:
            raise ValueError(
                f"tuple positions mismatch: expected {pattern}, got {list(positions)}"
            )
        data.append(tuple_size)
        data.extend(positions)
        offset += tuple_size

    scale = LEGACY_SCALE if version in (1, 2) else 1.0

    for _phase in range(phase_count):
        for pattern in NTupleNetwork.TUPLE_PATTERNS:
            weight_count = 3 ** len(pattern)
            for _ in range(weight_count):
                weight = struct.unpack_from("<f", payload, offset)[0]
                data.extend(struct.pack("<f", weight * scale))
                offset += 4

    for _phase in range(phase_count):
        for pattern in NTupleNetwork.TUPLE_PATTERNS:
            for _ in range(3 ** len(pattern)):
                data.extend(struct.pack("<I", 0))

    if offset != len(payload):
        raise ValueError(
            f"unexpected trailing bytes in legacy model: {len(payload) - offset}"
        )

    crc32 = zlib.crc32(data) & 0xFFFFFFFF
    converted = bytearray()
    converted.extend(
        struct.pack("<4sIIII", MAGIC, VERSION, num_tuples, crc32, phase_count)
    )
    converted.extend(data)

    out_path = Path(output_path)
    out_path.write_bytes(compress_model_bytes(bytes(converted)))
    return out_path


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Convert Reversi weights.bin to the latest version."
    )
    parser.add_argument("input", type=Path, help="Input weights.bin path")
    parser.add_argument("output", type=Path, help="Output weights.bin path")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    convert_model_to_v3(args.input, args.output)
    print(f"Converted {args.input} -> {args.output} (version {VERSION})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
