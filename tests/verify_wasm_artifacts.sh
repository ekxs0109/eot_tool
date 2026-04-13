#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUILD_DIR="$ROOT_DIR/build"

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

shopt -s nullglob
worker_helpers=("$BUILD_DIR"/fonttool-wasm-pthreads*.worker.js)
shopt -u nullglob

if (( ${#worker_helpers[@]} > 0 )); then
  for helper in "${worker_helpers[@]}"; do
    echo "found optional pthread worker helper: ${helper#$ROOT_DIR/}"
  done
else
  echo "optional pthread worker helper not emitted by this toolchain"
fi

if command -v node >/dev/null 2>&1; then
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
  console.log(`verified runtime mode for ${jsFileName}: ${actualMode}`);
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
else
  echo "note: node not available; skipping runtime mode metadata verification"
fi

echo "PASS: wasm artifacts verified"
