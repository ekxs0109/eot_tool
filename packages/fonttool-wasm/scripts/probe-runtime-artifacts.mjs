import { probeRuntimeArtifact } from "../dist/core/runtime-artifact-probe.js";

const nodeThreadSupport = {
  runtimeKind: "node",
  sharedArrayBuffer: typeof SharedArrayBuffer === "function",
  crossOriginIsolated: false,
  pthreadsPossible: typeof SharedArrayBuffer === "function"
};

async function verifyProbe(strategy, expected) {
  const probe = await probeRuntimeArtifact({
    strategy,
    support: nodeThreadSupport
  });

  const { diagnostics } = probe;
  if (diagnostics.variant !== expected.variant) {
    throw new Error(
      `${strategy} probe resolved variant ${diagnostics.variant}, expected ${expected.variant}`
    );
  }

  if (diagnostics.resolvedMode !== expected.resolvedMode) {
    throw new Error(
      `${strategy} probe resolved mode ${diagnostics.resolvedMode}, expected ${expected.resolvedMode}`
    );
  }

  if ((diagnostics.fallbackReason ?? null) !== (expected.fallbackReason ?? null)) {
    throw new Error(
      `${strategy} probe fallback ${diagnostics.fallbackReason ?? "(none)"}, expected ${expected.fallbackReason ?? "(none)"}`
    );
  }

  if (diagnostics.requestedThreads !== 0 || diagnostics.effectiveThreads !== 0) {
    throw new Error(
      `${strategy} probe expected idle diagnostics, got requested=${diagnostics.requestedThreads} effective=${diagnostics.effectiveThreads}`
    );
  }

  console.log(
    `verified staged runtime probe for ${strategy}: variant=${diagnostics.variant} mode=${diagnostics.resolvedMode}`
  );
}

await verifyProbe("single", {
  variant: "single",
  resolvedMode: "single",
  fallbackReason: "requested-single"
});

await verifyProbe("auto", {
  variant: "pthread",
  resolvedMode: "threaded"
});
