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
  RuntimeSupport,
  RuntimeVariant,
  RuntimeVariantAssets
} from "./types.js";

const PACKAGE_ROOT_URL = new URL("../../", import.meta.url);
const TEXT_DECODER = new TextDecoder();
const TEXT_ENCODER = new TextEncoder();
const INPUT_SCRATCH_PADDING = 64;
const INPUT_STACK_GUARD_BYTES = 4096;
const WASM_BUFFER_STRUCT_SIZE = 8;
const WASM_RUNTIME_DIAGNOSTICS_STRUCT_SIZE = 16;

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

const SINGLE_EXPORTS = {
  bufferDestroy: "m",
  convert: "n",
  stackAlloc: "p",
  stackRestore: "o",
  stackSave: "q",
  threadMode: "k",
  runtimeDiagnostics: "l"
} as const satisfies ExportMap;

const PTHREAD_EXPORTS = {
  bufferDestroy: "B",
  convert: "C",
  stackAlloc: "P",
  stackRestore: "O",
  stackSave: "Q",
  threadMode: "z",
  runtimeDiagnostics: "A"
} as const satisfies ExportMap;

type ExportMap = {
  readonly bufferDestroy: string;
  readonly convert: string;
  readonly stackAlloc: string;
  readonly stackRestore: string;
  readonly stackSave: string;
  readonly threadMode: string;
  readonly runtimeDiagnostics: string;
};

type NativeFunction = (...args: number[]) => number;

type NativeRuntimeExports = Record<string, unknown>;

type NativeRuntimeModule = {
  HEAPU8: Uint8Array;
};

type NativeRuntimeDiagnostics = {
  effectiveThreads: number;
  fallbackReason?: string;
  requestedThreads: number;
  threadMode: string;
};

type InstantiatedRuntime = {
  exports: NativeRuntimeExports;
  module: NativeRuntimeModule;
};

type RuntimeModuleFactory = (options?: {
  instantiateWasm?: (
    imports: object,
    successCallback: (
      instance: object,
      module: object
    ) => void
  ) => object;
  locateFile?: (path: string) => string;
}) => Promise<NativeRuntimeModule>;

type RuntimeArtifactModule = {
  default?: RuntimeModuleFactory;
};

type RuntimeLoadFailure = Error & {
  cause?: unknown;
};

type WasmApi = {
  Instance: new (
    module: object,
    imports?: object
  ) => {
    exports: object;
  };
  compile(source: Uint8Array): Promise<object>;
};

function getWasmApi(): WasmApi {
  const api = (globalThis as typeof globalThis & {
    WebAssembly?: WasmApi;
  }).WebAssembly;

  if (api === undefined) {
    throw new Error("fonttool-wasm requires WebAssembly support.");
  }

  return api;
}

function resolveSupport(support: RuntimeSupport = detectRuntimeSupport()): RuntimeSupport {
  if (support.runtimeKind !== "node") {
    return support;
  }

  return {
    ...support,
    pthreadsPossible: support.sharedArrayBuffer
  };
}

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
  support: RuntimeSupport = resolveSupport()
): RuntimeDecision {
  const normalizedSupport = resolveSupport(support);

  if (strategy === "single") {
    return buildDecision(strategy, normalizedSupport, "single", "requested-single");
  }

  if (strategy === "auto") {
    if (normalizedSupport.pthreadsPossible) {
      return buildDecision(strategy, normalizedSupport, "pthread");
    }

    return buildDecision(
      strategy,
      normalizedSupport,
      "single",
      "pthreads-unavailable"
    );
  }

  if (!normalizedSupport.pthreadsPossible) {
    throw new Error(
      "fonttool-wasm pthreads mode is unavailable in this runtime."
    );
  }

  return buildDecision(strategy, normalizedSupport, "pthread");
}

function resolveAssetUrl(path: string): URL {
  return new URL(path, PACKAGE_ROOT_URL);
}

async function loadBinary(url: URL): Promise<Uint8Array> {
  if (url.protocol === "file:") {
    const { readFile } = await import("node:fs/promises");
    return readFile(url);
  }

  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(
      `Failed to load fonttool-wasm artifact ${url.href}: ${response.status} ${response.statusText}`
    );
  }

  return new Uint8Array(await response.arrayBuffer());
}

function getExportMap(variant: RuntimeVariant): ExportMap {
  return variant === "pthread" ? PTHREAD_EXPORTS : SINGLE_EXPORTS;
}

