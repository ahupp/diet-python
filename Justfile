set shell := ["bash", "-euo", "pipefail", "-c"]

repo_root := justfile_directory()
cpython_bin := repo_root + "/vendor/cpython/python"
venv_dir := repo_root + "/.venv"
uv_cache_dir := repo_root + "/.uv-cache"
cargo_home := env_var_or_default("CARGO_HOME", repo_root + "/tmp/cargo-home")
out_name := env_var_or_default("OUT_NAME", "diet_python")
wasm_pack_mode := env_var_or_default("WASM_PACK_MODE", "no-install")
pyo3_python := cpython_bin
web_dir := repo_root + "/web"
port := env_var_or_default("PORT", "8000")
host := env_var_or_default("HOST", "127.0.0.1")
log_file := env_var_or_default("LOG_FILE", "/tmp/diet_python_web_inspector.log")
url := "http://" + host + ":" + port
limit_wrapper := repo_root + "/scripts/run_with_limits.sh"

export REPO_ROOT := repo_root
export CPYTHON_BIN := cpython_bin
export VENV_DIR := venv_dir
export UV_CACHE_DIR := uv_cache_dir
export UV_PYTHON := cpython_bin
export PYO3_PYTHON := pyo3_python
export PYO3_PYTHON_REAL := pyo3_python
export CARGO_HOME := cargo_home
export OUT_NAME := out_name
export WASM_PACK_MODE := wasm_pack_mode
export WEB_DIR := web_dir
export PORT := port
export HOST := host
export LOG_FILE := log_file
export URL := url
export LIMIT_WRAPPER := limit_wrapper

[private]
ensure-cpython:
  #!/usr/bin/env bash
  if [[ ! -d "$REPO_ROOT/vendor/cpython" ]]; then
    echo "cpython checkout not found at $REPO_ROOT/vendor/cpython" >&2
    exit 1
  fi
  if [[ ! -x "$CPYTHON_BIN" ]]; then
    echo "python not found in $CPYTHON_BIN" >&2
    exit 1
  fi

update-venv build="debug": ensure-cpython
  #!/usr/bin/env bash
  BUILD="{{build}}"

  if [[ "$BUILD" != "debug" && "$BUILD" != "release" ]]; then
    echo "build must be 'debug' or 'release'" >&2
    exit 2
  fi

  rm -rf "$VENV_DIR"
  uv venv --python "$CPYTHON_BIN" "$VENV_DIR"

  (
    cd "$REPO_ROOT"
    VIRTUAL_ENV="$VENV_DIR" PATH="$VENV_DIR/bin:$PATH" \
      uv sync --group dev --frozen --active
  )

  if [[ "$BUILD" == "release" ]]; then
    BUILD_ARGS=(--release)
  else
    BUILD_ARGS=()
  fi

  (
    cd "$REPO_ROOT"
    cargo build --quiet "${BUILD_ARGS[@]}" -p soac-pyo3
  )

run-cpython-tests jobs="0" *args='': (update-venv "debug")
  #!/usr/bin/env bash
  cd "$REPO_ROOT"

  TEST_JOBS="{{jobs}}"
  if ! [[ "$TEST_JOBS" =~ ^[0-9]+$ ]]; then
    echo "invalid jobs '$TEST_JOBS' (expected non-negative integer)" >&2
    exit 2
  fi

  set -- {{args}}

  export SOURCE_DATE_EPOCH="$(date +%s)"

  # Regrtest must run the vendored CPython interpreter from the source tree so
  # stdlib modules resolve from vendor/cpython/Lib. The venv is only used for
  # bootstrap consistency and to ensure the debug extension build already exists.
  PYTHON_BIN="$CPYTHON_BIN"
  PYTHONPATH_PREFIX="$REPO_ROOT/vendor/cpython/Lib:$REPO_ROOT:$REPO_ROOT/target/debug"
  SKIP_ARGS=()
  while IFS= read -r skip_id; do
    [ -n "$skip_id" ] && SKIP_ARGS+=(-x "$skip_id")
  done < <(
    SKIP_FILE="$REPO_ROOT/cpython_skipped_tests.txt" \
    EXPECTED_FAILURES_FILE="$REPO_ROOT/EXPECTED_FAILURE.md" \
    SKIP_EXPECTED_FAILURES="${SKIP_EXPECTED_FAILURES:-1}" \
    PYTHON_BIN="$PYTHON_BIN" \
    "$REPO_ROOT/scripts/collect_cpython_skip_ids.sh"
  )

  find "$REPO_ROOT/vendor/cpython" -name '*.pyc' -delete

  (
    cd "$REPO_ROOT/vendor/cpython"

    TEST_CMD=(
      "$LIMIT_WRAPPER"
      "$PYTHON_BIN"
      -m test "-j$TEST_JOBS" -v
      "${SKIP_ARGS[@]}"
      "$@"
    )

    DIET_PYTHON_INSTALL_HOOK=1 \
    DIET_PYTHON_TEST_PATCHES=1 \
    PYTHONDONTWRITEBYTECODE=1 \
    PYTHONPATH="$PYTHONPATH_PREFIX${PYTHONPATH:+:$PYTHONPATH}" \
    "${TEST_CMD[@]}"
  )

