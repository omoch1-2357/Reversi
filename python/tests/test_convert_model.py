import struct
import zlib
from pathlib import Path

import pytest

from convert_model import LEGACY_SCALE, convert_model_to_v3
from export_model import HEADER_SIZE, MAGIC, VERSION
from ntuple import NTupleNetwork
from rust_training import compress_model_bytes, decompress_model_bytes

OUTPUT_DIR = Path(__file__).resolve().parent


def _output_path(name: str) -> Path:
    path = OUTPUT_DIR / name
    path.unlink(missing_ok=True)
    return path


def _legacy_v2_bytes(ntuple: NTupleNetwork, phase_count: int) -> bytes:
    data = bytearray()

    for pattern in ntuple.TUPLE_PATTERNS:
        data.append(len(pattern))
        data.extend(bytes(pattern))

    for phase_idx in range(phase_count):
        for tuple_idx, pattern in enumerate(ntuple.TUPLE_PATTERNS):
            weights = ntuple.weights[phase_idx][tuple_idx]
            assert len(weights) == 3 ** len(pattern)
            for weight in weights:
                data.extend(struct.pack("<f", float(weight)))

    crc32 = zlib.crc32(data) & 0xFFFFFFFF
    header = struct.pack(
        "<4sIIII", MAGIC, 2, len(ntuple.TUPLE_PATTERNS), crc32, phase_count
    )
    return compress_model_bytes(header + data)


def test_convert_model_to_v3_rewrites_version_and_scales_weights() -> None:
    ntuple = NTupleNetwork()
    ntuple.weights[0][0][0] = 2.0
    ntuple.weights[1][-1][-1] = -6.0

    src = _output_path("_legacy_v2_weights.bin")
    dst = _output_path("_converted_v3_weights.bin")
    try:
        src.write_bytes(_legacy_v2_bytes(ntuple, 2))

        convert_model_to_v3(src, dst)
        payload = decompress_model_bytes(dst.read_bytes())

        magic, version, num_tuples, expected_crc32, phase_count = struct.unpack(
            "<4sIIII", payload[:HEADER_SIZE]
        )
        assert magic == MAGIC
        assert version == VERSION
        assert num_tuples == len(ntuple.TUPLE_PATTERNS)
        assert phase_count == 2
        assert (zlib.crc32(payload[HEADER_SIZE:]) & 0xFFFFFFFF) == expected_crc32

        offset = HEADER_SIZE
        for pattern in ntuple.TUPLE_PATTERNS:
            offset += 1 + len(pattern)

        first_weight = struct.unpack_from("<f", payload, offset)[0]
        assert first_weight == pytest.approx(2.0 * LEGACY_SCALE)

        last_weight_offset = offset
        for phase in ntuple.weights[:phase_count]:
            for weights in phase:
                last_weight_offset += len(weights) * 4
        last_weight_offset -= 4
        last_weight = struct.unpack_from("<f", payload, last_weight_offset)[0]
        assert last_weight == pytest.approx(-6.0 * LEGACY_SCALE)

        visit_count_offset = last_weight_offset + 4
        visit_count = struct.unpack_from("<I", payload, visit_count_offset)[0]
        assert visit_count == 0
    finally:
        src.unlink(missing_ok=True)
        dst.unlink(missing_ok=True)


def test_convert_model_to_v3_accepts_existing_v3_as_passthrough() -> None:
    ntuple = NTupleNetwork()
    src = _output_path("_already_v3_weights.bin")
    dst = _output_path("_passthrough_v3_weights.bin")
    try:
        legacy = _legacy_v2_bytes(ntuple, 1)

        payload = bytearray(decompress_model_bytes(legacy))
        payload[4:8] = struct.pack("<I", VERSION)
        payload[12:16] = struct.pack(
            "<I", zlib.crc32(payload[HEADER_SIZE:]) & 0xFFFFFFFF
        )
        src.write_bytes(compress_model_bytes(payload))

        convert_model_to_v3(src, dst)
        assert decompress_model_bytes(dst.read_bytes()) == bytes(payload)
    finally:
        src.unlink(missing_ok=True)
        dst.unlink(missing_ok=True)
