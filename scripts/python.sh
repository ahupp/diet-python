#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CPYTHON_DIR="$REPO_ROOT/../cpython"
VENV_DIR="$REPO_ROOT/.venv-cpython"
PYTHON_BIN="$CPYTHON_DIR/python"

export UV_CACHE_DIR="$REPO_ROOT/.uv-cache"
export UV_PYTHON="${UV_PYTHON:-python3.14}"

if [ ! -d "$CPYTHON_DIR" ]; then
  echo "cpython checkout not found" >&2
  exit 1
fi

if [ ! -x "$PYTHON_BIN" ]; then
  echo "python not found in ${PYTHON_BIN}" >&2
  exit 1
fi

rm -rf "${VENV_DIR}"
uv venv "$VENV_DIR"

(
  cd "$REPO_ROOT" &&
  VIRTUAL_ENV="$VENV_DIR" PATH="$VENV_DIR/bin:$PATH" \
  uv sync --group dev --no-install-project --frozen --active
)

PYO3_PYTHON_REAL="$("$VENV_DIR/bin/python" - <<'PY'
import os
import sys

print(os.path.realpath(sys.executable))
PY
)" &&
(
  cd "$REPO_ROOT" &&
  PYO3_PYTHON="$PYO3_PYTHON_REAL" cargo build --quiet -p dp-pyo3
)

# Ensure stale bytecode doesn't bypass the transform.
find "$CPYTHON_DIR" -name '*.pyc' -delete

PYTHONPATH_PREFIX="$CPYTHON_DIR/Lib:$REPO_ROOT:$REPO_ROOT/target/debug"

exec env \
  DIET_PYTHON_INSTALL_HOOK=1 \
  PYTHONDONTWRITEBYTECODE=1 \
  PYTHONPATH="$PYTHONPATH_PREFIX${PYTHONPATH:+:$PYTHONPATH}" \
  "$VENV_DIR/bin/python" "$@"