build-web-inspector:
  #!/usr/bin/env bash
  echo "[1/3] Building wasm package..."

  required_wasm_bindgen_version() {
    awk '
      $0 == "name = \"wasm-bindgen\"" { found = 1; next }
      found && $1 == "version" {
        gsub(/"/, "", $3);
        print $3;
        exit
      }
    ' "$REPO_ROOT/Cargo.lock"
  }

  installed_wasm_bindgen_version() {
    if command -v wasm-bindgen >/dev/null 2>&1; then
      wasm-bindgen --version | awk '{print $2}'
    fi
  }

  ensure_wasm_bindgen() {
    local required installed root
    required="$(required_wasm_bindgen_version)"
    if [ -z "$required" ]; then
      echo "Could not determine required wasm-bindgen version from Cargo.lock" >&2
      exit 1
    fi

    installed="$(installed_wasm_bindgen_version || true)"
    if [ "$installed" = "$required" ]; then
      return
    fi

    root="$CARGO_HOME"
    mkdir -p "$root"
    if [ ! -x "$root/bin/wasm-bindgen" ]; then
      echo "Installing wasm-bindgen-cli $required to $root ..."
      CARGO_HOME="$CARGO_HOME" cargo install wasm-bindgen-cli --version "$required" --root "$root"
    fi
    export PATH="$root/bin:$PATH"
  }

  cd "$REPO_ROOT"
  ensure_wasm_bindgen
  wasm-pack build dp-transform \
    --target web \
    --out-dir ../web/pkg \
    --out-name "$OUT_NAME" \
    --mode "$WASM_PACK_MODE"

run-web-inspector: build-web-inspector ensure-cpython
  #!/usr/bin/env bash
  echo "[2/3] Starting web server in $WEB_DIR on $URL ..."

  if [ ! -x "$CPYTHON_BIN" ]; then
    PYTHON_BIN="$(command -v python3)"
  else
    PYTHON_BIN="$CPYTHON_BIN"
  fi

  cd "$REPO_ROOT"
  HOST="$HOST" PORT="$PORT" "$PYTHON_BIN" web/inspector_server.py >"$LOG_FILE" 2>&1 &
  SERVER_PID=$!

  cleanup() {
    if kill -0 "$SERVER_PID" >/dev/null 2>&1; then
      kill "$SERVER_PID" >/dev/null 2>&1 || true
    fi
  }
  trap cleanup EXIT INT TERM

  sleep 0.5

  if ! kill -0 "$SERVER_PID" >/dev/null 2>&1; then
    echo "Web inspector server exited before startup. Log: $LOG_FILE" >&2
    if [ -f "$LOG_FILE" ]; then
      sed -n '1,160p' "$LOG_FILE" >&2
    fi
    wait "$SERVER_PID"
  fi

  echo "[3/3] Opening browser..."
  if command -v open >/dev/null 2>&1; then
    open "$URL" >/dev/null 2>&1 || true
  elif command -v xdg-open >/dev/null 2>&1; then
    xdg-open "$URL" >/dev/null 2>&1 || true
  else
    echo "No browser opener found. Open this URL manually: $URL"
  fi

  echo "Serving $URL (pid=$SERVER_PID). Press Ctrl+C to stop."
  wait "$SERVER_PID"

perf-pystone-jit-warm loops="500000" output_prefix="logs/pystone_jit_perf_warm": ensure-cpython
  #!/usr/bin/env bash
  mkdir -p logs

  LOOPS="{{loops}}"
  OUTPUT_PREFIX="{{output_prefix}}"
  WARMUP_LOOPS="${WARMUP_LOOPS:-1000}"
  PERF_FREQUENCY="${PERF_FREQUENCY:-999}"
  PERF_CALL_GRAPH="${PERF_CALL_GRAPH:-dwarf,16384}"
  PERF_PERCENT_LIMIT="${PERF_PERCENT_LIMIT:-0.5}"

  PERF_DATA="${OUTPUT_PREFIX}.data"
  RUN_LOG="${OUTPUT_PREFIX}.log"
  REPORT_SYMBOLS="${OUTPUT_PREFIX}_report.txt"
  REPORT_DSO="${OUTPUT_PREFIX}_by_dso.txt"
  REPORT_DSO_SYMBOLS="${OUTPUT_PREFIX}_by_dso_symbol.txt"
  REPORT_CALLGRAPH="${OUTPUT_PREFIX}_callgraph.txt"
  PYO3_RELEASE_LIB="$REPO_ROOT/target/release/libdiet_python.so"
  PYO3_STAGING_DIR="$(mktemp -d)"
  PYTHONPATH_PREFIX="${REPO_ROOT}:${PYO3_STAGING_DIR}"

  cleanup() {
    rm -rf "$PYO3_STAGING_DIR"
  }
  trap cleanup EXIT

  if ! command -v perf >/dev/null 2>&1; then
    echo "perf is required but was not found on PATH" >&2
    exit 1
  fi

  echo "date: $(date +%F)"
  echo "warmup loops: ${WARMUP_LOOPS}"
  echo "profile loops: ${LOOPS}"
  echo "perf data: ${PERF_DATA}"
  echo "run log: ${RUN_LOG}"
  echo "report: ${REPORT_SYMBOLS}"
  echo "report by dso: ${REPORT_DSO}"
  echo "report by dso/symbol: ${REPORT_DSO_SYMBOLS}"
  echo "report callgraph: ${REPORT_CALLGRAPH}"

  cd "$REPO_ROOT"
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
    LOOPS="${LOOPS}" \
    WARMUP_LOOPS="${WARMUP_LOOPS}" \
    PYTHONDONTWRITEBYTECODE=1 \
    PYTHONPATH="${PYTHONPATH_PREFIX}${PYTHONPATH:+:${PYTHONPATH}}" \
    "$CPYTHON_BIN" -c 'import os; from diet_import_hook import install; install(); import pystone; warmup_loops = int(os.environ["WARMUP_LOOPS"]); loops = int(os.environ["LOOPS"]); warmup_loops > 0 and pystone.pystones(warmup_loops); pystone.main(loops)' \
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
    --sort overhead,dso \
    -i "${PERF_DATA}" \
    >"${REPORT_DSO}"

  perf report \
    --stdio \
    --percent-limit "${PERF_PERCENT_LIMIT}" \
    --sort overhead,dso,symbol \
    -i "${PERF_DATA}" \
    >"${REPORT_DSO_SYMBOLS}"

  perf report \
    --stdio \
    --percent-limit "${PERF_PERCENT_LIMIT}" \
    --sort overhead,symbol \
    --children \
    --call-graph graph,0.5,caller \
    -i "${PERF_DATA}" \
    >"${REPORT_CALLGRAPH}"

  echo "finished"

pytest-cpython *args='': (update-venv "debug")
  #!/usr/bin/env bash
  cd "$REPO_ROOT"

  set -- {{args}}
  if [ "$#" -eq 0 ]; then
    "$VENV_DIR/bin/python" -m pytest --help
    exit 0
  fi

  export RUST_LOG="${RUST_LOG:-soac_eval::tree_walk::eval=info}"
  # Repo tests are written around transforming integration modules and the
  # modules they explicitly opt into. Rewriting pytest/stdlib imports here
  # adds noise and teardown-only failures without improving coverage.
  export DIET_PYTHON_INTEGRATION_ONLY="${DIET_PYTHON_INTEGRATION_ONLY:-1}"

  TMP_PYTEST_OUTPUT="$(mktemp -t diet-python-pytest.XXXXXX.log)"
  TEST_START_NS="$(date +%s%N)"
  TEST_CMD=(
    "$LIMIT_WRAPPER"
    "$VENV_DIR/bin/python"
    -m pytest --tb=native
    "$@"
  )

  set +e
  DIET_PYTHON_TIMEOUT_SECS="${DIET_PYTHON_TIMEOUT_SECS:-30}" \
  "${TEST_CMD[@]}" 2>&1 | tee "$TMP_PYTEST_OUTPUT"
  TEST_STATUS=${PIPESTATUS[0]}
  set -e

  TEST_END_NS="$(date +%s%N)"
  "$VENV_DIR/bin/python" -c 'import re, sys; path, start_ns, end_ns = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]); pattern = re.compile(r"soac_jit_precompile .* elapsed_ms=([0-9]+(?:\\.[0-9]+)?)"); compile_ms = sum(float(match.group(1)) for line in open(path, "r", encoding="utf-8", errors="replace") if (match := pattern.search(line))); total_s = (end_ns - start_ns) / 1_000_000_000.0; compile_s = compile_ms / 1000.0; non_compile_s = total_s - compile_s; print(f"[diet-python timing] total_test_s={total_s:.3f} compile_s={compile_s:.3f} non_compile_s={non_compile_s:.3f}")' "$TMP_PYTEST_OUTPUT" "$TEST_START_NS" "$TEST_END_NS"
  rm -f "$TMP_PYTEST_OUTPUT"
  exit "$TEST_STATUS"

benchmark loops="1000000": (update-venv "release")
  #!/usr/bin/env bash
  echo "date: $(date +%F)"
  echo "loops: {{loops}}"

  cd "$REPO_ROOT"

  echo "jit transformed"
  "$VENV_DIR/bin/python" -m diet_import_hook.exec pystone.py "{{loops}}"

  echo "stock cpython"
  "$VENV_DIR/bin/python" pystone.py "{{loops}}"
