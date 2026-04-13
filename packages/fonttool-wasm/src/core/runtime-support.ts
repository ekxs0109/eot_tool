import type { RuntimeSupport } from "./types.js";

type RuntimeGlobals = typeof globalThis & {
  SharedArrayBuffer?: unknown;
  crossOriginIsolated?: boolean;
  process?: {
    versions?: {
      node?: string;
    };
  };
  window?: unknown;
  document?: unknown;
};

export function detectRuntimeSupport(): RuntimeSupport {
  const runtime = globalThis as RuntimeGlobals;
  const isNode = typeof runtime.process?.versions?.node === "string";
  const isBrowser =
    typeof runtime.window !== "undefined" &&
    typeof runtime.document !== "undefined";
  const sharedArrayBuffer =
    typeof runtime.SharedArrayBuffer === "function";
  const crossOriginIsolated = runtime.crossOriginIsolated === true;
  const pthreadsPossible =
    isBrowser && !isNode && sharedArrayBuffer && crossOriginIsolated;

  return {
    runtimeKind: isNode ? "node" : "browser",
    sharedArrayBuffer,
    crossOriginIsolated,
    pthreadsPossible
  };
}
