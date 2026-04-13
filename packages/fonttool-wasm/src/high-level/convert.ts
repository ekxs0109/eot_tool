import { loadFonttool } from "./load-fonttool.js";
import type {
  ConvertOptions,
  ConvertResult,
  FonttoolBinaryInput
} from "../core/types.js";

export async function convert(
  input: FonttoolBinaryInput,
  options: ConvertOptions
): Promise<ConvertResult> {
  const runtime = await loadFonttool(options);

  try {
    return await runtime.convert(input, options);
  } finally {
    await runtime.dispose();
  }
}
