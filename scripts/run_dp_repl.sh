#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PYTHON_BIN="${PYTHON_BIN:-}"

if [ -z "$PYTHON_BIN" ]; then
  if [ -x "$REPO_ROOT/cpython/python" ]; then
    PYTHON_BIN="$REPO_ROOT/cpython/python"
  elif command -v python3 >/dev/null 2>&1; then
    PYTHON_BIN="$(command -v python3)"
  else
    PYTHON_BIN="$(command -v python)"
  fi
fi

(
  cd "$REPO_ROOT" &&
  cargo build --quiet -p dp-pyo3
)

export PYTHONDONTWRITEBYTECODE=1
export DIET_PYTHON_INSTALL_HOOK=1
export PYTHONPATH="$REPO_ROOT:$REPO_ROOT/target/debug${PYTHONPATH:+:$PYTHONPATH}"

exec "$PYTHON_BIN" -i -c "import diet_python as dp; print('diet_python loaded')"
