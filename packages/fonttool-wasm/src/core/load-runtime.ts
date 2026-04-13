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

type Cwrap = (
  ident: string,
  returnType: "number" | "string" | "boolean" | null,
  argTypes: Array<"number" | "string" | "array" | "boolean">
) => (...args: unknown[]) => unknown;

type NativeRuntimeModule = {
  HEAPU8: Uint8Array;
  cwrap: Cwrap;
} & Record<StableExportName, NativeFunction>;

type NativeRuntimeExports = Record<string, unknown>;

type RuntimeModuleFactory = (options?: {
  instantiateWasm?: (
    imports: object,
    successCallback: (instance: object, module: object) => void
  ) => object;
  locateFile?: (path: string) => string;
}) => Promise<NativeRuntimeModule>;

type RuntimeArtifactModule = {
  default?: RuntimeModuleFactory;
};

type RuntimeLoadFailure = Error & {
  cause?: unknown;
};

type InstantiatedRuntime = {
  exports: NativeRuntimeExports;
  module: NativeRuntimeModule;
};

type StackExports = {
  stackAlloc: NativeFunction;
  stackRestore: NativeFunction;
  stackSave: NativeFunction;
};

type NativeRuntimeDiagnostics = {
  effectiveThreads: number;
  fallbackReason?: string;
  requestedThreads: number;
  resolvedMode: "single" | "threaded";
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

function resolveAssetUrl(path: string): URL {
  return new URL(path, PACKAGE_ROOT_URL);
}

function resolveVariantAssets(
  assets: RuntimeAssets,
  variant: RuntimeVariant
): RuntimeVariantAssets {
  return variant === "pthread" ? assets.pthreads : assets.single;
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

async function loadRuntimeArtifactModule(
  assets: RuntimeVariantAssets
): Promise<InstantiatedRuntime> {
  const jsUrl = resolveAssetUrl(assets.jsUrl);
  const wasmUrl = resolveAssetUrl(assets.wasmUrl);
  const workerUrl =
    assets.workerUrl !== undefined ? resolveAssetUrl(assets.workerUrl) : undefined;
  const wasmApi = getWasmApi();
  const wasmBinary = await loadBinary(wasmUrl);
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

function withStack<T>(
  stack: StackExports,
  fn: () => T
): T {
  const pointer = stack.stackSave();

  try {
    return fn();
  } finally {
    stack.stackRestore(pointer);
  }
}

function withStackBuffer<T>(
  module: NativeRuntimeModule,
  stack: StackExports,
  bytes: Uint8Array,
  fn: (pointer: number) => T
): T {
  return withStack(stack, () => {
    const pointer = stack.stackAlloc(bytes.byteLength);
    getHeap(module).set(bytes, pointer);
    return fn(pointer);
  });
}

function writeCString(
  module: NativeRuntimeModule,
  stack: StackExports,
  value: string
): number {
  const encoded = TEXT_ENCODER.encode(value);
  const pointer = stack.stackAlloc(encoded.byteLength + 1);
  const heap = getHeap(module);

  heap.set(encoded, pointer);
  heap[pointer + encoded.byteLength] = 0;
  return pointer;
}

function throwForStatus(status: number, operation: string): never {
  const label = EOT_STATUS_LABELS.get(status) ?? "EOT_ERR_UNKNOWN";
  throw new Error(
    `fonttool-wasm ${operation} failed with ${label} (${status}).`
  );
}

function getStackExports(
  exports: NativeRuntimeExports,
  variant: RuntimeVariant
): StackExports {
  const exportNames = variant === "pthread"
    ? { stackAlloc: "P", stackRestore: "O", stackSave: "Q" }
    : { stackAlloc: "p", stackRestore: "o", stackSave: "q" };

  const stackAlloc = exports[exportNames.stackAlloc];
  const stackRestore = exports[exportNames.stackRestore];
  const stackSave = exports[exportNames.stackSave];

  if (
    typeof stackAlloc !== "function" ||
    typeof stackRestore !== "function" ||
    typeof stackSave !== "function"
  ) {
    throw new Error("fonttool-wasm runtime is missing stack helper exports.");
  }

  return {
    stackAlloc: stackAlloc as NativeFunction,
    stackRestore: stackRestore as NativeFunction,
    stackSave: stackSave as NativeFunction
  };
}

function readNativeDiagnostics(
  module: NativeRuntimeModule,
  stack: StackExports
): NativeRuntimeDiagnostics {
  const runtimeDiagnostics = getNativeFunction(
    module,
    STABLE_EXPORT_NAMES.runtimeDiagnostics
  );

  return withStack(stack, () => {
    const diagnosticsPointer = stack.stackAlloc(WASM_RUNTIME_DIAGNOSTICS_STRUCT_SIZE);
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

    const diagnostics: NativeRuntimeDiagnostics = {
      requestedThreads: readUint32(module, diagnosticsPointer),
      effectiveThreads: readUint32(module, diagnosticsPointer + 4),
      resolvedMode: readCString(
        module,
        readUint32(module, diagnosticsPointer + 8)
      ) as "single" | "threaded"
    };
    const fallbackReason = readCString(
      module,
      readUint32(module, diagnosticsPointer + 12)
    );

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
  instantiatedRuntime: InstantiatedRuntime
): LoadedFonttoolRuntime {
  let module: NativeRuntimeModule | null = instantiatedRuntime.module;
  let stack: StackExports | null = getStackExports(
    instantiatedRuntime.exports,
    decision.variant
  );
  const diagnostics: RuntimeDiagnostics = {
    requestedStrategy: decision.requestedStrategy,
    resolvedMode: "single",
    runtimeKind: decision.runtimeKind,
    variant: decision.variant,
    requestedThreads: 0,
    effectiveThreads: 0
  };

  applyNativeDiagnostics(
    diagnostics,
    decision,
    readNativeDiagnostics(module, stack)
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
      if (module === null || stack === null) {
        throw new Error("fonttool-wasm runtime resources are unavailable.");
      }
      const activeModule = module;
      const activeStack = stack;

      const binaryInput = asUint8Array(input);
      const convert = getNativeFunction(activeModule, STABLE_EXPORT_NAMES.convert);
      const bufferDestroy = getNativeFunction(
        activeModule,
        STABLE_EXPORT_NAMES.bufferDestroy
      );

      return withStackBuffer(activeModule, activeStack, binaryInput, (inputPointer) =>
        withStack(activeStack, () => {
          const outputKindPointer = writeCString(
            activeModule,
            activeStack,
            options.outputKind
          );
          const variationAxesPointer = writeCString(
            activeModule,
            activeStack,
            options.variationAxes ?? ""
          );
          const outputPointer = activeStack.stackAlloc(WASM_BUFFER_STRUCT_SIZE);
          getHeap(activeModule).fill(
            0,
            outputPointer,
            outputPointer + WASM_BUFFER_STRUCT_SIZE
          );

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
            const dataPointer = readUint32(activeModule, outputPointer);
            const dataLength = readUint32(activeModule, outputPointer + 4);
            const output = new Uint8Array(dataLength);
            output.set(
              getHeap(activeModule).subarray(dataPointer, dataPointer + dataLength)
            );

            applyNativeDiagnostics(
              diagnostics,
              decision,
              readNativeDiagnostics(activeModule, activeStack)
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
      );
    },
    dispose(): void {
      disposed = true;
      stack = null;
      module = null;
    }
  };
}

async function loadResolvedRuntime(
  decision: RuntimeDecision,
  assets: RuntimeAssets,
  support: RuntimeSupport
): Promise<LoadedFonttoolRuntime> {
  try {
    const runtime = await loadRuntimeArtifactModule(
      resolveVariantAssets(assets, decision.variant)
    );
    return createLoadedRuntime(decision, assets, support, runtime);
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
