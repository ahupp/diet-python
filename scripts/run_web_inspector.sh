#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WEB_DIR="$ROOT_DIR/web"
PORT="${PORT:-8000}"
HOST="${HOST:-127.0.0.1}"
URL="http://$HOST:$PORT"

WASM_PACK_MODE="${WASM_PACK_MODE:-no-install}"
OUT_NAME="${OUT_NAME:-diet_python}"
CARGO_HOME="${CARGO_HOME:-/tmp/cargo-home}"

required_wasm_bindgen_version() {
  awk '
    $0 == "name = \"wasm-bindgen\"" { found = 1; next }
    found && $1 == "version" {
      gsub(/"/, "", $3);
      print $3;
      exit
    }
  ' "$ROOT_DIR/Cargo.lock"
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

  root="/tmp/wasm-tools-$required"
  if [ ! -x "$root/bin/wasm-bindgen" ]; then
    echo "Installing wasm-bindgen-cli $required to $root ..."
    CARGO_HOME="$CARGO_HOME" cargo install wasm-bindgen-cli --version "$required" --root "$root"
  fi
  export PATH="$root/bin:$PATH"
}

cd "$ROOT_DIR"

echo "[1/3] Building wasm package..."
ensure_wasm_bindgen
wasm-pack build dp-transform \
  --target web \
  --out-dir ../web/pkg \
  --out-name "$OUT_NAME" \
  --mode "$WASM_PACK_MODE"

echo "[2/3] Starting web server in $WEB_DIR on $URL ..."
cd "$WEB_DIR"
python3 -m http.server "$PORT" --bind "$HOST" >/tmp/diet_python_web_inspector.log 2>&1 &
SERVER_PID=$!

cleanup() {
  if kill -0 "$SERVER_PID" >/dev/null 2>&1; then
    kill "$SERVER_PID" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT INT TERM

sleep 0.5

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
