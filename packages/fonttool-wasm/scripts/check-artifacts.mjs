import { readdir } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const packageDir = path.resolve(scriptDir, "..");
const vendorDir = path.join(packageDir, "vendor", "wasm");

const REQUIRED_ARTIFACTS = [
  "fonttool-wasm.js",
  "fonttool-wasm.wasm",
  "fonttool-wasm-pthreads.js",
  "fonttool-wasm-pthreads.wasm"
];

const OPTIONAL_PTHREAD_WORKER_PATTERN = /^fonttool-wasm-pthreads.*\.worker\.js$/;

async function listFiles(dir) {
  try {
    const entries = await readdir(dir, { withFileTypes: true });
    return entries.filter((entry) => entry.isFile()).map((entry) => entry.name).sort();
  } catch (error) {
    if (error && error.code === "ENOENT") {
      return [];
    }
    throw error;
  }
}

function requireFiles(files, required, label) {
  for (const fileName of required) {
    if (!files.includes(fileName)) {
      throw new Error(`missing required ${label} artifact: ${fileName}`);
    }
    console.log(`found required ${label} artifact: ${fileName}`);
  }
}

async function main() {
  const vendorFiles = await listFiles(vendorDir);

  requireFiles(vendorFiles, REQUIRED_ARTIFACTS, "vendor");

  const vendorWorkers = vendorFiles.filter((fileName) =>
    OPTIONAL_PTHREAD_WORKER_PATTERN.test(fileName)
  );

  if (vendorWorkers.length === 0) {
    console.log("optional pthread worker helper not emitted by this toolchain");
    return;
  }

  for (const fileName of vendorWorkers) {
    console.log(`found vendor pthread worker helper: ${fileName}`);
  }
}

main().catch((error) => {
  console.error(error.message || error);
  process.exit(1);
});
