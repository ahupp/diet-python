#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CPYTHON_DIR="$REPO_ROOT/vendor/cpython"
VENV_DIR="$REPO_ROOT/.venv-cpython"
PYTHON_BIN="$CPYTHON_DIR/python"

export UV_CACHE_DIR="$REPO_ROOT/.uv-cache"
export UV_PYTHON="${UV_PYTHON:-$PYTHON_BIN}"

if [ ! -d "$CPYTHON_DIR" ]; then
  echo "cpython checkout not found at ${CPYTHON_DIR}" >&2
  exit 1
fi
if [ ! -x "$PYTHON_BIN" ]; then
  echo "python not found in ${PYTHON_BIN}" >&2
  exit 1
fi

rm -rf "${VENV_DIR}"
uv venv --python "$PYTHON_BIN" "$VENV_DIR"

(
  cd "$REPO_ROOT" &&
  VIRTUAL_ENV="$VENV_DIR" PATH="$VENV_DIR/bin:$PATH"  \
  uv sync --group dev --no-install-project --frozen --active
)

echo "building soac-pyo3"
(
  cd "$REPO_ROOT" &&
  PYO3_PYTHON_REAL="$("$VENV_DIR/bin/python" - <<'PY'
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
  if [ "$#" -eq 0 ]; then
    "$VENV_DIR/bin/python" -m pytest --help
  else
    "$VENV_DIR/bin/python" -m pytest "$@"
  fi
)
