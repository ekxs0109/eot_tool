import { resolveRuntimeAssets } from "./assets.js";
import { detectRuntimeSupport } from "./runtime-support.js";
import type {
  ConvertOptions,
  ConvertResult,
  FonttoolBinaryInput,
  LoadedFonttoolRuntime,
  LoadRuntimeOptions,
  RuntimeAssets,
  RuntimeDecision,
  RuntimeDiagnostics,
  RuntimeStrategy,
  RuntimeSupport,
  RuntimeVariant,
  RuntimeVariantAssets
} from "./types.js";

const PACKAGE_ROOT_URL = new URL("../../", import.meta.url);
const TEXT_DECODER = new TextDecoder();
const TEXT_ENCODER = new TextEncoder();
const WASM_BUFFER_STRUCT_SIZE = 8;
const WASM_RUNTIME_DIAGNOSTICS_STRUCT_SIZE = 16;

const STABLE_EXPORT_NAMES = {
  bufferDestroy: "_wasm_buffer_destroy",
  convert: "_wasm_convert_otf_to_embedded_font",
  free: "_free",
  malloc: "_malloc",
  runtimeDiagnostics: "_wasm_runtime_get_diagnostics",
  threadMode: "_wasm_runtime_thread_mode"
} as const;

const EOT_STATUS_LABELS = new Map<number, string>([
  [0, "EOT_OK"],
  [1, "EOT_ERR_INVALID_ARGUMENT"],
  [2, "EOT_ERR_IO"],
  [3, "EOT_ERR_TRUNCATED"],
  [4, "EOT_ERR_INVALID_MAGIC"],
  [5, "EOT_ERR_ALLOCATION"],
  [6, "EOT_ERR_INVALID_STRING_LENGTH"],
  [7, "EOT_ERR_INVALID_PADDING"],
  [8, "EOT_ERR_INVALID_SIZE_METADATA"],
  [9, "EOT_ERR_CORRUPT_DATA"],
  [10, "EOT_ERR_DECOMPRESS_FAILED"]
]);

type StableExportName =
  (typeof STABLE_EXPORT_NAMES)[keyof typeof STABLE_EXPORT_NAMES];

type NativeFunction = (...args: number[]) => number;

type NativeRuntimeModule = {
  HEAPU8: Uint8Array;
  cwrap: (
    ident: string,
    returnType: "number" | "string" | "boolean" | null,
    argTypes: Array<"number" | "string" | "array" | "boolean">
  ) => (...args: unknown[]) => unknown;
} & Record<StableExportName, NativeFunction>;

type RuntimeModuleFactory = (options?: {
  locateFile?: (path: string) => string;
}) => Promise<NativeRuntimeModule>;

type RuntimeArtifactModule = {
  default?: RuntimeModuleFactory;
};

type RuntimeLoadFailure = Error & {
  cause?: unknown;
};

type NativeRuntimeDiagnostics = {
  requestedThreads: number;
  effectiveThreads: number;
  resolvedMode: "single" | "threaded";
  fallbackReason?: string;
};

function buildDecision(
  strategy: RuntimeStrategy,
  support: RuntimeSupport,
  variant: RuntimeVariant,
  fallbackReason?: string
): RuntimeDecision {
  const decision: RuntimeDecision = {
    requestedStrategy: strategy,
    resolvedMode: variant === "pthread" ? "pthreads" : "single-thread",
    runtimeKind: support.runtimeKind,
    variant
  };

  if (fallbackReason !== undefined) {
    decision.fallbackReason = fallbackReason;
  }

  return decision;
}

export function resolveRuntimeMode(
  strategy: RuntimeStrategy = "single",
  support: RuntimeSupport = detectRuntimeSupport()
): RuntimeDecision {
  if (strategy === "single") {
    return buildDecision(strategy, support, "single", "requested-single");
  }

  if (strategy === "auto") {
    if (support.pthreadsPossible) {
      return buildDecision(strategy, support, "pthread");
    }

    return buildDecision(strategy, support, "single", "pthreads-unavailable");
  }

  if (!support.pthreadsPossible) {
    throw new Error("fonttool-wasm pthreads mode is unavailable in this runtime.");
  }

  return buildDecision(strategy, support, "pthread");
}

function createInitialDiagnostics(decision: RuntimeDecision): RuntimeDiagnostics {
  return {
    requestedStrategy: decision.requestedStrategy,
    resolvedMode: decision.variant === "pthread" ? "threaded" : "single",
    runtimeKind: decision.runtimeKind,
    variant: decision.variant,
    requestedThreads: 0,
    effectiveThreads: 0,
    ...(decision.fallbackReason !== undefined
      ? { fallbackReason: decision.fallbackReason }
      : {})
  };
}

function resolveAssetUrl(path: string): URL {
  return new URL(path, PACKAGE_ROOT_URL);
}

function resolveVariantAssets(
  assets: RuntimeAssets,
  variant: RuntimeVariant
): RuntimeVariantAssets {
  return variant === "pthread" ? assets.pthreads : assets.single;
}

