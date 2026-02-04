#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

echo "date: $(date +%F)"

cargo build --release

echo "tree-walking eval"
./vendor/cpython/python -m diet_import_hook.exec pystone

echo "stock cpython"
./vendor/cpython/python -S pystone.py 1000000

echo "transform-only"
./scripts/python.sh pystone.py 1000000 2> /dev/null
