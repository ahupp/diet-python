#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CPYTHON_DIR="$REPO_ROOT/vendor/cpython"
VENV_DIR="$REPO_ROOT/.venv-cpython"
PYTHON_BIN="$CPYTHON_DIR/python"

export UV_CACHE_DIR="$REPO_ROOT/.uv-cache"
export UV_PYTHON="${UV_PYTHON:-$PYTHON_BIN}"

if [ ! -d "$CPYTHON_DIR" ]; then
  echo "cpython checkout not found at ${CPYTHON_DIR}" >&2
  exit 1
fi
if [ ! -x "$PYTHON_BIN" ]; then
  echo "python not found in ${PYTHON_BIN}" >&2
  exit 1
fi

rm -rf "${VENV_DIR}"
uv venv --python "$PYTHON_BIN" "$VENV_DIR"

(
  cd "$REPO_ROOT" &&
  VIRTUAL_ENV="$VENV_DIR" PATH="$VENV_DIR/bin:$PATH"  \
  uv sync --group dev --no-install-project --frozen --active
)

echo "building soac-pyo3"
(
  cd "$REPO_ROOT" &&
  PYO3_PYTHON_REAL="$("$VENV_DIR/bin/python" - <<'PY'
import os
import sys

print(os.path.realpath(sys.executable))
PY
)" &&
  PYO3_PYTHON="$PYO3_PYTHON_REAL" cargo build --quiet -p soac-pyo3
)


echo "starting tests"
(
  cd "$REPO_ROOT" &&
  if [ "$#" -eq 0 ]; then
    "$VENV_DIR/bin/python" -m pytest --help
  else
    export RUST_LOG="${RUST_LOG:-soac_eval::tree_walk::eval=info}"
    # Repo tests are written around transforming integration modules and the
    # modules they explicitly opt into. Rewriting pytest/stdlib imports here
    # adds noise and teardown-only failures without improving coverage.
    export DIET_PYTHON_INTEGRATION_ONLY="${DIET_PYTHON_INTEGRATION_ONLY:-1}"
    TMP_PYTEST_OUTPUT="$(mktemp -t diet-python-pytest.XXXXXX.log)"
    TEST_START_NS="$(date +%s%N)"
    set +e
    "$VENV_DIR/bin/python" -m pytest --tb=native "$@" 2>&1 | tee "$TMP_PYTEST_OUTPUT"
    TEST_STATUS=${PIPESTATUS[0]}
    set -e
    TEST_END_NS="$(date +%s%N)"
    "$VENV_DIR/bin/python" - "$TMP_PYTEST_OUTPUT" "$TEST_START_NS" "$TEST_END_NS" <<'PY'
import re
import sys

path, start_ns, end_ns = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
pattern = re.compile(r"soac_jit_precompile .* elapsed_ms=([0-9]+(?:\.[0-9]+)?)")
compile_ms = 0.0
with open(path, "r", encoding="utf-8", errors="replace") as f:
    for line in f:
        match = pattern.search(line)
        if match:
            compile_ms += float(match.group(1))

total_s = (end_ns - start_ns) / 1_000_000_000.0
compile_s = compile_ms / 1000.0
non_compile_s = total_s - compile_s
print(
    f"[diet-python timing] total_test_s={total_s:.3f} "
    f"compile_s={compile_s:.3f} non_compile_s={non_compile_s:.3f}"
)
PY
    rm -f "$TMP_PYTEST_OUTPUT"
    exit "$TEST_STATUS"
  fi
)
