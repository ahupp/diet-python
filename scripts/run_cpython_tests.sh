#!/usr/bin/env bash
set -euo pipefail

CPYTHON_DIR="cpython"
VENV_DIR="cpython-venv"
PYTHON_VERSION="${UV_PYTHON_VERSION:-3.12}"

# Repository root, used to expose ``sitecustomize.py`` for the import hook.
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

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

# Ensure ``sitecustomize`` is on the import path so the diet-python import hook
# is installed for all modules loaded during the test run.
(cd "$CPYTHON_DIR" && PYTHONPATH="$REPO_ROOT${PYTHONPATH:+:$PYTHONPATH}" "../$VENV_DIR/bin/python" -m test -j0 "$@")
