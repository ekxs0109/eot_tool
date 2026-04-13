import { describe, expect, it } from "vitest";

import { resolveRuntimeMode } from "../src/core/load-runtime.js";
import { detectRuntimeSupport as detectCoreRuntimeSupport } from "../src/core/runtime-support.js";
import { detectRuntimeSupport as detectHighLevelRuntimeSupport } from "../src/high-level/detect-runtime-support.js";
import type { RuntimeSupport } from "../src/core/types.js";

const noThreads: RuntimeSupport = {
  runtimeKind: "browser",
  sharedArrayBuffer: false,
  crossOriginIsolated: false,
  pthreadsPossible: false
};

const browserThreads: RuntimeSupport = {
  runtimeKind: "browser",
  sharedArrayBuffer: true,
  crossOriginIsolated: true,
  pthreadsPossible: true
};

describe("resolveRuntimeMode", () => {
  it("returns single-thread diagnostics for single strategy", () => {
    expect(resolveRuntimeMode("single", browserThreads)).toEqual({
      fallbackReason: "requested-single",
      requestedStrategy: "single",
      resolvedMode: "single-thread",
      runtimeKind: "browser",
      variant: "single"
    });
  });

  it("falls back to single-thread diagnostics when auto cannot use pthreads", () => {
    expect(resolveRuntimeMode("auto", noThreads)).toEqual({
      fallbackReason: "pthreads-unavailable",
      requestedStrategy: "auto",
      resolvedMode: "single-thread",
      runtimeKind: "browser",
      variant: "single"
    });
  });

  it("returns pthread diagnostics when auto can use pthreads", () => {
    expect(resolveRuntimeMode("auto", browserThreads)).toEqual({
      fallbackReason: undefined,
      requestedStrategy: "auto",
      resolvedMode: "pthreads",
      runtimeKind: "browser",
      variant: "pthread"
    });
  });
});

describe("detectRuntimeSupport", () => {
  it("returns the same answer from core and high-level entry points", () => {
    expect(detectHighLevelRuntimeSupport()).toEqual(detectCoreRuntimeSupport());
  });
});
