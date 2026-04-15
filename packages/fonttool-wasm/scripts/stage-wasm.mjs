import { mkdir, readdir } from "node:fs/promises";
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

async function listMatchingFiles(dir, pattern) {
  try {
    const entries = await readdir(dir, { withFileTypes: true });
    return entries
      .filter((entry) => entry.isFile() && pattern.test(entry.name))
      .map((entry) => entry.name)
      .sort();
  } catch (error) {
    if (error && error.code === "ENOENT") {
      return [];
    }
    throw error;
  }
}

async function copyArtifacts() {
  await mkdir(vendorDir, { recursive: true });
  const vendorFiles = await readdir(vendorDir);
  const present = vendorFiles.filter((fileName) => REQUIRED_ARTIFACTS.includes(fileName));
  if (present.length !== REQUIRED_ARTIFACTS.length) {
    const missing = REQUIRED_ARTIFACTS.filter((fileName) => !present.includes(fileName));
    throw new Error(
      `missing required vendored WASM artifact(s): ${missing.join(", ")}`
    );
  }

  for (const fileName of REQUIRED_ARTIFACTS) {
    console.log(`using vendored artifact ${path.join("packages/fonttool-wasm/vendor/wasm", fileName)}`);
  }

  const workerArtifacts = await listMatchingFiles(vendorDir, OPTIONAL_PTHREAD_WORKER_PATTERN);
  if (workerArtifacts.length === 0) {
    console.log("optional pthread worker helper not emitted by this toolchain");
    return;
  }

  for (const fileName of workerArtifacts) {
    console.log(
      `using vendored pthread worker helper ${path.join("packages/fonttool-wasm/vendor/wasm", fileName)}`
    );
  }
}

copyArtifacts().catch((error) => {
  console.error(error.message || error);
  process.exit(1);
});
