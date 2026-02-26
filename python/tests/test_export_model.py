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
    payload = _exported_payload(ntuple)

    tuple_definitions_len = sum(1 + len(pattern) for pattern in ntuple.TUPLE_PATTERNS)
    weights_len = sum((3 ** len(pattern)) * 4 for pattern in ntuple.TUPLE_PATTERNS)
    expected_len = HEADER_SIZE + tuple_definitions_len + weights_len

    assert len(payload) == expected_len
