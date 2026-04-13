import { loadRuntime } from "../core/load-runtime.js";
import type { LoadedFonttoolRuntime, LoadRuntimeOptions } from "../core/types.js";
import { detectRuntimeSupport } from "./detect-runtime-support.js";

export async function loadFonttool(
  options: LoadRuntimeOptions = {}
): Promise<LoadedFonttoolRuntime> {
  return loadRuntime({
    ...options,
    support: options.support ?? detectRuntimeSupport()
  });
}
