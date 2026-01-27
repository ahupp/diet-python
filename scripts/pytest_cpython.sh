#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CPYTHON_DIR="$REPO_ROOT/../cpython"
VENV_DIR="$REPO_ROOT/.venv-cpython"
PYTHON_BIN="$CPYTHON_DIR/python"
export RUST_BACKTRACE=1
export UV_CACHE_DIR="$REPO_ROOT/.uv-cache"
if [ -z "${UV_PYTHON:-}" ]; then
  export UV_PYTHON="python3.14"
else
  export UV_PYTHON
fi

rm -rf "${VENV_DIR}"
uv venv "$VENV_DIR"

(
  cd "$REPO_ROOT" &&
  VIRTUAL_ENV="$VENV_DIR" PATH="$VENV_DIR/bin:$PATH"  \
  uv sync --group dev --no-install-project --frozen --active
)

echo "building dp-pyo3"
(
  cd "$REPO_ROOT" &&
  PYO3_PYTHON_REAL="$("$VENV_DIR/bin/python" - <<'PY'
import os
import sys

print(os.path.realpath(sys.executable))
PY
)" &&
  PYO3_PYTHON="$PYO3_PYTHON_REAL" cargo build --quiet -p dp-pyo3
)


echo "starting tests"
(
  cd "$REPO_ROOT" &&
  if [ "$#" -eq 0 ]; then
    "$VENV_DIR/bin/python" -m pytest --help
  else
    "$VENV_DIR/bin/python" -m pytest "$@"
  fi
)
