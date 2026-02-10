#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SET_GLOB="${CPYTHON_TEST_SETS_GLOB:-$REPO_ROOT/test_sets/*.txt}"
MODE="${DIET_PYTHON_MODE:-eval}"
TIMEOUT_SECS="${CPYTHON_TEST_TIMEOUT_SECS:-180}"
TEMPDIR_PATH="${CPYTHON_TEST_TEMPDIR:-/tmp/diet-python-cpython-tests}"
LOG_DIR="${CPYTHON_TEST_LOG_DIR:-$REPO_ROOT/logs}"

usage() {
  cat <<'USAGE'
Usage: ./scripts/run_cpython_test_sets.sh [--mode eval|transform] [--timeout <seconds>] [--tempdir <path>]

Runs each test set file in test_sets/ sequentially (part_01 -> part_10).
The wrapper forces single-process regrtest execution via DIET_PYTHON_TEST_JOBS=1.
USAGE
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --mode)
      shift
      [ "$#" -gt 0 ] || { echo "--mode requires a value" >&2; exit 2; }
      MODE="$1"
      ;;
    --mode=*)
      MODE="${1#*=}"
      ;;
    --timeout)
      shift
      [ "$#" -gt 0 ] || { echo "--timeout requires a value" >&2; exit 2; }
      TIMEOUT_SECS="$1"
      ;;
    --timeout=*)
      TIMEOUT_SECS="${1#*=}"
      ;;
    --tempdir)
      shift
      [ "$#" -gt 0 ] || { echo "--tempdir requires a value" >&2; exit 2; }
      TEMPDIR_PATH="$1"
      ;;
    --tempdir=*)
      TEMPDIR_PATH="${1#*=}"
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage
      exit 2
      ;;
  esac
  shift
done

if [[ "$MODE" != "eval" && "$MODE" != "transform" ]]; then
  echo "invalid mode '$MODE' (expected eval|transform)" >&2
  exit 2
fi
if ! [[ "$TIMEOUT_SECS" =~ ^[0-9]+$ ]]; then
  echo "invalid timeout '$TIMEOUT_SECS' (expected integer seconds)" >&2
  exit 2
fi

mkdir -p "$LOG_DIR" "$TEMPDIR_PATH"

# Interrupted runs can leave stale workers behind.
pkill -f "test.libregrtest.worker" >/dev/null 2>&1 || true

SUMMARY_LOG="$LOG_DIR/cpython_${MODE}_test_sets_summary.log"
: > "$SUMMARY_LOG"
echo "mode=$MODE timeout=$TIMEOUT_SECS tempdir=$TEMPDIR_PATH jobs=1" | tee -a "$SUMMARY_LOG"

failed=0
shopt -s nullglob
set_files=( $SET_GLOB )
shopt -u nullglob
mapfile -t set_files < <(printf '%s\n' "${set_files[@]}" | sort)
if [ "${#set_files[@]}" -eq 0 ]; then
  echo "no test set files found matching $SET_GLOB" >&2
  exit 2
fi

for set_file in "${set_files[@]}"; do
  abs_set="$(realpath "$set_file")"
  set_name="$(basename "$set_file" .txt)"
  set_log="$LOG_DIR/cpython_${MODE}_${set_name}.log"

  echo "=== RUN $abs_set ===" | tee -a "$SUMMARY_LOG"
  set +e
  DIET_PYTHON_MODE="$MODE" \
  DIET_PYTHON_TEST_JOBS=1 \
  ./scripts/run_cpython_tests.sh \
    -x slow \
    --timeout "$TIMEOUT_SECS" \
    --tempdir "$TEMPDIR_PATH" \
    -f "$abs_set" \
    2>&1 | tee "$set_log"
  ec=${PIPESTATUS[0]}
  set -e

  echo "=== EXIT $abs_set : $ec ===" | tee -a "$SUMMARY_LOG"
  if [ "$ec" -ne 0 ]; then
    failed=1
    echo "FAILED_SET=$abs_set EXIT=$ec" | tee -a "$SUMMARY_LOG"
  fi
done

if [ "$failed" -ne 0 ]; then
  echo "One or more sets failed. See $SUMMARY_LOG." >&2
  exit 1
fi

echo "All sets completed successfully. Summary: $SUMMARY_LOG"
