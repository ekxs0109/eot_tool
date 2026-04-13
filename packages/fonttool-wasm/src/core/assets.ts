import type { RuntimeAssets } from "./types.js";

const DEFAULT_RUNTIME_ASSETS: RuntimeAssets = {
  single: {
    jsUrl: "./vendor/wasm/fonttool-wasm.js",
    wasmUrl: "./vendor/wasm/fonttool-wasm.wasm"
  },
  pthreads: {
    jsUrl: "./vendor/wasm/fonttool-wasm-pthreads.js",
    wasmUrl: "./vendor/wasm/fonttool-wasm-pthreads.wasm"
  }
};

export function getDefaultRuntimeAssets(): RuntimeAssets {
  return {
    single: { ...DEFAULT_RUNTIME_ASSETS.single },
    pthreads: { ...DEFAULT_RUNTIME_ASSETS.pthreads }
  };
}

export function resolveRuntimeAssets(
  overrides?: Partial<RuntimeAssets>
): RuntimeAssets {
  const defaults = getDefaultRuntimeAssets();

  return {
    single: {
      ...defaults.single,
      ...(overrides?.single ?? {})
    },
    pthreads: {
      ...defaults.pthreads,
      ...(overrides?.pthreads ?? {})
    }
  };
}
