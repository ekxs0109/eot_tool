import { resolveRuntimeAssets } from "./assets.js";
import { detectRuntimeSupport } from "./runtime-support.js";
import type {
  ConvertOptions,
  ConvertResult,
  FonttoolBinaryInput,
  LoadedFonttoolRuntime,
  LoadRuntimeOptions,
  RuntimeDecision,
  RuntimeDiagnostics,
  RuntimeStrategy,
  RuntimeSupport
} from "./types.js";

const NOT_IMPLEMENTED_MESSAGE =
  "fonttool-wasm runtime loading is not implemented yet. Task 3 wires the real WASM loader.";

export function resolveRuntimeMode(
  strategy: RuntimeStrategy = "single",
  support: RuntimeSupport = detectRuntimeSupport()
): RuntimeDecision {
  if (strategy === "single") {
    return {
      fallbackReason: "requested-single",
      requestedStrategy: strategy,
      resolvedMode: "single-thread",
      runtimeKind: support.runtimeKind,
      variant: "single"
    };
  }

  if (strategy === "auto") {
    if (support.pthreadsPossible) {
      return {
        requestedStrategy: strategy,
        resolvedMode: "pthreads",
        runtimeKind: support.runtimeKind,
        variant: "pthread"
      };
    }

    return {
      fallbackReason: "pthreads-unavailable",
      requestedStrategy: strategy,
      resolvedMode: "single-thread",
      runtimeKind: support.runtimeKind,
      variant: "single"
    };
  }

  if (!support.pthreadsPossible) {
    throw new Error(
      "fonttool-wasm pthreads mode requires browser SharedArrayBuffer and cross-origin isolation support."
    );
  }

  return {
    requestedStrategy: strategy,
    resolvedMode: "pthreads",
    runtimeKind: support.runtimeKind,
    variant: "pthread"
  };
}

function createPlaceholderRuntime(
  decision: RuntimeDecision,
  options: LoadRuntimeOptions
): LoadedFonttoolRuntime {
  const assets = resolveRuntimeAssets(options.assets);
  const support = options.support ?? detectRuntimeSupport();
  const diagnostics: RuntimeDiagnostics = {
    ...decision,
    effectiveThreads: decision.variant === "pthread" ? 2 : 1,
    requestedThreads: decision.requestedStrategy === "single" ? 1 : 0
  };

  return {
    diagnostics,
    assets,
    support,
    async convert(
      _input: FonttoolBinaryInput,
      options: ConvertOptions
    ): Promise<ConvertResult> {
      throw new Error(
        `${NOT_IMPLEMENTED_MESSAGE} Requested output kind: ${options.outputKind}.`
      );
    },
    dispose(): void {
      // Placeholder runtimes do not own resources yet.
    }
  };
}

export async function loadSingleThreadRuntime(
  options: LoadRuntimeOptions = {}
): Promise<LoadedFonttoolRuntime> {
  return createPlaceholderRuntime(
    resolveRuntimeMode("single", options.support ?? detectRuntimeSupport()),
    options
  );
}

export async function loadPthreadsRuntime(
  options: LoadRuntimeOptions = {}
): Promise<LoadedFonttoolRuntime> {
  const support = options.support ?? detectRuntimeSupport();

  if (!support.pthreadsPossible) {
    throw new Error(
      "fonttool-wasm pthreads mode requires browser SharedArrayBuffer and cross-origin isolation support."
    );
  }

  return createPlaceholderRuntime(resolveRuntimeMode("pthreads", support), {
    ...options,
    support
  });
}

export async function loadRuntime(
  options: LoadRuntimeOptions = {}
): Promise<LoadedFonttoolRuntime> {
  const requestedStrategy = options.strategy ?? "single";
  const support = options.support ?? detectRuntimeSupport();
  const decision = resolveRuntimeMode(requestedStrategy, support);

  if (decision.resolvedMode === "pthreads") {
    return loadPthreadsRuntime({
      ...options,
      support
    });
  }

  return createPlaceholderRuntime(decision, {
    ...options,
    support
  });
}
