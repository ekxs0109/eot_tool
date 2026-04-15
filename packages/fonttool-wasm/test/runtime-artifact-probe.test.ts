import { describe, expect, it } from "vitest";

import { probeRuntimeArtifact } from "../src/core/runtime-artifact-probe.js";
import type { RuntimeSupport } from "../src/core/types.js";

const nodeThreadSupport: RuntimeSupport = {
  runtimeKind: "node",
  sharedArrayBuffer: true,
  crossOriginIsolated: false,
  pthreadsPossible: true
};

describe.sequential("probeRuntimeArtifact", () => {
  it("reports the staged single-thread runtime through the high-level loader contract", async () => {
    const probe = await probeRuntimeArtifact({
      strategy: "single",
      support: nodeThreadSupport
    });

    expect(probe.diagnostics).toMatchObject({
      effectiveThreads: 0,
      fallbackReason: "requested-single",
      requestedStrategy: "single",
      requestedThreads: 0,
      resolvedMode: "single",
      runtimeKind: "node",
      variant: "single"
    });
    expect(probe.support).toEqual(nodeThreadSupport);
  });

  it("reports the staged pthread runtime through the high-level loader contract", async () => {
    const probe = await probeRuntimeArtifact({
      strategy: "auto",
      support: nodeThreadSupport
    });

    expect(probe.diagnostics).toMatchObject({
      effectiveThreads: 0,
      requestedStrategy: "auto",
      requestedThreads: 0,
      resolvedMode: "threaded",
      runtimeKind: "node",
      variant: "pthread"
    });
    expect(probe.diagnostics.fallbackReason).toBeUndefined();
    expect(probe.support).toEqual(nodeThreadSupport);
  });
});
