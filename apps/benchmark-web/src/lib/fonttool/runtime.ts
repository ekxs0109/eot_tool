import {
  detectRuntimeSupport,
  loadFonttool,
  type LoadRuntimeOptions,
  type RuntimeDiagnostics,
  type RuntimeAssets,
  type RuntimeSupport
} from "fonttool-wasm";

const browserRuntimeAssets: RuntimeAssets = {
  single: {
    jsUrl: "/node_modules/fonttool-wasm/vendor/wasm/fonttool-wasm.js",
    wasmUrl: "/node_modules/fonttool-wasm/vendor/wasm/fonttool-wasm.wasm"
  },
  pthreads: {
    jsUrl: "/node_modules/fonttool-wasm/vendor/wasm/fonttool-wasm-pthreads.js",
    wasmUrl: "/node_modules/fonttool-wasm/vendor/wasm/fonttool-wasm-pthreads.wasm"
  }
};

export function getFonttoolRuntimeSupport(): RuntimeSupport {
  return detectRuntimeSupport();
}

export async function warmupFonttoolRuntime(
  options: LoadRuntimeOptions = {}
): Promise<RuntimeDiagnostics> {
  const runtime = await loadFonttool({
    ...options,
    assets:
      typeof window === "undefined"
        ? options.assets
        : {
            ...browserRuntimeAssets,
            ...options.assets
          }
  });

  try {
    return runtime.diagnostics;
  } finally {
    await runtime.dispose();
  }
}
