import struct
import zlib

import pytest

from export_model import HEADER_SIZE, MAGIC, VERSION
from ntuple import NTupleNetwork
from train import build_parser, main, verify_exported_model


def test_parser_supports_phase_2_6_cli_options() -> None:
    args = build_parser().parse_args(
        [
            "--games",
            "123",
            "--alpha",
            "0.02",
            "--lambda",
            "0.8",
            "--epsilon",
            "0.05",
            "--output",
            "out.bin",
            "--seed",
            "2026",
        ]
    )

    assert args.games == 123
    assert args.alpha == pytest.approx(0.02)
    assert args.lambda_ == pytest.approx(0.8)
    assert args.epsilon == pytest.approx(0.05)
    assert str(args.output) == "out.bin"
    assert args.seed == 2026


def test_main_runs_pipeline_and_outputs_valid_model(tmp_path) -> None:
    run_dir = tmp_path / "valid-model"
    run_dir.mkdir()
    output = run_dir / "weights.bin"

    exit_code = main(["--games", "0", "--output", str(output), "--seed", "42"])

    assert exit_code == 0
    payload = output.read_bytes()
    assert len(payload) >= HEADER_SIZE

    magic, version, num_tuples, crc32, reserved = struct.unpack(
        "<4sIIII", payload[:HEADER_SIZE]
    )
    assert magic == MAGIC
    assert version == VERSION
    assert num_tuples == len(NTupleNetwork().TUPLE_PATTERNS)
    assert reserved == 0
    assert (zlib.crc32(payload[HEADER_SIZE:]) & 0xFFFFFFFF) == crc32


def test_verify_exported_model_detects_crc_mismatch(tmp_path) -> None:
    run_dir = tmp_path / "crc-mismatch"
    run_dir.mkdir()
    output = run_dir / "weights.bin"
    main(["--games", "0", "--output", str(output), "--seed", "42"])

    payload = bytearray(output.read_bytes())
    payload[-1] ^= 0x01
    output.write_bytes(payload)

    with pytest.raises(ValueError, match="CRC32 mismatch"):
        verify_exported_model(output, NTupleNetwork().TUPLE_PATTERNS)
