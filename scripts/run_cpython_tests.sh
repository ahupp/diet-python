#!/usr/bin/env bash
set -euo pipefail

CPYTHON_DIR="cpython"
PYTHON_VERSION="${UV_PYTHON_VERSION:-3.12}"
PREFIX_DIR="cpython-install-$PYTHON_VERSION"

# Repository root, used to expose ``sitecustomize.py`` for the import hook.
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SKIP_FILE="$REPO_ROOT/cpython_skipped_tests.txt"

export SOURCE_DATE_EPOCH="$(date +%s)"

if [ ! -d "$CPYTHON_DIR" ]; then
    git clone --depth 1 --branch "$PYTHON_VERSION" https://github.com/python/cpython.git "$CPYTHON_DIR"
else
    if ! git -C "$CPYTHON_DIR" switch "$PYTHON_VERSION" >/dev/null 2>&1; then
        if [ "${FETCH_CPYTHON:-0}" = "1" ]; then
            git -C "$CPYTHON_DIR" fetch origin
            git -C "$CPYTHON_DIR" switch "$PYTHON_VERSION"
        else
            echo "cpython branch $PYTHON_VERSION not found; rerun with FETCH_CPYTHON=1 to update." >&2
            exit 1
        fi
    fi
fi

PYTHON_BIN="$REPO_ROOT/$PREFIX_DIR/bin/python3"
if [ ! -x "$PYTHON_BIN" ]; then
  (
    cd "$CPYTHON_DIR" &&
    ./configure --prefix="$REPO_ROOT/$PREFIX_DIR" &&
    make -j"$(nproc)" &&
    make install
  )
fi

# Expose CPython's standard library and test package so modules are loaded from
# source and can be transformed.

# Ensure stale bytecode doesn't bypass the transform.
find "$CPYTHON_DIR" -name '*.pyc' -delete

PYTHONPATH_PREFIX="$REPO_ROOT/$CPYTHON_DIR/Lib:$REPO_ROOT"
SKIP_ARGS=()
if [ -f "$SKIP_FILE" ]; then
  while IFS= read -r line; do
    trimmed="$(printf '%s' "$line" | sed 's/[[:space:]]*$//')"
    if [ -z "$trimmed" ] || [ "${trimmed#\#}" != "$trimmed" ]; then
      continue
    fi
    SKIP_ARGS+=(-x "$trimmed")
  done < "$SKIP_FILE"
fi

(
  cd "$CPYTHON_DIR" &&
  PYTHONDONTWRITEBYTECODE=1 \
  PYTHONPATH="$PYTHONPATH_PREFIX${PYTHONPATH:+:$PYTHONPATH}" \
  "$PYTHON_BIN" -m test -j0 "${SKIP_ARGS[@]}" "$@"
)
