import { readFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { describe, expect, it } from "vitest";

import { loadRuntime } from "../src/core/load-runtime.js";
import type { RuntimeSupport } from "../src/core/types.js";
import createSingleModule from "../vendor/wasm/fonttool-wasm.js";
import createPthreadModule from "../vendor/wasm/fonttool-wasm-pthreads.js";

const thisDir = dirname(fileURLToPath(import.meta.url));
const sampleFontPath = resolve(thisDir, "../../../testdata/cff-static.otf");

const nodeThreadSupport: RuntimeSupport = {
  runtimeKind: "node",
  sharedArrayBuffer: true,
  crossOriginIsolated: false,
  pthreadsPossible: true
};

async function loadSampleFont(): Promise<Uint8Array> {
  return new Uint8Array(await readFile(sampleFontPath));
}

async function loadVendorModule(
  createModule: (options?: { locateFile?: (path: string) => string }) => Promise<Record<string, unknown>>,
  jsFileName: string
): Promise<Record<string, unknown>> {
  return createModule({
    locateFile(path) {
      return resolve(thisDir, "../vendor/wasm", path);
    }
  });
}

describe.sequential("loadRuntime", () => {
  it("exposes stable allocator exports on staged modules", async () => {
    const single = await loadVendorModule(createSingleModule, "fonttool-wasm.js");
    const pthread = await loadVendorModule(
      createPthreadModule,
      "fonttool-wasm-pthreads.js"
    );

    for (const runtimeModule of [single, pthread]) {
      expect(runtimeModule).toMatchObject({
        HEAPU8: expect.any(Uint8Array),
        _free: expect.any(Function),
        _malloc: expect.any(Function),
        _wasm_buffer_destroy: expect.any(Function),
        _wasm_convert_otf_to_embedded_font: expect.any(Function),
        _wasm_runtime_get_diagnostics: expect.any(Function),
        _wasm_runtime_thread_mode: expect.any(Function),
        cwrap: expect.any(Function)
      });
    }
  });

  it("loads the staged single-thread runtime and converts a font", async () => {
    const runtime = await loadRuntime({
      strategy: "single",
      support: nodeThreadSupport
    });

    try {
      expect(runtime.diagnostics).toMatchObject({
        effectiveThreads: 0,
        fallbackReason: "requested-single",
        requestedStrategy: "single",
        requestedThreads: 0,
        resolvedMode: "single",
        runtimeKind: "node",
        variant: "single"
      });

      const result = await runtime.convert(await loadSampleFont(), {
        outputKind: "eot",
        strategy: "single",
        support: nodeThreadSupport
      });

      expect(result.outputKind).toBe("eot");
      expect(result.data.byteLength).toBeGreaterThan(0);
      expect(result.diagnostics).toMatchObject({
        requestedStrategy: "single",
        resolvedMode: "single",
        runtimeKind: "node",
        variant: "single"
      });
      expect(result.diagnostics.fallbackReason).toBe("requested-single");
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
      expect(runtime.diagnostics).toMatchObject({
        effectiveThreads: 0,
        requestedStrategy: "auto",
        requestedThreads: 0,
        resolvedMode: "threaded",
        runtimeKind: "node",
        variant: "pthread"
      });
      expect(runtime.diagnostics.fallbackReason).toBeUndefined();

      const result = await runtime.convert(await loadSampleFont(), {
        outputKind: "eot",
        strategy: "auto",
        support: nodeThreadSupport
      });

      expect(result.outputKind).toBe("eot");
      expect(result.data.byteLength).toBeGreaterThan(0);
      expect(result.diagnostics).toMatchObject({
        fallbackReason: "task-count-clamped",
        requestedStrategy: "auto",
        resolvedMode: "single",
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
      expect(runtime.diagnostics).toMatchObject({
        effectiveThreads: 0,
        fallbackReason: "pthreads-load-failed",
        requestedStrategy: "auto",
        requestedThreads: 0,
        resolvedMode: "single",
        runtimeKind: "node",
        variant: "single"
      });

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
        resolvedMode: "single",
        runtimeKind: "node",
        variant: "single"
      });
      expect(runtime.diagnostics).toMatchObject(result.diagnostics);
    } finally {
      await runtime.dispose();
    }
  });
});
