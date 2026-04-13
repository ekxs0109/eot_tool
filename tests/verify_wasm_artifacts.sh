#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUILD_DIR="$ROOT_DIR/build"
PACKAGE_DIR="$ROOT_DIR/packages/fonttool-wasm"
STAGED_WASM_DIR="$PACKAGE_DIR/vendor/wasm"

if ! command -v node >/dev/null 2>&1; then
  echo "error: node is required for wasm artifact staging and verification" >&2
  exit 1
fi

required_artifacts=(
  "$BUILD_DIR/fonttool-wasm.js"
  "$BUILD_DIR/fonttool-wasm.wasm"
  "$BUILD_DIR/fonttool-wasm-pthreads.js"
  "$BUILD_DIR/fonttool-wasm-pthreads.wasm"
)

for artifact in "${required_artifacts[@]}"; do
  if [[ ! -f "$artifact" ]]; then
    echo "missing required artifact: ${artifact#$ROOT_DIR/}" >&2
    exit 1
  fi
  echo "found required artifact: ${artifact#$ROOT_DIR/}"
done

node "$PACKAGE_DIR/scripts/stage-wasm.mjs"
node "$PACKAGE_DIR/scripts/check-artifacts.mjs"

staged_required_artifacts=(
  "$STAGED_WASM_DIR/fonttool-wasm.js"
  "$STAGED_WASM_DIR/fonttool-wasm.wasm"
  "$STAGED_WASM_DIR/fonttool-wasm-pthreads.js"
  "$STAGED_WASM_DIR/fonttool-wasm-pthreads.wasm"
)

for artifact in "${staged_required_artifacts[@]}"; do
  if [[ ! -f "$artifact" ]]; then
    echo "missing staged artifact after verification: ${artifact#$ROOT_DIR/}" >&2
    exit 1
  fi
  echo "found staged artifact: ${artifact#$ROOT_DIR/}"
done

shopt -s nullglob
worker_helpers=("$BUILD_DIR"/fonttool-wasm-pthreads*.worker.js)
staged_worker_helpers=("$STAGED_WASM_DIR"/fonttool-wasm-pthreads*.worker.js)
shopt -u nullglob

if (( ${#worker_helpers[@]} > 0 )); then
  for helper in "${worker_helpers[@]}"; do
    echo "found optional pthread worker helper: ${helper#$ROOT_DIR/}"
  done

  if (( ${#worker_helpers[@]} != ${#staged_worker_helpers[@]} )); then
    echo "staged pthread worker helper count did not match build output" >&2
    exit 1
  fi

  for helper in "${staged_worker_helpers[@]}"; do
    echo "found staged pthread worker helper: ${helper#$ROOT_DIR/}"
  done
else
  echo "optional pthread worker helper not emitted by this toolchain"
fi

BUILD_DIR="$BUILD_DIR" node <<'EOF'
const { pathToFileURL } = require('url');
const path = require('path');

async function verifyArtifactMode(buildDir, jsFileName, expectedMode) {
  const moduleUrl = pathToFileURL(path.join(buildDir, jsFileName)).href;
  const createModule = (await import(moduleUrl)).default;
  const moduleInstance = await createModule({
    locateFile(fileName) {
      return path.join(buildDir, fileName);
    },
  });
  const getMode = moduleInstance.cwrap(
      'wasm_runtime_thread_mode', 'string', []);
  const actualMode = getMode();
  if (actualMode !== expectedMode) {
    throw new Error(
        `${jsFileName} reported mode ${actualMode}, expected ${expectedMode}`);
  }
  const getDiagnostics = moduleInstance.cwrap(
      'wasm_runtime_get_diagnostics', 'number', ['number']);
  getDiagnostics(0);
  console.log(`verified runtime mode for ${jsFileName}: ${actualMode}`);
  console.log(`verified diagnostics export for ${jsFileName}`);
}

async function main() {
  const buildDir = process.env.BUILD_DIR;
  if (!buildDir) {
    throw new Error('BUILD_DIR is required');
  }

  await verifyArtifactMode(buildDir, 'fonttool-wasm.js', 'single-thread');
  await verifyArtifactMode(buildDir, 'fonttool-wasm-pthreads.js', 'pthreads');
}

main().catch((error) => {
  console.error(error.message || error);
  process.exit(1);
});
EOF

echo "PASS: wasm artifacts verified"
