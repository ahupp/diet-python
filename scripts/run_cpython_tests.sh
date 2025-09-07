#!/usr/bin/env bash
set -euo pipefail

CPYTHON_DIR="cpython"
VENV_DIR="cpython-venv"
PYTHON_VERSION="${UV_PYTHON_VERSION:-3.12}"

if ! command -v uv >/dev/null 2>&1; then
  echo "uv is required but not installed. Install it from https://astral.sh/uv." >&2
  exit 1
fi

if [ ! -d "$CPYTHON_DIR" ]; then
  git clone --depth 1 https://github.com/python/cpython.git "$CPYTHON_DIR"
fi

uv python install "$PYTHON_VERSION"
if [ ! -d "$VENV_DIR" ]; then
  uv venv "$VENV_DIR" --python "$(uv python find "$PYTHON_VERSION")"
fi

(cd "$CPYTHON_DIR" && "../$VENV_DIR/bin/python" -m test -j0)
