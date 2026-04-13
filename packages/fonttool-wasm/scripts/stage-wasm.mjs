import { access, copyFile, mkdir, readdir, rm } from "node:fs/promises";
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

async function assertExists(filePath) {
  try {
    await access(filePath);
  } catch (error) {
    if (error && error.code === "ENOENT") {
      throw new Error(`missing required build artifact: ${path.relative(repoRoot, filePath)}`);
    }
    throw error;
  }
}

async function copyArtifacts() {
  await mkdir(vendorDir, { recursive: true });

  const workerArtifacts = await listMatchingFiles(
    buildDir,
    OPTIONAL_PTHREAD_WORKER_PATTERN
  );

  const stagedCandidates = [
    ...REQUIRED_ARTIFACTS,
    ...(await listMatchingFiles(vendorDir, OPTIONAL_PTHREAD_WORKER_PATTERN))
  ];

  for (const fileName of stagedCandidates) {
    await rm(path.join(vendorDir, fileName), { force: true });
  }

  for (const fileName of REQUIRED_ARTIFACTS) {
    const sourcePath = path.join(buildDir, fileName);
    await assertExists(sourcePath);
    const targetPath = path.join(vendorDir, fileName);
    await copyFile(sourcePath, targetPath);
    console.log(`staged ${path.relative(repoRoot, targetPath)}`);
  }

  for (const fileName of workerArtifacts) {
    const sourcePath = path.join(buildDir, fileName);
    const targetPath = path.join(vendorDir, fileName);
    await copyFile(sourcePath, targetPath);
    console.log(`staged ${path.relative(repoRoot, targetPath)}`);
  }

  if (workerArtifacts.length === 0) {
    console.log("optional pthread worker helper not emitted by this toolchain");
  }
}

copyArtifacts().catch((error) => {
  console.error(error.message || error);
  process.exit(1);
});
