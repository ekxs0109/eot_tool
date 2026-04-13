import { readFile, readdir } from "node:fs/promises";
import { createHash } from "node:crypto";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const packageDir = path.resolve(scriptDir, "..");
const repoRoot = path.resolve(packageDir, "..", "..");
const buildDir = path.join(repoRoot, "build");
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

async function sha256(filePath) {
  const content = await readFile(filePath);
  return createHash("sha256").update(content).digest("hex");
}

function requireFiles(files, required, label) {
  for (const fileName of required) {
    if (!files.includes(fileName)) {
      throw new Error(`missing required ${label} artifact: ${fileName}`);
    }
    console.log(`found required ${label} artifact: ${fileName}`);
  }
}

async function requireMatchingFile(fileName) {
  const buildPath = path.join(buildDir, fileName);
  const stagedPath = path.join(vendorDir, fileName);
  const [buildHash, stagedHash] = await Promise.all([
    sha256(buildPath),
    sha256(stagedPath)
  ]);

  if (buildHash !== stagedHash) {
    throw new Error(
      `staged artifact drift detected for ${fileName}: build=${buildHash} staged=${stagedHash}`
    );
  }

  console.log(`verified staged artifact matches build output: ${fileName}`);
}

async function main() {
  const buildFiles = await listFiles(buildDir);
  const stagedFiles = await listFiles(vendorDir);

  requireFiles(buildFiles, REQUIRED_ARTIFACTS, "build");
  requireFiles(stagedFiles, REQUIRED_ARTIFACTS, "staged");

  for (const fileName of REQUIRED_ARTIFACTS) {
    await requireMatchingFile(fileName);
  }

  const buildWorkers = buildFiles.filter((fileName) =>
    OPTIONAL_PTHREAD_WORKER_PATTERN.test(fileName)
  );
  const stagedWorkers = stagedFiles.filter((fileName) =>
    OPTIONAL_PTHREAD_WORKER_PATTERN.test(fileName)
  );

  if (buildWorkers.length === 0) {
    if (stagedWorkers.length > 0) {
      throw new Error(
        `staged unexpected pthread worker helpers: ${stagedWorkers.join(", ")}`
      );
    }
    console.log("optional pthread worker helper not emitted by this toolchain");
    return;
  }

  for (const fileName of buildWorkers) {
    console.log(`found optional build pthread worker helper: ${fileName}`);
  }

  const missingWorkers = buildWorkers.filter(
    (fileName) => !stagedWorkers.includes(fileName)
  );
  if (missingWorkers.length > 0) {
    throw new Error(
      `missing staged pthread worker helper(s): ${missingWorkers.join(", ")}`
    );
  }

  const unexpectedWorkers = stagedWorkers.filter(
    (fileName) => !buildWorkers.includes(fileName)
  );
  if (unexpectedWorkers.length > 0) {
    throw new Error(
      `staged unexpected pthread worker helper(s): ${unexpectedWorkers.join(", ")}`
    );
  }

  for (const fileName of stagedWorkers) {
    console.log(`found staged pthread worker helper: ${fileName}`);
  }

  for (const fileName of buildWorkers) {
    await requireMatchingFile(fileName);
  }
}

main().catch((error) => {
  console.error(error.message || error);
  process.exit(1);
});