async function loadRuntimeArtifactModule(
  assets: RuntimeVariantAssets
): Promise<NativeRuntimeModule> {
  const jsUrl = resolveAssetUrl(assets.jsUrl);
  const wasmUrl = resolveAssetUrl(assets.wasmUrl);
  const workerUrl =
    assets.workerUrl !== undefined ? resolveAssetUrl(assets.workerUrl) : undefined;
  const importedModule = (await import(jsUrl.href)) as RuntimeArtifactModule;
  const factory = importedModule.default;

  if (typeof factory !== "function") {
    throw new Error(
      `fonttool-wasm runtime module ${jsUrl.href} did not export a default factory.`
    );
  }

  return factory({
    locateFile(path) {
      if (path.endsWith(".wasm")) {
        return wasmUrl.href;
      }

      if (workerUrl !== undefined && path.endsWith(".worker.js")) {
        return workerUrl.href;
      }

      return new URL(path, jsUrl).href;
    }
  });
}

function createRuntimeLoadError(
  variant: RuntimeVariant,
  error: unknown
): RuntimeLoadFailure {
  const failure = new Error(
    `Failed to load fonttool-wasm ${variant} runtime.`
  ) as RuntimeLoadFailure;
  failure.cause = error;
  return failure;
}

function getNativeFunction(
  module: NativeRuntimeModule,
  exportName: StableExportName
): NativeFunction {
  const fn = module[exportName];

  if (typeof fn !== "function") {
    throw new Error(
      `fonttool-wasm runtime is missing required export ${exportName}.`
    );
  }

  return fn;
}

function getHeap(module: NativeRuntimeModule): Uint8Array {
  if (!(module.HEAPU8 instanceof Uint8Array)) {
    throw new Error("fonttool-wasm runtime did not expose HEAPU8.");
  }

  return module.HEAPU8;
}

function getDataView(module: NativeRuntimeModule): DataView {
  const heap = getHeap(module);
  return new DataView(heap.buffer, heap.byteOffset, heap.byteLength);
}

function readUint32(module: NativeRuntimeModule, pointer: number): number {
  return getDataView(module).getUint32(pointer, true);
}

function readCString(module: NativeRuntimeModule, pointer: number): string {
  if (pointer === 0) {
    return "";
  }

  const heap = getHeap(module);
  let end = pointer;
  while (end < heap.length && heap[end] !== 0) {
    end += 1;
  }

  return TEXT_DECODER.decode(heap.subarray(pointer, end));
}

function asUint8Array(input: FonttoolBinaryInput): Uint8Array {
  if (input instanceof Uint8Array) {
    return input;
  }

  if (ArrayBuffer.isView(input)) {
    return new Uint8Array(input.buffer, input.byteOffset, input.byteLength);
  }

  return new Uint8Array(input);
}

function withAllocatedPointer<T>(
  module: NativeRuntimeModule,
  length: number,
  fn: (pointer: number) => T
): T {
  const malloc = getNativeFunction(module, STABLE_EXPORT_NAMES.malloc);
  const free = getNativeFunction(module, STABLE_EXPORT_NAMES.free);
  const pointer = malloc(length);

  if (pointer === 0) {
    throw new Error("fonttool-wasm could not allocate runtime memory.");
  }

  try {
    return fn(pointer);
  } finally {
    free(pointer);
  }
}

function withAllocatedBytes<T>(
  module: NativeRuntimeModule,
  bytes: Uint8Array,
  fn: (pointer: number) => T
): T {
  return withAllocatedPointer(module, bytes.byteLength, (pointer) => {
    getHeap(module).set(bytes, pointer);
    return fn(pointer);
  });
}

function withCString<T>(
  module: NativeRuntimeModule,
  value: string,
  fn: (pointer: number) => T
): T {
  const encoded = TEXT_ENCODER.encode(value);
  return withAllocatedBytes(
    module,
    Uint8Array.from([...encoded, 0]),
    fn
  );
}

function throwForStatus(status: number, operation: string): never {
  const label = EOT_STATUS_LABELS.get(status) ?? "EOT_ERR_UNKNOWN";
  throw new Error(
    `fonttool-wasm ${operation} failed with ${label} (${status}).`
  );
}

function readNativeDiagnostics(
  module: NativeRuntimeModule
): NativeRuntimeDiagnostics {
  const runtimeDiagnostics = getNativeFunction(
    module,
    STABLE_EXPORT_NAMES.runtimeDiagnostics
  );

  return withAllocatedPointer(module, WASM_RUNTIME_DIAGNOSTICS_STRUCT_SIZE, (pointer) => {
    getHeap(module).fill(0, pointer, pointer + WASM_RUNTIME_DIAGNOSTICS_STRUCT_SIZE);

    const status = runtimeDiagnostics(pointer);
    if (status !== 0) {
      throwForStatus(status, "runtime diagnostics");
    }

    const diagnostics: NativeRuntimeDiagnostics = {
      requestedThreads: readUint32(module, pointer),
      effectiveThreads: readUint32(module, pointer + 4),
      resolvedMode: readCString(module, readUint32(module, pointer + 8)) as
        | "single"
        | "threaded"
    };
    const fallbackReason = readCString(module, readUint32(module, pointer + 12));

    if (fallbackReason !== "") {
      diagnostics.fallbackReason = fallbackReason;
    }

    return diagnostics;
  });
}

