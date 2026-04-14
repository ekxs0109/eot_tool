import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { execFile } from "node:child_process";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const packageDir = path.resolve(scriptDir, "..");
const execFileAsync = promisify(execFile);

const FORBIDDEN_PATTERNS = [
  /\.map$/,
  /^vendor\/wasm\/\.gitkeep$/,
  /^apps\//,
  /^test\//,
  /^scripts\//
];

const MAX_UNPACKED_SIZE_BYTES = 1_200_000;

function fail(message) {
  console.error(message);
  process.exit(1);
}

async function main() {
  const tempDir = await mkdtemp(path.join(tmpdir(), "fonttool-wasm-pack-"));
  const packJsonPath = path.join(tempDir, "pack-result.json");

  const { stdout } = await execFileAsync(
    "npm",
    ["pack", "--json"],
    { cwd: packageDir }
  );

  try {
    await writeFile(packJsonPath, stdout, "utf8");

    const raw = await readFile(packJsonPath, "utf8");
    const parsed = JSON.parse(raw);
    const result = Array.isArray(parsed) ? parsed[0] : parsed;

    if (result === undefined || !Array.isArray(result.files)) {
      fail("pack-check: could not read npm pack result metadata.");
    }

    for (const file of result.files) {
      const filePath = file.path;
      if (FORBIDDEN_PATTERNS.some((pattern) => pattern.test(filePath))) {
        fail(`pack-check: forbidden file included in tarball: ${filePath}`);
      }
    }

    if (typeof result.unpackedSize === "number" &&
        result.unpackedSize > MAX_UNPACKED_SIZE_BYTES) {
      fail(
        `pack-check: unpacked tarball too large (${result.unpackedSize} bytes > ${MAX_UNPACKED_SIZE_BYTES} bytes).`
      );
    }

    console.log(
      `pack-check: OK (${result.filename}, unpackedSize=${result.unpackedSize ?? "unknown"})`
    );
  } finally {
    await rm(tempDir, { force: true, recursive: true });
  }
}

main().catch((error) => {
  fail(`pack-check: ${error instanceof Error ? error.message : String(error)}`);
});
