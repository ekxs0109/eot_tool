import { readFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { loadFonttool } from "../dist/index.js";

const thisDir = dirname(fileURLToPath(import.meta.url));
const sampleFontPath = resolve(thisDir, "../../../testdata/cff-static.otf");

const runtime = await loadFonttool({
  strategy: "auto"
});

try {
  const input = new Uint8Array(await readFile(sampleFontPath));
  const result = await runtime.convert(input, {
    outputKind: "eot",
    strategy: "auto"
  });

  if (result.data.byteLength === 0) {
    throw new Error("fonttool-wasm smoke conversion produced no output.");
  }

  process.stdout.write(
    `${JSON.stringify({
      bytes: result.data.byteLength,
      diagnostics: result.diagnostics
    })}\n`
  );
} finally {
  await runtime.dispose();
}
