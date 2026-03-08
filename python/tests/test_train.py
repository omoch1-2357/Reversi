import struct
import zlib
from pathlib import Path

import pytest

from export_model import HEADER_SIZE, MAGIC, VERSION
from ntuple import NTupleNetwork
from rust_training import compress_model_bytes, decompress_model_bytes
from train import build_parser, main, verify_exported_model


OUTPUT_DIR = Path(__file__).resolve().parent


def _output_path(name: str) -> Path:
    path = OUTPUT_DIR / name
    path.unlink(missing_ok=True)
    return path


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
            "--progress-interval",
            "25",
        ]
    )

    assert args.games == 123
    assert args.alpha == pytest.approx(0.02)
    assert args.lambda_ == pytest.approx(0.8)
    assert args.epsilon == pytest.approx(0.05)
    assert str(args.output) == "out.bin"
    assert args.seed == 2026
    assert args.progress_interval == 25


def test_main_runs_pipeline_and_outputs_valid_model() -> None:
    output = _output_path("_generated_valid_weights.bin")

    try:
        exit_code = main(["--games", "0", "--output", str(output), "--seed", "42"])

        assert exit_code == 0
        payload = decompress_model_bytes(output.read_bytes())
        assert len(payload) >= HEADER_SIZE

        magic, version, num_tuples, crc32, phase_count = struct.unpack(
            "<4sIIII", payload[:HEADER_SIZE]
        )
        assert magic == MAGIC
        assert version == VERSION
        assert num_tuples == len(NTupleNetwork.TUPLE_PATTERNS)
        assert phase_count == NTupleNetwork.PHASE_COUNT
        assert (zlib.crc32(payload[HEADER_SIZE:]) & 0xFFFFFFFF) == crc32
    finally:
        output.unlink(missing_ok=True)


def test_verify_exported_model_detects_crc_mismatch() -> None:
    output = _output_path("_generated_crc_weights.bin")

    try:
        result = main(["--games", "0", "--output", str(output), "--seed", "42"])
        assert result == 0

        payload = bytearray(decompress_model_bytes(output.read_bytes()))
        payload[-1] ^= 0x01
        output.write_bytes(compress_model_bytes(payload))

        with pytest.raises(ValueError, match="CRC32 mismatch"):
            verify_exported_model(output, NTupleNetwork.TUPLE_PATTERNS)
    finally:
        output.unlink(missing_ok=True)


def test_main_emits_progress_logs(capsys: pytest.CaptureFixture[str]) -> None:
    output = _output_path("_generated_progress_weights.bin")

    try:
        exit_code = main(
            [
                "--games",
                "3",
                "--output",
                str(output),
                "--seed",
                "42",
                "--progress-interval",
                "2",
            ]
        )

        assert exit_code == 0
        captured = capsys.readouterr()
        assert "progress_interval=2" in captured.out
        assert "[progress] 2/3 games" in captured.out
        assert "[progress] 3/3 games" in captured.out
    finally:
        output.unlink(missing_ok=True)
