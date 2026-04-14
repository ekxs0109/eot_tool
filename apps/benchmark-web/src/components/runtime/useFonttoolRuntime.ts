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
const runtimeStrategySearchParam = "runtime";

function readInitialRuntimeStrategy(): RuntimeStrategy {
  if (typeof window === "undefined") {
    return "single";
  }

  const value = new URLSearchParams(window.location.search).get(
    runtimeStrategySearchParam
  );

  if (value === "single" || value === "auto" || value === "pthreads") {
    return value;
  }

  return "single";
}

function writeRuntimeStrategy(strategy: RuntimeStrategy) {
  if (typeof window === "undefined") {
    return;
  }

  const url = new URL(window.location.href);
  url.searchParams.set(runtimeStrategySearchParam, strategy);
  window.history.replaceState(null, "", url);
}

export function useFonttoolRuntime() {
  const [selectedStrategy, setSelectedStrategy] = useState<RuntimeStrategy>(
    readInitialRuntimeStrategy
  );
  const [loadState, setLoadState] = useState<RuntimeLoadState>({
    status: "idle"
  });

  async function warmRuntime(strategy: RuntimeStrategy) {
    setSelectedStrategy(strategy);
    writeRuntimeStrategy(strategy);
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
    selectedStrategy,
    support: initialSupport,
    warmRuntime
  };
}
