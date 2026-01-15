#!/usr/bin/env bash
set -euo pipefail

# Directory to clone the CPython repository into
REPO_DIR="${CPYTHON_DIR:-cpython}"
# Python version to use for the virtual environment
PY_VERSION="${PY_VERSION:-3.12}"
# Path to the virtual environment
VENV_DIR="${VENV_DIR:-.venv}"

# Clone CPython if the repository directory doesn't exist
if [ ! -d "$REPO_DIR" ]; then
  git clone https://github.com/python/cpython.git "$REPO_DIR"
fi

cd "$REPO_DIR"

# Try to check out a tag matching the requested Python version, falling back to
# the maintenance branch for that major/minor version.
MAJOR_MINOR="$(printf '%s' "$PY_VERSION" | cut -d. -f1,2)"
git fetch --tags --quiet
git checkout -q "v$PY_VERSION" 2>/dev/null || git checkout -q "$MAJOR_MINOR"

# Ensure the requested Python version is installed and create the venv
uv python install "$PY_VERSION"
uv venv --python "$PY_VERSION" --allow-existing "$VENV_DIR"

# Run the test suite using the uv-managed Python
PYTHONPATH="$PWD/Lib${PYTHONPATH:+:$PYTHONPATH}" "$VENV_DIR/bin/python" -m test "${TEST_OPTS:--j0}"
