import { loadFonttool } from "./load-fonttool.js";
import { detectRuntimeSupport } from "./detect-runtime-support.js";
import type {
  ConvertOptions,
  ConvertResult,
  FonttoolBinaryInput
} from "../core/types.js";

export async function convert(
  input: FonttoolBinaryInput,
  options: ConvertOptions
): Promise<ConvertResult> {
  const runtime = await loadFonttool({
    ...options,
    support: options.support ?? detectRuntimeSupport()
  });

  try {
    return await runtime.convert(input, options);
  } finally {
    await runtime.dispose();
  }
}
