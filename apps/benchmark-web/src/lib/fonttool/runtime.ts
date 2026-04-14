import {
  detectRuntimeSupport,
  loadFonttool,
  type LoadRuntimeOptions,
  type RuntimeDiagnostics,
  type RuntimeSupport
} from "fonttool-wasm";

export function getFonttoolRuntimeSupport(): RuntimeSupport {
  return detectRuntimeSupport();
}

export async function warmupFonttoolRuntime(
  options: LoadRuntimeOptions = {}
): Promise<RuntimeDiagnostics> {
  const runtime = await loadFonttool(options);

  try {
    return runtime.diagnostics;
  } finally {
    await runtime.dispose();
  }
}
