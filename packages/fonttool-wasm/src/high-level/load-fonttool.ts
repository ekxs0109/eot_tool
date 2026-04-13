import { loadRuntime } from "../core/load-runtime.js";
import type { LoadedFonttoolRuntime, LoadRuntimeOptions } from "../core/types.js";

export async function loadFonttool(
  options: LoadRuntimeOptions = {}
): Promise<LoadedFonttoolRuntime> {
  return loadRuntime(options);
}
