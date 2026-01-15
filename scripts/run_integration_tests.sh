#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CPYTHON_DIR="$REPO_ROOT/cpython"
VENV_DIR="$REPO_ROOT/.venv-cpython"
PYTHON_BIN="$CPYTHON_DIR/python"

if [ ! -x "$PYTHON_BIN" ]; then
  (
    cd "$CPYTHON_DIR" &&
    ./configure &&
    make -j"$(nproc)"
  )
fi

"$PYTHON_BIN" -m venv "$VENV_DIR"
"$VENV_DIR/bin/python" -m pip install --upgrade pip
"$VENV_DIR/bin/python" -m pip install pytest

(
  cd "$REPO_ROOT" &&
  "$VENV_DIR/bin/python" -m pytest tests/
)
