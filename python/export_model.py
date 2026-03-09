"""Export the N-Tuple model in the weights.bin binary format."""

from __future__ import annotations

from pathlib import Path
import struct
import zlib

from ntuple import NTupleNetwork
from rust_training import compress_model_bytes

MAGIC = b"NTRV"
VERSION = 3
HEADER_SIZE = 20


def _build_data_section(ntuple: NTupleNetwork) -> bytes:
    if len(ntuple.weights) != ntuple.PHASE_COUNT:
        raise ValueError(
            f"weights phase length must be {ntuple.PHASE_COUNT}, got {len(ntuple.weights)}"
        )

    data = bytearray()

    for pattern_idx, pattern in enumerate(ntuple.TUPLE_PATTERNS):
        if len(pattern) > 255:
            raise ValueError("tuple_size must fit in u8")
        for pos_idx, pos in enumerate(pattern):
            if pos < 0 or pos > 63:
                raise ValueError(
                    f"pattern[{pattern_idx}] has invalid board index "
                    f"{pos} at position {pos_idx}: {pattern}"
                )
        data.append(len(pattern))
        data.extend(bytes(pattern))

    for phase_idx, phase_weights in enumerate(ntuple.weights):
        if len(phase_weights) != len(ntuple.TUPLE_PATTERNS):
            raise ValueError(
                f"weights[{phase_idx}] tuple length must match TUPLE_PATTERNS length"
            )
        for tuple_idx, weights in enumerate(phase_weights):
            expected_len = 3 ** len(ntuple.TUPLE_PATTERNS[tuple_idx])
            if len(weights) != expected_len:
                raise ValueError(
                    f"weights[{phase_idx}][{tuple_idx}] length must be "
                    f"{expected_len}, got {len(weights)}"
                )
            for weight in weights:
                data.extend(struct.pack("<f", float(weight)))

    return bytes(data)


def export_model(ntuple: NTupleNetwork, path: str | Path) -> None:
    """Write an NTupleNetwork to weights.bin format."""

    data = _build_data_section(ntuple)
    data_crc32 = zlib.crc32(data) & 0xFFFFFFFF

    header_format = "<4sIIII"
    packed_header = struct.pack(
        header_format,
        MAGIC,
        VERSION,
        len(ntuple.TUPLE_PATTERNS),
        data_crc32,
        ntuple.PHASE_COUNT,
    )
    if len(packed_header) != struct.calcsize(header_format):
        raise ValueError("packed header size mismatch")

    header = bytearray()
    header.extend(packed_header)

    output = Path(path)
    output.write_bytes(compress_model_bytes(bytes(header) + data))