function applyNativeDiagnostics(
  diagnostics: RuntimeDiagnostics,
  decision: RuntimeDecision,
  nativeDiagnostics: NativeRuntimeDiagnostics
): void {
  diagnostics.requestedStrategy = decision.requestedStrategy;
  diagnostics.runtimeKind = decision.runtimeKind;
  diagnostics.variant = decision.variant;
  diagnostics.requestedThreads = nativeDiagnostics.requestedThreads;
  diagnostics.effectiveThreads = nativeDiagnostics.effectiveThreads;
  diagnostics.resolvedMode = nativeDiagnostics.resolvedMode;

  if (decision.fallbackReason !== undefined) {
    diagnostics.fallbackReason = decision.fallbackReason;
  } else if (nativeDiagnostics.fallbackReason !== undefined) {
    diagnostics.fallbackReason = nativeDiagnostics.fallbackReason;
  } else {
    delete diagnostics.fallbackReason;
  }
}

function createLoadedRuntime(
  decision: RuntimeDecision,
  assets: RuntimeAssets,
  support: RuntimeSupport,
  module: NativeRuntimeModule
): LoadedFonttoolRuntime {
  const diagnostics = createInitialDiagnostics(decision);
  let disposed = false;

  return {
    diagnostics,
    assets,
    support,
    async convert(
      input: FonttoolBinaryInput,
      options: ConvertOptions
    ): Promise<ConvertResult> {
      if (disposed) {
        throw new Error("fonttool-wasm runtime has already been disposed.");
      }

      const convert = getNativeFunction(module, STABLE_EXPORT_NAMES.convert);
      const bufferDestroy = getNativeFunction(
        module,
        STABLE_EXPORT_NAMES.bufferDestroy
      );
      const binaryInput = asUint8Array(input);

      return withAllocatedBytes(module, binaryInput, (inputPointer) =>
        withCString(module, options.outputKind, (outputKindPointer) =>
          withCString(module, options.variationAxes ?? "", (variationAxesPointer) =>
            withAllocatedPointer(module, WASM_BUFFER_STRUCT_SIZE, (outputPointer) => {
              getHeap(module).fill(0, outputPointer, outputPointer + WASM_BUFFER_STRUCT_SIZE);

              const status = convert(
                inputPointer,
                binaryInput.byteLength,
                outputKindPointer,
                variationAxesPointer,
                outputPointer
              );
              if (status !== 0) {
                throwForStatus(status, "convert");
              }

              try {
                const dataPointer = readUint32(module, outputPointer);
                const dataLength = readUint32(module, outputPointer + 4);
                const output = new Uint8Array(dataLength);
                output.set(getHeap(module).subarray(dataPointer, dataPointer + dataLength));

                applyNativeDiagnostics(
                  diagnostics,
                  decision,
                  readNativeDiagnostics(module)
                );

                return {
                  diagnostics: { ...diagnostics },
                  outputKind: options.outputKind,
                  data: output
                };
              } finally {
                bufferDestroy(outputPointer);
              }
            })
          )
        )
      );
    },
    dispose(): void {
      disposed = true;
    }
  };
}

async function loadResolvedRuntime(
  decision: RuntimeDecision,
  assets: RuntimeAssets,
  support: RuntimeSupport
): Promise<LoadedFonttoolRuntime> {
  try {
    const module = await loadRuntimeArtifactModule(
      resolveVariantAssets(assets, decision.variant)
    );
    return createLoadedRuntime(decision, assets, support, module);
  } catch (error) {
    throw createRuntimeLoadError(decision.variant, error);
  }
}

export async function loadSingleThreadRuntime(
  options: LoadRuntimeOptions = {}
): Promise<LoadedFonttoolRuntime> {
  const support = options.support ?? detectRuntimeSupport();
  const assets = resolveRuntimeAssets(options.assets);
  return loadResolvedRuntime(resolveRuntimeMode("single", support), assets, support);
}

export async function loadPthreadsRuntime(
  options: LoadRuntimeOptions = {}
): Promise<LoadedFonttoolRuntime> {
  const support = options.support ?? detectRuntimeSupport();
  const assets = resolveRuntimeAssets(options.assets);
  return loadResolvedRuntime(resolveRuntimeMode("pthreads", support), assets, support);
}

export async function loadRuntime(
  options: LoadRuntimeOptions = {}
): Promise<LoadedFonttoolRuntime> {
  const support = options.support ?? detectRuntimeSupport();
  const assets = resolveRuntimeAssets(options.assets);
  const strategy = options.strategy ?? "single";
  const decision = resolveRuntimeMode(strategy, support);

  if (decision.variant !== "pthread") {
    return loadResolvedRuntime(decision, assets, support);
  }

  try {
    return await loadResolvedRuntime(decision, assets, support);
  } catch (error) {
    if (strategy !== "auto") {
      throw error;
    }

    return loadResolvedRuntime(
      buildDecision("auto", support, "single", "pthreads-load-failed"),
      assets,
      support
    );
  }
}
