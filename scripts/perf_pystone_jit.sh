#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

mkdir -p logs

LOOPS="${1:-100000}"
OUTPUT_PREFIX="${2:-logs/pystone_jit_perf}"
PERF_FREQUENCY="${PERF_FREQUENCY:-999}"
PERF_CALL_GRAPH="${PERF_CALL_GRAPH:-dwarf,16384}"
PERF_PERCENT_LIMIT="${PERF_PERCENT_LIMIT:-0.5}"

PERF_DATA="${OUTPUT_PREFIX}.data"
RUN_LOG="${OUTPUT_PREFIX}.log"
REPORT_SYMBOLS="${OUTPUT_PREFIX}_report.txt"
REPORT_DSO_SYMBOLS="${OUTPUT_PREFIX}_by_dso_symbol.txt"
PYO3_RELEASE_LIB="$ROOT/target/release/libdiet_python.so"
PYO3_STAGING_DIR="$(mktemp -d)"
PYTHONPATH_PREFIX="${ROOT}:${PYO3_STAGING_DIR}"

cleanup() {
  rm -rf "$PYO3_STAGING_DIR"
}

trap cleanup EXIT

if ! command -v perf >/dev/null 2>&1; then
  echo "perf is required but was not found on PATH" >&2
  exit 1
fi

echo "date: $(date +%F)"
echo "loops: ${LOOPS}"
echo "perf data: ${PERF_DATA}"
echo "run log: ${RUN_LOG}"
echo "report: ${REPORT_SYMBOLS}"
echo "report by dso/symbol: ${REPORT_DSO_SYMBOLS}"

cargo build --release

if [ ! -f "$PYO3_RELEASE_LIB" ]; then
  echo "release extension not found at ${PYO3_RELEASE_LIB}" >&2
  exit 1
fi

ln -sf "$PYO3_RELEASE_LIB" "$PYO3_STAGING_DIR/diet_python.so"

perf record \
  --call-graph "${PERF_CALL_GRAPH}" \
  -F "${PERF_FREQUENCY}" \
  -o "${PERF_DATA}" \
  -- \
  env \
  PYTHONDONTWRITEBYTECODE=1 \
  PYTHONPATH="${PYTHONPATH_PREFIX}${PYTHONPATH:+:${PYTHONPATH}}" \
  ./vendor/cpython/python -m diet_import_hook.exec pystone "${LOOPS}" \
  >"${RUN_LOG}" 2>&1

perf report \
  --stdio \
  --percent-limit "${PERF_PERCENT_LIMIT}" \
  --sort overhead,symbol \
  -i "${PERF_DATA}" \
  >"${REPORT_SYMBOLS}"

perf report \
  --stdio \
  --percent-limit "${PERF_PERCENT_LIMIT}" \
  --sort overhead,dso,symbol \
  -i "${PERF_DATA}" \
  >"${REPORT_DSO_SYMBOLS}"

echo "finished"
