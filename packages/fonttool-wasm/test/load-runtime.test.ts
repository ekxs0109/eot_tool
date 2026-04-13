import { readFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { describe, expect, it } from "vitest";

import { loadRuntime } from "../src/core/load-runtime.js";
import type { RuntimeSupport } from "../src/core/types.js";

const thisDir = dirname(fileURLToPath(import.meta.url));
const sampleFontPath = resolve(thisDir, "../../../testdata/OpenSans-Regular.ttf");

const nodeThreadSupport: RuntimeSupport = {
  runtimeKind: "node",
  sharedArrayBuffer: true,
  crossOriginIsolated: false,
  pthreadsPossible: true
};

async function loadSampleFont(): Promise<Uint8Array> {
  return new Uint8Array(await readFile(sampleFontPath));
}

describe.sequential("loadRuntime", () => {
  it("loads the staged single-thread runtime and converts a font", async () => {
    const runtime = await loadRuntime({
      strategy: "single",
      support: nodeThreadSupport
    });

    try {
      const result = await runtime.convert(await loadSampleFont(), {
        outputKind: "eot",
        strategy: "single",
        support: nodeThreadSupport
      });

      expect(result.outputKind).toBe("eot");
      expect(result.data.byteLength).toBeGreaterThan(0);
      expect(result.diagnostics).toMatchObject({
        requestedStrategy: "single",
        resolvedMode: "single-thread",
        runtimeKind: "node",
        variant: "single"
      });
      expect(runtime.diagnostics).toMatchObject(result.diagnostics);
      expect(runtime.support).toEqual(nodeThreadSupport);
    } finally {
      await runtime.dispose();
    }
  });

  it("prefers the staged pthread runtime in node auto mode", async () => {
    const runtime = await loadRuntime({
      strategy: "auto",
      support: nodeThreadSupport
    });

    try {
      const result = await runtime.convert(await loadSampleFont(), {
        outputKind: "eot",
        strategy: "auto",
        support: nodeThreadSupport
      });

      expect(result.outputKind).toBe("eot");
      expect(result.data.byteLength).toBeGreaterThan(0);
      expect(result.diagnostics).toMatchObject({
        requestedStrategy: "auto",
        resolvedMode: "pthreads",
        runtimeKind: "node",
        variant: "pthread"
      });
      expect(result.diagnostics.requestedThreads).toBeGreaterThan(0);
      expect(runtime.diagnostics).toMatchObject(result.diagnostics);
    } finally {
      await runtime.dispose();
    }
  });

  it("falls back to the staged single-thread runtime when node pthread loading fails", async () => {
    const runtime = await loadRuntime({
      strategy: "auto",
      support: nodeThreadSupport,
      assets: {
        pthreads: {
          jsUrl: "./vendor/wasm/missing-fonttool-wasm-pthreads.js"
        }
      }
    });

    try {
      const result = await runtime.convert(await loadSampleFont(), {
        outputKind: "eot",
        strategy: "auto",
        support: nodeThreadSupport
      });

      expect(result.outputKind).toBe("eot");
      expect(result.data.byteLength).toBeGreaterThan(0);
      expect(result.diagnostics).toMatchObject({
        fallbackReason: "pthreads-load-failed",
        requestedStrategy: "auto",
        resolvedMode: "single-thread",
        runtimeKind: "node",
        variant: "single"
      });
      expect(runtime.diagnostics).toMatchObject(result.diagnostics);
    } finally {
      await runtime.dispose();
    }
  });
});
