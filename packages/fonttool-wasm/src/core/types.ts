export type RuntimeStrategy = "single" | "pthreads" | "auto";

export type ResolvedMode = "single" | "threaded";
export type RuntimeDecisionMode = "single-thread" | "pthreads";
export type RuntimeKind = "node" | "browser";
export type RuntimeVariant = "single" | "pthread";

export type OutputKind = "eot" | "fntdata";

export type FonttoolBinaryInput = ArrayBuffer | ArrayBufferView;

export interface RuntimeSupport {
  runtimeKind: RuntimeKind;
  sharedArrayBuffer: boolean;
  crossOriginIsolated: boolean;
  pthreadsPossible: boolean;
}

export interface RuntimeVariantAssets {
  jsUrl: string;
  wasmUrl: string;
  workerUrl?: string;
}

export interface RuntimeAssets {
  single: RuntimeVariantAssets;
  pthreads: RuntimeVariantAssets;
}

export interface LoadRuntimeOptions {
  strategy?: RuntimeStrategy;
  assets?: Partial<RuntimeAssets>;
  support?: RuntimeSupport;
}

export interface RuntimeDecision {
  requestedStrategy: RuntimeStrategy;
  resolvedMode: RuntimeDecisionMode;
  runtimeKind: RuntimeKind;
  variant: RuntimeVariant;
  fallbackReason?: string;
}

export interface RuntimeDiagnostics {
  requestedStrategy: RuntimeStrategy;
  resolvedMode: ResolvedMode;
  runtimeKind: RuntimeKind;
  variant: RuntimeVariant;
  fallbackReason?: string;
  requestedThreads: number;
  effectiveThreads: number;
}

export interface ConvertOptions extends LoadRuntimeOptions {
  outputKind: OutputKind;
  variationAxes?: string;
}

export interface ConvertResult {
  diagnostics: RuntimeDiagnostics;
  outputKind: OutputKind;
  data: Uint8Array;
}

export interface LoadedFonttoolRuntime {
  readonly diagnostics: RuntimeDiagnostics;
  readonly assets: RuntimeAssets;
  readonly support: RuntimeSupport;
  convert(
    input: FonttoolBinaryInput,
    options: ConvertOptions
  ): Promise<ConvertResult>;
  dispose(): void | Promise<void>;
}
