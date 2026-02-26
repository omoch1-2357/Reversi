"""Export the N-Tuple model in the weights.bin binary format."""

from __future__ import annotations

from pathlib import Path
import struct
import zlib

from ntuple import NTupleNetwork

MAGIC = b"NTRV"
VERSION = 1
HEADER_SIZE = 20


def _build_data_section(ntuple: NTupleNetwork) -> bytes:
    if len(ntuple.weights) != len(ntuple.TUPLE_PATTERNS):
        raise ValueError("weights length must match TUPLE_PATTERNS length")

    data = bytearray()

    for pattern in ntuple.TUPLE_PATTERNS:
        if len(pattern) > 255:
            raise ValueError("tuple_size must fit in u8")
        data.append(len(pattern))
        data.extend(bytes(pattern))

    for idx, weights in enumerate(ntuple.weights):
        expected_len = 3 ** len(ntuple.TUPLE_PATTERNS[idx])
        if len(weights) != expected_len:
            raise ValueError(
                f"weights[{idx}] length must be {expected_len}, got {len(weights)}"
            )
        for weight in weights:
            data.extend(struct.pack("<f", float(weight)))

    return bytes(data)


def export_model(ntuple: NTupleNetwork, path: str | Path) -> None:
    """Write an NTupleNetwork to weights.bin format."""

    data = _build_data_section(ntuple)
    data_crc32 = zlib.crc32(data) & 0xFFFFFFFF

    header = bytearray()
    header.extend(MAGIC)
    header.extend(struct.pack("<I", VERSION))
    header.extend(struct.pack("<I", len(ntuple.TUPLE_PATTERNS)))
    header.extend(struct.pack("<I", data_crc32))
    header.extend(struct.pack("<I", 0))

    output = Path(path)
    output.write_bytes(bytes(header) + data)
