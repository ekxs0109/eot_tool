export type BenchmarkScenario = {
  id: string;
  name: string;
  expectedOutput: "eot" | "fntdata";
  description: string;
  focusAreas: string[];
};

export const benchmarkScaffoldSummary =
  "This package is intentionally narrow: establish the browser app, prove the workspace runtime can be loaded, and reserve clear seams for the future benchmark harness.";

export const scaffoldBenchmarkScenarios: BenchmarkScenario[] = [
  {
    id: "decode-parity",
    name: "Decode parity checks",
    expectedOutput: "eot",
    description:
      "Load representative fixture fonts, invoke the WASM runtime from a non-UI boundary, and compare metadata before measuring timings.",
    focusAreas: [
      "fixture loading",
      "runtime warmup",
      "result capture"
    ]
  },
  {
    id: "subset-variation",
    name: "Subset and variation sampling",
    expectedOutput: "fntdata",
    description:
      "Exercise the browser-facing path that will eventually benchmark subset and variation-aware conversion without embedding those concerns into components.",
    focusAreas: [
      "scenario definitions",
      "timing hooks",
      "diagnostic reporting"
    ]
  }
];
