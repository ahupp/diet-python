#!/usr/bin/env bash
set -euo pipefail

CPYTHON_DIR="cpython"

# Repository root, used to expose ``sitecustomize.py`` for the import hook.
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SKIP_FILE="$REPO_ROOT/cpython_skipped_tests.txt"
EXPECTED_FAILURES_FILE="$REPO_ROOT/EXPECTED_FAILURE.md"
SKIP_EXPECTED_FAILURES="${SKIP_EXPECTED_FAILURES:-1}"

export SOURCE_DATE_EPOCH="$(date +%s)"


if [ ! -d "$CPYTHON_DIR" ]; then
    echo "cpython checkout not found" >&2
    exit 1
fi

PYTHON_BIN="$REPO_ROOT/$CPYTHON_DIR/python"
if [ ! -x "$PYTHON_BIN" ]; then
    echo "python not found in ${PYTHON_BIN}" >&2
    exit 1
fi

(
  cd "$REPO_ROOT" &&
  cargo build --quiet -p dp-pyo3
)

# Expose CPython's standard library and test package so modules are loaded from
# source and can be transformed.

# Ensure stale bytecode doesn't bypass the transform.
find "$CPYTHON_DIR" -name '*.pyc' -delete

PYTHONPATH_PREFIX="$REPO_ROOT/$CPYTHON_DIR/Lib:$REPO_ROOT:$REPO_ROOT/target/debug"
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
if [ "$SKIP_EXPECTED_FAILURES" = "1" ] && [ -f "$EXPECTED_FAILURES_FILE" ]; then
  while IFS= read -r test_id; do
    if [ -n "$test_id" ]; then
      SKIP_ARGS+=(-x "$test_id")
    fi
  done < <(rg -o '`[^`]+`' "$EXPECTED_FAILURES_FILE" | sed 's/`//g' | rg '^test(\\.|_)' | sort -u)
fi

(
  cd "$CPYTHON_DIR" &&
  DIET_PYTHON_INSTALL_HOOK=1 \
  PYTHONDONTWRITEBYTECODE=1 \
  PYTHONPATH="$PYTHONPATH_PREFIX${PYTHONPATH:+:$PYTHONPATH}" \
  "$PYTHON_BIN" -m test -j0 -v "${SKIP_ARGS[@]}" "$@"
)
