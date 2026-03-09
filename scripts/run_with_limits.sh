#!/usr/bin/env bash
set -euo pipefail

MEMORY_LIMIT_MB="${DIET_PYTHON_MEMORY_LIMIT_MB:-8192}"
TIMEOUT_SECS="${DIET_PYTHON_TIMEOUT_SECS:-180}"
CPUSET="${DIET_PYTHON_CPUSET:-0-7}"

if ! [[ "$MEMORY_LIMIT_MB" =~ ^[0-9]+$ ]]; then
  echo "invalid memory limit '$MEMORY_LIMIT_MB' (expected integer MiB)" >&2
  exit 2
fi
if ! [[ "$TIMEOUT_SECS" =~ ^[0-9]+$ ]]; then
  echo "invalid timeout '$TIMEOUT_SECS' (expected integer seconds)" >&2
  exit 2
fi

if [ "$#" -eq 0 ]; then
  echo "usage: $0 <command> [args...]" >&2
  exit 2
fi

CMD=("$@")

if [ -n "$CPUSET" ]; then
  if ! command -v taskset >/dev/null 2>&1; then
    echo "taskset not found but DIET_PYTHON_CPUSET='$CPUSET' was set" >&2
    exit 2
  fi
  echo "Restricting build/test execution to CPU set: $CPUSET" >&2
  CMD=(taskset -c "$CPUSET" "${CMD[@]}")
fi

if [ "$MEMORY_LIMIT_MB" -gt 0 ]; then
  MEMORY_LIMIT_KB=$((MEMORY_LIMIT_MB * 1024))
  MEMORY_LIMIT_BYTES=$((MEMORY_LIMIT_MB * 1024 * 1024))
  ulimit -m "$MEMORY_LIMIT_KB" 2>/dev/null || true
  ulimit -v "$MEMORY_LIMIT_KB"
  echo "Applying per-process memory caps: RSS=${MEMORY_LIMIT_MB} MiB (best effort), VM=${MEMORY_LIMIT_MB} MiB" >&2
  if command -v prlimit >/dev/null 2>&1; then
    CMD=(
      prlimit
      --rss="$MEMORY_LIMIT_BYTES:$MEMORY_LIMIT_BYTES"
      --as="$MEMORY_LIMIT_BYTES:$MEMORY_LIMIT_BYTES"
      --
      "${CMD[@]}"
    )
  fi
fi

if [ "$TIMEOUT_SECS" -gt 0 ]; then
  if ! command -v timeout >/dev/null 2>&1; then
    echo "timeout not found but DIET_PYTHON_TIMEOUT_SECS='$TIMEOUT_SECS' was set" >&2
    exit 2
  fi
  echo "Applying wall-clock timeout: ${TIMEOUT_SECS}s" >&2
  CMD=(
    timeout
    --foreground
    --kill-after=10s
    "${TIMEOUT_SECS}s"
    "${CMD[@]}"
  )
fi

exec "${CMD[@]}"
