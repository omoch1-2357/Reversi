from contextlib import contextmanager
import shutil
import struct
from pathlib import Path
import uuid
import zlib

import pytest

from export_model import HEADER_SIZE, MAGIC, VERSION
from ntuple import NTupleNetwork
from train import build_parser, main, verify_exported_model

TEST_TMP_ROOT = Path(__file__).resolve().parents[1] / ".tmp"


@contextmanager
def _workspace_tempdir():
    TEST_TMP_ROOT.mkdir(parents=True, exist_ok=True)
    temp_dir = TEST_TMP_ROOT / f"test-{uuid.uuid4().hex}"
    temp_dir.mkdir()
    try:
        yield temp_dir
    finally:
        shutil.rmtree(temp_dir, ignore_errors=True)


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


def test_main_runs_pipeline_and_outputs_valid_model() -> None:
    with _workspace_tempdir() as temp_dir:
        output = temp_dir / "weights.bin"

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


def test_verify_exported_model_detects_crc_mismatch() -> None:
    with _workspace_tempdir() as temp_dir:
        output = temp_dir / "weights.bin"
        main(["--games", "0", "--output", str(output), "--seed", "42"])

        payload = bytearray(output.read_bytes())
        payload[-1] ^= 0x01
        output.write_bytes(payload)

        with pytest.raises(ValueError, match="CRC32 mismatch"):
            verify_exported_model(output, NTupleNetwork().TUPLE_PATTERNS)
