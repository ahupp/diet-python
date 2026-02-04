#!/usr/bin/env bash
set -euo pipefail

# Repository root, used to expose ``sitecustomize.py`` for the import hook.
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CPYTHON_DIR="$REPO_ROOT/vendor/cpython"
SKIP_FILE="$REPO_ROOT/cpython_skipped_tests.txt"
EXPECTED_FAILURES_FILE="$REPO_ROOT/EXPECTED_FAILURE.md"
SKIP_EXPECTED_FAILURES="${SKIP_EXPECTED_FAILURES:-1}"
MEMORY_LIMIT_MB="${DIET_PYTHON_MEMORY_LIMIT_MB:-16384}"

export SOURCE_DATE_EPOCH="$(date +%s)"


if [ ! -d "$CPYTHON_DIR" ]; then
    echo "cpython checkout not found" >&2
    exit 1
fi

PYTHON_BIN="$CPYTHON_DIR/python"
if [ ! -x "$PYTHON_BIN" ]; then
    echo "python not found in ${PYTHON_BIN}" >&2
    exit 1
fi

(
  cd "$REPO_ROOT" &&
  cargo build --quiet -p soac-pyo3
)

# Expose CPython's standard library and test package so modules are loaded from
# source and can be transformed.

# Ensure stale bytecode doesn't bypass the transform.
find "$CPYTHON_DIR" -name '*.pyc' -delete

PYTHONPATH_PREFIX="$CPYTHON_DIR/Lib:$REPO_ROOT:$REPO_ROOT/target/debug"
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
  normalize_expected_to_module() {
    local test_id="$1"
    IFS='.' read -r -a parts <<< "$test_id"
    local idx=0
    local module=""
    if [ "${#parts[@]}" -eq 0 ]; then
      return
    fi
    if [ "${parts[0]}" = "test" ]; then
      idx=1
    fi
    while [ "$idx" -lt "${#parts[@]}" ]; do
      local part="${parts[$idx]}"
      if [[ "$part" == test_* ]]; then
        module="$part"
        idx=$((idx + 1))
        continue
      fi
      break
    done
    if [ -n "$module" ]; then
      printf '%s\n' "$module"
    else
      printf '%s\n' "$test_id"
    fi
  }

  EXPECTED_EXCLUDE_IDS=()
  while IFS= read -r test_id; do
    if [ -n "$test_id" ]; then
      module_id="$(normalize_expected_to_module "$test_id")"
      if [ -n "$module_id" ]; then
        EXPECTED_EXCLUDE_IDS+=("$module_id")
      fi
    fi
  done < <(rg -o '`[^`]+`' "$EXPECTED_FAILURES_FILE" | sed 's/`//g' | rg '^test(\.|_)' | sort -u)
  if [ "${#EXPECTED_EXCLUDE_IDS[@]}" -gt 0 ]; then
    while IFS= read -r module_id; do
      [ -n "$module_id" ] && SKIP_ARGS+=(-x "$module_id")
    done < <(printf '%s\n' "${EXPECTED_EXCLUDE_IDS[@]}" | sort -u)
  fi
fi

# Some environments disallow pseudo-terminals entirely (os.openpty -> EPERM).
# asyncio/test_events has hard PTY requirements and fails noisily in that case.
if ! "$PYTHON_BIN" - <<'PY' >/dev/null 2>&1; then
import os
import sys
try:
    master, slave = os.openpty()
except OSError:
    sys.exit(1)
else:
    os.close(master)
    os.close(slave)
PY
  echo "Skipping test_events: os.openpty is unavailable in this environment" >&2
  SKIP_ARGS+=(-x test_events)
fi

FORWARD_ARGS=()
while [ "$#" -gt 0 ]; do
  case "$1" in
    --memory-limit-mb)
      shift
      if [ "$#" -eq 0 ]; then
        echo "--memory-limit-mb requires a numeric value in MiB" >&2
        exit 2
      fi
      MEMORY_LIMIT_MB="$1"
      ;;
    --memory-limit-mb=*)
      MEMORY_LIMIT_MB="${1#*=}"
      ;;
    *)
      FORWARD_ARGS+=("$1")
      ;;
  esac
  shift
done

if ! [[ "$MEMORY_LIMIT_MB" =~ ^[0-9]+$ ]]; then
  echo "invalid memory limit '$MEMORY_LIMIT_MB' (expected integer MiB)" >&2
  exit 2
fi

(
  cd "$CPYTHON_DIR"

  USE_PRLIMIT=0
  if command -v prlimit >/dev/null 2>&1; then
    USE_PRLIMIT=1
  fi

  if [ "$MEMORY_LIMIT_MB" -gt 0 ]; then
    MEMORY_LIMIT_KB=$((MEMORY_LIMIT_MB * 1024))
    MEMORY_LIMIT_BYTES=$((MEMORY_LIMIT_MB * 1024 * 1024))
    # Best-effort RSS cap (not enforced on all kernels), inherited by workers.
    ulimit -m "$MEMORY_LIMIT_KB" 2>/dev/null || true
    # Per-process virtual memory cap inherited by workers.
    ulimit -v "$MEMORY_LIMIT_KB"
    echo "Applying per-process memory caps: RSS=${MEMORY_LIMIT_MB} MiB (best effort), VM=${MEMORY_LIMIT_MB} MiB" >&2
  fi

  TEST_CMD=(
    "$PYTHON_BIN"
    -m test -j0 -v
    "${SKIP_ARGS[@]}"
    "${FORWARD_ARGS[@]}"
  )
  if [ "$MEMORY_LIMIT_MB" -gt 0 ] && [ "$USE_PRLIMIT" -eq 1 ]; then
    TEST_CMD=(
      prlimit
      --rss="$MEMORY_LIMIT_BYTES:$MEMORY_LIMIT_BYTES"
      --as="$MEMORY_LIMIT_BYTES:$MEMORY_LIMIT_BYTES"
      --
      "${TEST_CMD[@]}"
    )
  fi

  DIET_PYTHON_INSTALL_HOOK=1 \
  DIET_PYTHON_TEST_PATCHES=1 \
  PYTHONDONTWRITEBYTECODE=1 \
  PYTHONPATH="$PYTHONPATH_PREFIX${PYTHONPATH:+:$PYTHONPATH}" \
  "${TEST_CMD[@]}"
)
