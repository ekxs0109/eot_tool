import { convert } from "./high-level/convert.js";
import { detectRuntimeSupport } from "./high-level/detect-runtime-support.js";
import { loadFonttool } from "./high-level/load-fonttool.js";

export { convert, detectRuntimeSupport, loadFonttool };

export type {
  ConvertOptions,
  ConvertResult,
  LoadedFonttoolRuntime,
  LoadRuntimeOptions,
  OutputKind,
  ResolvedMode,
  RuntimeAssets,
  RuntimeDecision,
  RuntimeDiagnostics,
  RuntimeKind,
  RuntimeStrategy,
  RuntimeSupport,
  RuntimeVariant
} from "./core/types.js";

export default {
  convert,
  detectRuntimeSupport,
  loadFonttool
};
