#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${REPO_ROOT}"

if command -v python3 >/dev/null 2>&1; then
  PYTHON_BIN="python3"
elif command -v python >/dev/null 2>&1; then
  PYTHON_BIN="python"
else
  echo "python3 or python was not found in PATH." >&2
  exit 1
fi

if ! command -v maturin >/dev/null 2>&1; then
  echo "maturin was not found in PATH. Install python requirements first." >&2
  exit 1
fi

has_output=0
for arg in "$@"; do
  if [[ "${arg}" == "--output" || "${arg}" == --output=* ]]; then
    has_output=1
    break
  fi
done

echo "> ${PYTHON_BIN} -m pip install -r python/requirements.txt"
"${PYTHON_BIN}" -m pip install -r python/requirements.txt

echo "> maturin build --manifest-path python/rust_training_ext/Cargo.toml --out python/dist --interpreter ${PYTHON_BIN}"
maturin build \
  --manifest-path python/rust_training_ext/Cargo.toml \
  --out python/dist \
  --interpreter "${PYTHON_BIN}"

wheels=(python/dist/reversi_training_ext-*.whl)
if [[ "${#wheels[@]}" -eq 0 ]]; then
  echo "No built wheel was found under python/dist." >&2
  exit 1
fi

wheel_path="$(ls -t "${wheels[@]}" | head -n 1)"

echo "> ${PYTHON_BIN} -m pip install --force-reinstall ${wheel_path}"
"${PYTHON_BIN}" -m pip install --force-reinstall "${wheel_path}"

train_args=("$@")
if [[ "${has_output}" -eq 0 ]]; then
  train_args+=("--output" "python/dist/weights.bin")
fi

echo "> ${PYTHON_BIN} python/train.py ${train_args[*]}"
"${PYTHON_BIN}" python/train.py "${train_args[@]}"
