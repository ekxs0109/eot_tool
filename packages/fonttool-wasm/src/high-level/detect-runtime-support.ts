import { detectRuntimeSupport as detectCoreRuntimeSupport } from "../core/runtime-support.js";
import type { RuntimeSupport } from "../core/types.js";

export function detectRuntimeSupport(): RuntimeSupport {
  const support = detectCoreRuntimeSupport();

  if (support.runtimeKind !== "node") {
    return support;
  }

  return {
    ...support,
    pthreadsPossible: support.sharedArrayBuffer
  };
}