function getNativeFunction(
  exports: NativeRuntimeExports,
  name: string
): NativeFunction {
  const value = exports[name];

  if (typeof value !== "function") {
    throw new Error(
      `fonttool-wasm runtime is missing required export ${name}.`
    );
  }

  return value as unknown as NativeFunction;
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

function writeCString(
  module: NativeRuntimeModule,
  stackAlloc: NativeFunction,
  value: string
): number {
  const encoded = TEXT_ENCODER.encode(value);
  const pointer = stackAlloc(encoded.byteLength + 1);
  const heap = getHeap(module);

  heap.set(encoded, pointer);
  heap[pointer + encoded.byteLength] = 0;

  return pointer;
}

function alignDown(value: number, alignment: number): number {
  return value - (value % alignment);
}

function reserveInputPointer(
  module: NativeRuntimeModule,
  stackPointer: number,
  inputLength: number
): number {
  const heap = getHeap(module);
  const alignedLength = alignDown(inputLength + 15, 16);
  const pointer = alignDown(
    heap.byteLength - alignedLength - INPUT_SCRATCH_PADDING,
    16
  );

  if (pointer <= stackPointer + INPUT_STACK_GUARD_BYTES) {
    throw new Error(
      "fonttool-wasm input is too large for the current runtime memory layout."
    );
  }

  return pointer;
}

function throwForStatus(status: number, operation: string): never {
  const label = EOT_STATUS_LABELS.get(status) ?? "EOT_ERR_UNKNOWN";
  throw new Error(
    `fonttool-wasm ${operation} failed with ${label} (${status}).`
  );
}

function withStack<T>(
  module: NativeRuntimeModule,
  stackSave: NativeFunction,
  stackRestore: NativeFunction,
  fn: () => T
): T {
  const stackPointer = stackSave();

  try {
    return fn();
  } finally {
    stackRestore(stackPointer);
  }
}

async function instantiateRuntimeVariant(
  variant: RuntimeVariant,
  assets: RuntimeVariantAssets
): Promise<InstantiatedRuntime> {
  const jsUrl = resolveAssetUrl(assets.jsUrl);
  const wasmUrl = resolveAssetUrl(assets.wasmUrl);
  const workerUrl =
    assets.workerUrl !== undefined ? resolveAssetUrl(assets.workerUrl) : undefined;
  const wasmBinary = await loadBinary(wasmUrl);
  const wasmApi = getWasmApi();
  const compiledWasm = await wasmApi.compile(wasmBinary);
  const importedModule = (await import(jsUrl.href)) as RuntimeArtifactModule;
  const factory = importedModule.default;

  if (typeof factory !== "function") {
    throw new Error(
      `fonttool-wasm runtime module ${jsUrl.href} did not export a default factory.`
    );
  }

  let capturedExports: NativeRuntimeExports | undefined;

  const module = await factory({
    instantiateWasm(imports, successCallback) {
      const instance = new wasmApi.Instance(compiledWasm, imports);
      capturedExports = instance.exports as NativeRuntimeExports;
      successCallback(instance, compiledWasm);
      return instance.exports;
    },
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

  if (capturedExports === undefined) {
    throw new Error(
      `fonttool-wasm runtime module ${jsUrl.href} did not instantiate WebAssembly exports.`
    );
  }

  return {
    exports: capturedExports,
    module
  };
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

function readNativeDiagnostics(
  module: NativeRuntimeModule,
  exports: NativeRuntimeExports,
  exportMap: ExportMap
): NativeRuntimeDiagnostics {
  const stackAlloc = getNativeFunction(exports, exportMap.stackAlloc);
  const stackRestore = getNativeFunction(exports, exportMap.stackRestore);
  const stackSave = getNativeFunction(exports, exportMap.stackSave);
  const runtimeDiagnostics = getNativeFunction(exports, exportMap.runtimeDiagnostics);
  const threadMode = getNativeFunction(exports, exportMap.threadMode);

  return withStack(module, stackSave, stackRestore, () => {
    const diagnosticsPointer = stackAlloc(WASM_RUNTIME_DIAGNOSTICS_STRUCT_SIZE);
    const heap = getHeap(module);

    heap.fill(
      0,
      diagnosticsPointer,
      diagnosticsPointer + WASM_RUNTIME_DIAGNOSTICS_STRUCT_SIZE
    );

    const status = runtimeDiagnostics(diagnosticsPointer);
    if (status !== 0) {
      throwForStatus(status, "runtime diagnostics");
    }

    const fallbackReason = readCString(module, readUint32(module, diagnosticsPointer + 12));
    const nativeThreadMode = readCString(module, threadMode());

    const diagnostics: NativeRuntimeDiagnostics = {
      effectiveThreads: readUint32(module, diagnosticsPointer + 4),
      requestedThreads: readUint32(module, diagnosticsPointer),
      threadMode: nativeThreadMode
    };

    if (fallbackReason !== "") {
      diagnostics.fallbackReason = fallbackReason;
    }

    return diagnostics;
  });
}

function applyDiagnostics(
  diagnostics: RuntimeDiagnostics,
  decision: RuntimeDecision,
  nativeDiagnostics: NativeRuntimeDiagnostics
): void {
  diagnostics.requestedThreads = nativeDiagnostics.requestedThreads;
  diagnostics.effectiveThreads = nativeDiagnostics.effectiveThreads;
  diagnostics.resolvedMode = decision.resolvedMode;
  diagnostics.runtimeKind = decision.runtimeKind;
  diagnostics.variant = decision.variant;
  diagnostics.requestedStrategy = decision.requestedStrategy;

  const fallbackReason = decision.fallbackReason ?? nativeDiagnostics.fallbackReason;
  if (fallbackReason !== undefined) {
    diagnostics.fallbackReason = fallbackReason;
  } else {
    delete diagnostics.fallbackReason;
  }
}

function createLoadedRuntime(
  decision: RuntimeDecision,
  assets: ReturnType<typeof resolveRuntimeAssets>,
  support: RuntimeSupport,
  instantiated: InstantiatedRuntime
): LoadedFonttoolRuntime {
  const exportMap = getExportMap(decision.variant);
  const diagnostics: RuntimeDiagnostics = {
    ...decision,
    effectiveThreads: 0,
    requestedThreads: 0
  };

  applyDiagnostics(
    diagnostics,
    decision,
    readNativeDiagnostics(instantiated.module, instantiated.exports, exportMap)
  );

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

      const stackAlloc = getNativeFunction(instantiated.exports, exportMap.stackAlloc);
      const stackRestore = getNativeFunction(
        instantiated.exports,
        exportMap.stackRestore
      );
      const stackSave = getNativeFunction(instantiated.exports, exportMap.stackSave);
      const convert = getNativeFunction(instantiated.exports, exportMap.convert);
      const bufferDestroy = getNativeFunction(
        instantiated.exports,
        exportMap.bufferDestroy
      );
      const binaryInput = asUint8Array(input);

      const result = withStack(
        instantiated.module,
        stackSave,
        stackRestore,
        (): ConvertResult => {
          const stackPointer = stackSave();
          const inputPointer = reserveInputPointer(
            instantiated.module,
            stackPointer,
            binaryInput.byteLength
          );
          const heap = getHeap(instantiated.module);

          heap.set(binaryInput, inputPointer);

          const outputKindPointer = writeCString(
            instantiated.module,
            stackAlloc,
            options.outputKind
          );
          const variationAxesPointer = writeCString(
            instantiated.module,
            stackAlloc,
            options.variationAxes ?? ""
          );
          const outputPointer = stackAlloc(WASM_BUFFER_STRUCT_SIZE);

          heap.fill(0, outputPointer, outputPointer + WASM_BUFFER_STRUCT_SIZE);

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
            const dataPointer = readUint32(instantiated.module, outputPointer);
            const dataLength = readUint32(instantiated.module, outputPointer + 4);
            const output = new Uint8Array(dataLength);

            output.set(
              getHeap(instantiated.module).subarray(
                dataPointer,
                dataPointer + dataLength
              )
            );

            const nativeDiagnostics = readNativeDiagnostics(
              instantiated.module,
              instantiated.exports,
              exportMap
            );

            applyDiagnostics(diagnostics, decision, nativeDiagnostics);

            return {
              data: output,
              diagnostics: {
                ...diagnostics
              },
              outputKind: options.outputKind
            };
          } finally {
            bufferDestroy(outputPointer);
          }
        }
      );

      return result;
    },
    dispose(): void {
      disposed = true;
    }
  };
}

async function loadResolvedRuntime(
  decision: RuntimeDecision,
  assets: ReturnType<typeof resolveRuntimeAssets>,
  support: RuntimeSupport
): Promise<LoadedFonttoolRuntime> {
  try {
    const instantiated = await instantiateRuntimeVariant(
      decision.variant,
      assets[decision.variant === "pthread" ? "pthreads" : "single"]
    );

    return createLoadedRuntime(decision, assets, support, instantiated);
  } catch (error) {
    throw createRuntimeLoadError(decision.variant, error);
  }
}

export async function loadSingleThreadRuntime(
  options: LoadRuntimeOptions = {}
): Promise<LoadedFonttoolRuntime> {
  const support = resolveSupport(options.support);
  const assets = resolveRuntimeAssets(options.assets);
  const decision = resolveRuntimeMode("single", support);

  return loadResolvedRuntime(decision, assets, support);
}

export async function loadPthreadsRuntime(
  options: LoadRuntimeOptions = {}
): Promise<LoadedFonttoolRuntime> {
  const support = resolveSupport(options.support);
  const assets = resolveRuntimeAssets(options.assets);
  const decision = resolveRuntimeMode("pthreads", support);

  return loadResolvedRuntime(decision, assets, support);
}

export async function loadRuntime(
  options: LoadRuntimeOptions = {}
): Promise<LoadedFonttoolRuntime> {
  const support = resolveSupport(options.support);
  const assets = resolveRuntimeAssets(options.assets);
  const requestedStrategy = options.strategy ?? "single";
  const decision = resolveRuntimeMode(requestedStrategy, support);

  if (decision.variant !== "pthread") {
    return loadResolvedRuntime(decision, assets, support);
  }

  try {
    return await loadResolvedRuntime(decision, assets, support);
  } catch (error) {
    if (requestedStrategy !== "auto") {
      throw error;
    }

    return loadResolvedRuntime(
      buildDecision("auto", support, "single", "pthreads-load-failed"),
      assets,
      support
    );
  }
}
