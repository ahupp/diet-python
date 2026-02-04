#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CPYTHON_DIR="$REPO_ROOT/vendor/cpython"
VENV_DIR="$REPO_ROOT/.venv-cpython"
PYTHON_BIN="$CPYTHON_DIR/python"

export UV_CACHE_DIR="$REPO_ROOT/.uv-cache"
export UV_PYTHON="${UV_PYTHON:-python3.14}"

rm -rf "${VENV_DIR}"
uv venv "$VENV_DIR"

(
  cd "$REPO_ROOT" &&
  VIRTUAL_ENV="$VENV_DIR" PATH="$VENV_DIR/bin:$PATH"  \
  uv sync --group dev --no-install-project --frozen --active
)

echo "building soac-pyo3"
(
  cd "$REPO_ROOT" &&
  PYO3_PYTHON_REAL="$($VENV_DIR/bin/python - <<'PY'
import os
import sys

print(os.path.realpath(sys.executable))
PY
)" &&
  PYO3_PYTHON="$PYO3_PYTHON_REAL" cargo build --quiet -p soac-pyo3
)


echo "starting tests"
(
  cd "$REPO_ROOT" &&
  DIET_PYTHON_INTEGRATION_ONLY=1 \
  "$VENV_DIR/bin/python" -m pytest tests/test_eval_source_simple_integration.py
)
