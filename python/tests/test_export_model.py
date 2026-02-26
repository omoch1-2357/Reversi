import struct
from unittest.mock import patch
import zlib

from export_model import HEADER_SIZE, MAGIC, VERSION, export_model
from ntuple import NTupleNetwork


def _exported_payload(ntuple: NTupleNetwork) -> bytes:
    captured: dict[str, bytes] = {}

    def _capture(_self, payload: bytes) -> int:
        captured["payload"] = payload
        return len(payload)

    with patch("export_model.Path.write_bytes", autospec=True, side_effect=_capture):
        export_model(ntuple, "weights.bin")

    return captured["payload"]


def test_export_model_writes_header_magic_version_and_num_tuples() -> None:
    ntuple = NTupleNetwork()
    payload = _exported_payload(ntuple)

    magic, version, num_tuples, _crc32, reserved = struct.unpack(
        "<4sIIII", payload[:HEADER_SIZE]
    )
    assert magic == MAGIC
    assert version == VERSION
    assert num_tuples == len(ntuple.TUPLE_PATTERNS)
    assert reserved == 0


def test_export_model_writes_crc32_for_data_section() -> None:
    ntuple = NTupleNetwork()
    payload = _exported_payload(ntuple)

    _, _, _, expected_crc32, _ = struct.unpack("<4sIIII", payload[:HEADER_SIZE])
    actual_crc32 = zlib.crc32(payload[HEADER_SIZE:]) & 0xFFFFFFFF
    assert expected_crc32 == actual_crc32


def test_export_model_binary_length_matches_tuple_and_weight_layout() -> None:
    ntuple = NTupleNetwork()
    for pattern_idx, weights in enumerate(ntuple.weights):
        weights[0] = pattern_idx + 0.125
        weights[-1] = -(pattern_idx + 0.5)

    payload = _exported_payload(ntuple)

    offset = HEADER_SIZE
    for pattern in ntuple.TUPLE_PATTERNS:
        tuple_size = payload[offset]
        assert tuple_size == len(pattern)
        offset += 1

        positions = list(payload[offset : offset + tuple_size])
        assert positions == pattern
        offset += tuple_size

    for pattern_idx, pattern in enumerate(ntuple.TUPLE_PATTERNS):
        weights = ntuple.weights[pattern_idx]
        expected_len = 3 ** len(pattern)
        assert len(weights) == expected_len

        for weight_idx in range(expected_len):
            raw = payload[offset : offset + 4]
            value = struct.unpack("<f", raw)[0]
            expected_value = float(weights[weight_idx])
            assert value == expected_value
            assert raw == struct.pack("<f", expected_value)
            offset += 4

    assert offset == len(payload)
