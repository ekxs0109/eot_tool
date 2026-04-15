#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PACKAGE_DIR="$ROOT_DIR/packages/fonttool-wasm"
STAGED_WASM_DIR="$PACKAGE_DIR/vendor/wasm"

if ! command -v node >/dev/null 2>&1; then
  echo "error: node is required for wasm artifact staging and verification" >&2
  exit 1
fi

required_artifacts=(
  "$STAGED_WASM_DIR/fonttool-wasm.js"
  "$STAGED_WASM_DIR/fonttool-wasm.wasm"
  "$STAGED_WASM_DIR/fonttool-wasm-pthreads.js"
  "$STAGED_WASM_DIR/fonttool-wasm-pthreads.wasm"
)

for artifact in "${required_artifacts[@]}"; do
  if [[ ! -f "$artifact" ]]; then
    echo "missing required artifact: ${artifact#$ROOT_DIR/}" >&2
    exit 1
  fi
  echo "found required vendored artifact: ${artifact#$ROOT_DIR/}"
done

node "$PACKAGE_DIR/scripts/stage-wasm.mjs"
node "$PACKAGE_DIR/scripts/check-artifacts.mjs"
pnpm --filter fonttool-wasm build

shopt -s nullglob
staged_worker_helpers=("$STAGED_WASM_DIR"/fonttool-wasm-pthreads*.worker.js)
shopt -u nullglob

if (( ${#staged_worker_helpers[@]} > 0 )); then
  for helper in "${staged_worker_helpers[@]}"; do
    echo "found vendored pthread worker helper: ${helper#$ROOT_DIR/}"
  done
else
  echo "optional pthread worker helper not emitted by this toolchain"
fi

node "$PACKAGE_DIR/scripts/probe-runtime-artifacts.mjs"

echo "PASS: wasm artifacts verified"
