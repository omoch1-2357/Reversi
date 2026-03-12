import json
import struct
import zlib
from pathlib import Path

import pytest

from export_model import HEADER_SIZE, MAGIC, VERSION
from ntuple import NTupleNetwork
from rust_training import compress_model_bytes, decompress_model_bytes
from train import build_parser, main, train_and_export, verify_exported_model


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
            "--alpha-decay",
            "inverse_visit",
            "--alpha-decay-start-game",
            "12",
            "--lambda",
            "0.8",
            "--epsilon",
            "0.05",
            "--output",
            "out.bin",
            "--seed",
            "2026",
            "--threads",
            "0",
            "--progress-interval",
            "25",
            "--random-opening-plies",
            "4",
            "--checkpoint-interval",
            "50",
            "--checkpoint-dir",
            "checkpoints",
            "--resume-from",
            "resume.bin",
            "--status-file",
            "status.json",
            "--no-verify",
        ]
    )

    assert args.games == 123
    assert args.alpha == pytest.approx(0.02)
    assert args.alpha_decay == "inverse_visit"
    assert args.alpha_decay_start_game == 12
    assert args.lambda_ == pytest.approx(0.8)
    assert args.epsilon == pytest.approx(0.05)
    assert str(args.output) == "out.bin"
    assert args.seed == 2026
    assert args.threads == 0
    assert args.progress_interval == 25
    assert args.random_opening_plies == 4
    assert args.checkpoint_interval == 50
    assert str(args.checkpoint_dir) == "checkpoints"
    assert str(args.resume_from) == "resume.bin"
    assert str(args.status_file) == "status.json"
    assert args.verify is False


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
    status = _output_path("_generated_progress_status.json")

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
                "--status-file",
                str(status),
            ]
        )

        assert exit_code == 0
        captured = capsys.readouterr()
        assert "threads=0" in captured.out
        assert "alpha_decay=none" in captured.out
        assert "alpha_decay_start_game=0" in captured.out
        assert "progress_interval=2" in captured.out
        assert "random_opening_plies=0" in captured.out
        assert "checkpoint_interval=0" in captured.out
        assert f"status_file={status}" in captured.out
        assert "[progress] 2/3 games" in captured.out
        assert "[progress] 3/3 games" in captured.out
        status_payload = json.loads(status.read_text())
        assert status_payload["state"] == "completed"
        assert status_payload["completed_games"] == 3
        assert status_payload["total_games"] == 3
        assert status_payload["output_path"] == str(output)
    finally:
        output.unlink(missing_ok=True)
        status.unlink(missing_ok=True)


def test_parser_defaults_threads_to_zero() -> None:
    args = build_parser().parse_args([])
    assert args.threads == 0
    assert args.alpha_decay == "none"
    assert args.alpha_decay_start_game == 0
    assert args.status_file is None
    assert args.verify is True


def test_train_and_export_writes_checkpoints_and_resumes(monkeypatch) -> None:
    output = _output_path("_generated_checkpoint_weights.bin")
    resume = _output_path("_resume_checkpoint_weights.bin")
    checkpoint_dir = OUTPUT_DIR / "_checkpoints"
    checkpoint_dir.mkdir(exist_ok=True)
    for path in checkpoint_dir.glob("*"):
        path.unlink()

    seed_model = train_and_export(
        games=0,
        alpha=0.01,
        lambda_=0.7,
        epsilon=0.1,
        output=resume,
        seed=42,
        threads=1,
        random_opening_plies=0,
        alpha_decay="none",
        alpha_decay_start_game=0,
        progress_interval=0,
        checkpoint_interval=0,
        checkpoint_dir=None,
        resume_from=None,
        status_file=None,
        verify=True,
    )
    resume_bytes = seed_model.read_bytes()
    calls: list[dict[str, object]] = []

    def _train_to_bytes(**kwargs):
        calls.append(kwargs)
        callback = kwargs["progress_callback"]
        callback(kwargs["games"], kwargs["games"], 0.0)
        return resume_bytes

    monkeypatch.setattr("train.train_to_bytes", _train_to_bytes)

    try:
        result = train_and_export(
            games=5,
            alpha=0.01,
            lambda_=0.7,
            epsilon=0.1,
            output=output,
            seed=42,
            threads=2,
            random_opening_plies=4,
            alpha_decay="inverse_game",
            alpha_decay_start_game=9,
            progress_interval=10,
            checkpoint_interval=2,
            checkpoint_dir=checkpoint_dir,
            resume_from=resume,
            status_file=None,
            verify=True,
        )

        assert result == output
        assert [call["games"] for call in calls] == [2, 2, 1]
        assert calls[0]["initial_model"] == resume_bytes
        assert all(call["random_opening_plies"] == 4 for call in calls)
        assert all(call["alpha_decay"] == "inverse_game" for call in calls)
        assert [call["alpha_decay_start_game"] for call in calls] == [9, 11, 13]
        checkpoints = sorted(checkpoint_dir.glob("*.bin"))
        assert len(checkpoints) == 3
        assert output.exists()
    finally:
        output.unlink(missing_ok=True)
        resume.unlink(missing_ok=True)
        for path in checkpoint_dir.glob("*"):
            path.unlink()
        checkpoint_dir.rmdir()


def test_train_and_export_rejects_inverse_visit_with_checkpoints() -> None:
    output = _output_path("_generated_inverse_visit_checkpoint_weights.bin")

    try:
        with pytest.raises(ValueError, match="visit counts are not serialized"):
            train_and_export(
                games=5,
                alpha=0.01,
                lambda_=0.7,
                epsilon=0.1,
                output=output,
                seed=42,
                threads=1,
                random_opening_plies=0,
                alpha_decay="inverse_visit",
                alpha_decay_start_game=0,
                progress_interval=0,
                checkpoint_interval=1,
                checkpoint_dir=None,
                resume_from=None,
                status_file=None,
                verify=True,
            )
    finally:
        output.unlink(missing_ok=True)


def test_main_writes_failed_status_file(monkeypatch) -> None:
    status = _output_path("_generated_failed_status.json")

    def _raise(**_kwargs):
        raise RuntimeError("training exploded")

    monkeypatch.setattr("train.train_and_export", _raise)

    try:
        exit_code = main(["--games", "5", "--status-file", str(status)])

        assert exit_code == 1
        status_payload = json.loads(status.read_text())
        assert status_payload["state"] == "failed"
        assert status_payload["total_games"] == 5
        assert status_payload["error"] == "training exploded"
    finally:
        status.unlink(missing_ok=True)
