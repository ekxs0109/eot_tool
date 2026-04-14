import type { RuntimeStrategy, RuntimeSupport } from "fonttool-wasm";
import {
  formatCapability,
  formatFallbackReason
} from "@/lib/formatting/runtime";
import type { RuntimeLoadState } from "@/components/runtime/useFonttoolRuntime";

type RuntimePanelProps = {
  loadState: RuntimeLoadState;
  support: RuntimeSupport;
  onWarmRuntime: (strategy: RuntimeStrategy) => void;
};

export function RuntimePanel({
  loadState,
  support,
  onWarmRuntime
}: RuntimePanelProps) {
  return (
    <section className="panel-card" aria-labelledby="runtime-title">
      <div className="panel-header">
        <p className="eyebrow">Runtime boundary</p>
        <h2 id="runtime-title">Workspace runtime status</h2>
      </div>

      <dl className="stat-grid">
        <div className="stat-tile">
          <dt>Runtime kind</dt>
          <dd>{support.runtimeKind}</dd>
        </div>

        <div className="stat-tile">
          <dt>SharedArrayBuffer</dt>
          <dd>{formatCapability(support.sharedArrayBuffer)}</dd>
        </div>

        <div className="stat-tile">
          <dt>Cross-origin isolated</dt>
          <dd>{formatCapability(support.crossOriginIsolated)}</dd>
        </div>

        <div className="stat-tile">
          <dt>Pthreads possible</dt>
          <dd>{formatCapability(support.pthreadsPossible)}</dd>
        </div>
      </dl>

      <div className="runtime-actions">
        <button
          className="primary-action"
          type="button"
          onClick={() => {
            onWarmRuntime("single");
          }}
          disabled={loadState.status === "loading"}
        >
          {loadState.status === "loading" ? "Loading runtime..." : "Warm single-thread runtime"}
        </button>
      </div>

      <div className="runtime-status">
        <p className="status-label">Load status</p>

        {loadState.status === "idle" ? (
          <p>Runtime has not been loaded yet. Use the warmup action to verify the package boundary.</p>
        ) : null}

        {loadState.status === "loading" ? (
          <p>Loading the workspace `fonttool-wasm` runtime bundle.</p>
        ) : null}

        {loadState.status === "ready" ? (
          <dl className="diagnostic-list">
            <div>
              <dt>Resolved mode</dt>
              <dd>{loadState.diagnostics.resolvedMode}</dd>
            </div>

            <div>
              <dt>Requested threads</dt>
              <dd>{String(loadState.diagnostics.requestedThreads)}</dd>
            </div>

            <div>
              <dt>Effective threads</dt>
              <dd>{String(loadState.diagnostics.effectiveThreads)}</dd>
            </div>

            <div>
              <dt>Fallback</dt>
              <dd>{formatFallbackReason(loadState.diagnostics.fallbackReason)}</dd>
            </div>
          </dl>
        ) : null}

        {loadState.status === "error" ? <p className="status-error">{loadState.message}</p> : null}
      </div>
    </section>
  );
}
