import { detectRuntimeSupport as detectCoreRuntimeSupport } from "../core/runtime-support.js";
import type { RuntimeSupport } from "../core/types.js";

export function detectRuntimeSupport(): RuntimeSupport {
  return detectCoreRuntimeSupport();
}
