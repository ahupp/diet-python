#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

LOOPS="${1:-1000000}"

echo "date: $(date +%F)"
echo "loops: ${LOOPS}"

cargo build --release

echo "jit transformed"
./vendor/cpython/python -m diet_import_hook.exec pystone "${LOOPS}"

echo "stock cpython"
./vendor/cpython/python -S pystone.py "${LOOPS}"
