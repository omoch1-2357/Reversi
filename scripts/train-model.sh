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

has_cli_option() {
  local long_option="$1"
  shift

  local arg
  for arg in "$@"; do
    if [[ "${arg}" == "${long_option}" || "${arg}" == "${long_option}="* ]]; then
      return 0
    fi
  done

  return 1
}

echo "> ${PYTHON_BIN} -m pip install -r python/requirements.txt"
"${PYTHON_BIN}" -m pip install -r python/requirements.txt

if ! command -v maturin >/dev/null 2>&1; then
  echo "maturin was not found in PATH after installing python requirements." >&2
  exit 1
fi

echo "> maturin build --release --manifest-path python/rust_training_ext/Cargo.toml --out python/dist --interpreter ${PYTHON_BIN}"
maturin build \
  --release \
  --manifest-path python/rust_training_ext/Cargo.toml \
  --out python/dist \
  --interpreter "${PYTHON_BIN}"

shopt -s nullglob
wheels=(python/dist/reversi_training_ext-*.whl)
shopt -u nullglob
if [[ "${#wheels[@]}" -eq 0 ]]; then
  echo "No built wheel was found under python/dist." >&2
  exit 1
fi

wheel_path=""
for wheel in "${wheels[@]}"; do
  if [[ -z "${wheel_path}" || "${wheel}" -nt "${wheel_path}" ]]; then
    wheel_path="${wheel}"
  fi
done

echo "> ${PYTHON_BIN} -m pip install --force-reinstall ${wheel_path}"
"${PYTHON_BIN}" -m pip install --force-reinstall "${wheel_path}"

train_args=("$@")
if ! has_cli_option "--games" "${train_args[@]}"; then
  train_args+=("--games" "500000")
fi
if ! has_cli_option "--progress-interval" "${train_args[@]}"; then
  train_args+=("--progress-interval" "10000")
fi
if ! has_cli_option "--output" "${train_args[@]}"; then
  train_args+=("--output" "rust/src/ai/weights.bin")
fi

echo "> ${PYTHON_BIN} python/train.py ${train_args[*]}"
"${PYTHON_BIN}" python/train.py "${train_args[@]}"
