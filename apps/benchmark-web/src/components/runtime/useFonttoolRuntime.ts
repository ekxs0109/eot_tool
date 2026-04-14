import { startTransition, useState } from "react";
import type { RuntimeDiagnostics, RuntimeStrategy } from "fonttool-wasm";
import {
  getFonttoolRuntimeSupport,
  warmupFonttoolRuntime
} from "@/lib/fonttool/runtime";

export type RuntimeLoadState =
  | {
      status: "idle";
    }
  | {
      status: "loading";
    }
  | {
      status: "ready";
      diagnostics: RuntimeDiagnostics;
    }
  | {
      status: "error";
      message: string;
    };

const initialSupport = getFonttoolRuntimeSupport();

export function useFonttoolRuntime() {
  const [loadState, setLoadState] = useState<RuntimeLoadState>({
    status: "idle"
  });

  async function warmRuntime(strategy: RuntimeStrategy) {
    setLoadState({ status: "loading" });

    try {
      const diagnostics = await warmupFonttoolRuntime({ strategy });

      startTransition(() => {
        setLoadState({
          status: "ready",
          diagnostics
        });
      });
    } catch (error) {
      const message =
        error instanceof Error ? error.message : "Unknown fonttool runtime error.";

      startTransition(() => {
        setLoadState({
          status: "error",
          message
        });
      });
    }
  }

  return {
    loadState,
    support: initialSupport,
    warmRuntime
  };
}
