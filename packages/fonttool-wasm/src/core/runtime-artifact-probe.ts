import { loadRuntime } from "./load-runtime.js";
import type {
  LoadRuntimeOptions,
  RuntimeDiagnostics,
  RuntimeSupport
} from "./types.js";

export interface RuntimeArtifactProbeResult {
  diagnostics: RuntimeDiagnostics;
  support: RuntimeSupport;
}

export async function probeRuntimeArtifact(
  options: LoadRuntimeOptions = {}
): Promise<RuntimeArtifactProbeResult> {
  const runtime = await loadRuntime(options);

  try {
    return {
      diagnostics: { ...runtime.diagnostics },
      support: runtime.support
    };
  } finally {
    await runtime.dispose();
  }
}
