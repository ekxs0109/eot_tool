import { resolveRuntimeAssets } from "./assets.js";
import { detectRuntimeSupport } from "./runtime-support.js";
import type {
  ConvertOptions,
  ConvertResult,
  FonttoolBinaryInput,
  LoadedFonttoolRuntime,
  LoadRuntimeOptions,
  ResolvedMode,
  RuntimeStrategy,
  RuntimeSupport
} from "./types.js";

const NOT_IMPLEMENTED_MESSAGE =
  "fonttool-wasm runtime loading is not implemented yet. Task 3 wires the real WASM loader.";

export function resolveRuntimeMode(
  strategy: RuntimeStrategy = "single",
  support: RuntimeSupport = detectRuntimeSupport()
): ResolvedMode {
  if (strategy === "single") {
    return "single-thread";
  }

  if (strategy === "auto") {
    return support.pthreadsPossible ? "pthreads" : "single-thread";
  }

  if (!support.pthreadsPossible) {
    throw new Error(
      "fonttool-wasm pthreads mode requires browser SharedArrayBuffer and cross-origin isolation support."
    );
  }

  return "pthreads";
}

function createPlaceholderRuntime(
  requestedStrategy: RuntimeStrategy,
  resolvedMode: ResolvedMode,
  options: LoadRuntimeOptions
): LoadedFonttoolRuntime {
  const assets = resolveRuntimeAssets(options.assets);
  const support = options.support ?? detectRuntimeSupport();

  return {
    requestedStrategy,
    resolvedMode,
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
  return createPlaceholderRuntime("single", "single-thread", options);
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

  return createPlaceholderRuntime("pthreads", "pthreads", {
    ...options,
    support
  });
}

export async function loadRuntime(
  options: LoadRuntimeOptions = {}
): Promise<LoadedFonttoolRuntime> {
  const requestedStrategy = options.strategy ?? "single";
  const support = options.support ?? detectRuntimeSupport();
  const resolvedMode = resolveRuntimeMode(requestedStrategy, support);

  if (resolvedMode === "pthreads") {
    return loadPthreadsRuntime({
      ...options,
      support
    });
  }

  return createPlaceholderRuntime(requestedStrategy, resolvedMode, {
    ...options,
    support
  });
}
