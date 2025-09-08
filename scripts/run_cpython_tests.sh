#!/usr/bin/env bash
set -euo pipefail

CPYTHON_DIR="cpython"
VENV_DIR="cpython-venv"
PYTHON_VERSION="${UV_PYTHON_VERSION:-3.12}"

# Repository root, used to expose ``sitecustomize.py`` for the import hook.
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

export SOURCE_DATE_EPOCH="$(date +%s)"

TRANSFORMS_SET=0
TRANSFORMS=""
ARGS=()
while [[ $# -gt 0 ]]; do
  case $1 in
    --transforms)
      TRANSFORMS_SET=1
      TRANSFORMS="$2"
      shift 2
      ;;
    *)
      ARGS+=("$1")
      shift
      ;;
  esac
done
set -- "${ARGS[@]}"

if ! command -v uv >/dev/null 2>&1; then
  echo "uv is required but not installed. Install it from https://astral.sh/uv." >&2
  exit 1
fi

if [ ! -d "$CPYTHON_DIR" ]; then
    git clone --depth 1 --branch "$PYTHON_VERSION" https://github.com/python/cpython.git "$CPYTHON_DIR"
else
    git -C "$CPYTHON_DIR" fetch origin
    git -C "$CPYTHON_DIR" switch "$PYTHON_VERSION"
fi

uv python install "$PYTHON_VERSION"
if [ ! -d "$VENV_DIR" ]; then
  uv venv "$VENV_DIR" --python "$(uv python find "$PYTHON_VERSION")"
fi

# Ensure stale bytecode doesn't bypass the transform.
find "$CPYTHON_DIR" -name '*.pyc' -delete

# The Python builds installed by ``uv`` exclude the ``test`` package. Expose it
# from the CPython source tree via ``PYTHONPATH`` so ``python -m test`` is
# available without modifying the virtual environment.

# Ensure ``sitecustomize`` and CPython's ``Lib`` (which contains the ``test``
# package) are on the import path so the diet-python import hook is installed
# for all modules loaded during the test run and the standard library ``test``
# package is available.
if [ $TRANSFORMS_SET -eq 1 ]; then
  (cd "$CPYTHON_DIR" && PYTHONDONTWRITEBYTECODE=1 DIET_PYTHON_TRANSFORMS="$TRANSFORMS" PYTHONPATH="$REPO_ROOT:$REPO_ROOT/$CPYTHON_DIR/Lib${PYTHONPATH:+:$PYTHONPATH}" "../$VENV_DIR/bin/python" -m test -j0 "$@")
else
  (cd "$CPYTHON_DIR" && PYTHONDONTWRITEBYTECODE=1 PYTHONPATH="$REPO_ROOT:$REPO_ROOT/$CPYTHON_DIR/Lib${PYTHONPATH:+:$PYTHONPATH}" "../$VENV_DIR/bin/python" -m test -j0 "$@")
fi
