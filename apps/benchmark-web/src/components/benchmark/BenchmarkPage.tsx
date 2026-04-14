import { RuntimePanel } from "@/components/runtime/RuntimePanel";
import { useFonttoolRuntime } from "@/components/runtime/useFonttoolRuntime";
import {
  benchmarkScaffoldSummary,
  scaffoldBenchmarkScenarios
} from "@/lib/benchmark/scenarios";

export function BenchmarkPage() {
  const { loadState, support, warmRuntime } = useFonttoolRuntime();

  return (
    <main className="page-shell">
      <section className="hero-card">
        <p className="eyebrow">Benchmark scaffold</p>
        <h1>Benchmark the browser runtime without mixing loader code into the UI.</h1>
        <p className="hero-copy">{benchmarkScaffoldSummary}</p>
      </section>

      <section className="content-grid">
        <RuntimePanel
          loadState={loadState}
          support={support}
          onWarmRuntime={() => {
            void warmRuntime("single");
          }}
        />

        <section className="panel-card" aria-labelledby="benchmark-scenarios-title">
          <div className="panel-header">
            <p className="eyebrow">Planned suites</p>
            <h2 id="benchmark-scenarios-title">Scaffolded benchmark surfaces</h2>
          </div>

          <div className="scenario-list">
            {scaffoldBenchmarkScenarios.map((scenario) => (
              <article className="scenario-card" key={scenario.id}>
                <div className="scenario-header">
                  <h3>{scenario.name}</h3>
                  <span className="scenario-badge">{scenario.expectedOutput}</span>
                </div>

                <p>{scenario.description}</p>

                <ul className="scenario-points">
                  {scenario.focusAreas.map((area) => (
                    <li key={area}>{area}</li>
                  ))}
                </ul>
              </article>
            ))}
          </div>
        </section>
      </section>
    </main>
  );
}
