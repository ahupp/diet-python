#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CPYTHON_DIR="$REPO_ROOT/cpython"
VENV_DIR="$REPO_ROOT/.venv-cpython"
PYTHON_BIN="$CPYTHON_DIR/python"
export UV_CACHE_DIR="$REPO_ROOT/.uv-cache"
export UV_PYTHON="$PYTHON_BIN"

if test ! -x "$PYTHON_BIN" ; then
  (
    cd "$CPYTHON_DIR" &&
    ./configure &&
    make -j"$(nproc)"
  )
fi

if test ! -d "$VENV_DIR" ; then
  uv venv "$VENV_DIR"
  (
    cd "$REPO_ROOT" &&
    VIRTUAL_ENV="$VENV_DIR" PATH="$VENV_DIR/bin:$PATH"  \
    uv sync --group dev --no-install-project --frozen --active
  )
fi

(
  cd "$REPO_ROOT" &&
  cargo build --quiet -p dp-pyo3
)

(
  cd "$REPO_ROOT" &&
  if [ "$#" -eq 0 ]; then
    "$VENV_DIR/bin/python" -m pytest --help
  else
    "$VENV_DIR/bin/python" -m pytest "$@"
  fi
)
