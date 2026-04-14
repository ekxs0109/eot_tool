import type { AppLocale } from "@/lib/formatting/i18n";

type LocalizedText = Record<AppLocale, string>;

export type BenchmarkScenario = {
  id: string;
  name: LocalizedText;
  expectedOutput: "eot" | "fntdata";
  description: LocalizedText;
  focusAreas: LocalizedText[];
};

export const benchmarkScaffoldSummary: LocalizedText = {
  en: "This package is intentionally narrow: establish the browser app, prove the workspace runtime can be loaded, and reserve clear seams for the future benchmark harness.",
  zh: "当前页面先聚焦在一件事：把浏览器端应用与运行时边界搭起来，证明运行时可加载，并为后续真正的 benchmark harness 预留清晰接口。"
};

export const scaffoldBenchmarkScenarios: BenchmarkScenario[] = [
  {
    id: "decode-parity",
    name: {
      en: "Decode parity checks",
      zh: "解码一致性检查"
    },
    expectedOutput: "eot",
    description: {
      en: "Load representative fixture fonts, invoke the WASM runtime from a non-UI boundary, and compare metadata before measuring timings.",
      zh: "加载代表性字体样本，在非 UI 边界调用 WASM 运行时，并在计时前先比对基础元数据。"
    },
    focusAreas: [
      {
        en: "fixture loading",
        zh: "样本装载"
      },
      {
        en: "runtime warmup",
        zh: "运行时预热"
      },
      {
        en: "result capture",
        zh: "结果采集"
      }
    ]
  },
  {
    id: "subset-variation",
    name: {
      en: "Subset & variation sampling",
      zh: "子集与变体采样"
    },
    expectedOutput: "fntdata",
    description: {
      en: "Exercise the browser-facing path that will eventually benchmark subset and variation-aware conversion without embedding those concerns into components.",
      zh: "演练未来将用于 benchmark 的浏览器路径，覆盖子集与变体感知转换，同时不把这些逻辑塞进组件本身。"
    },
    focusAreas: [
      {
        en: "scenario definitions",
        zh: "场景定义"
      },
      {
        en: "timing hooks",
        zh: "计时挂钩"
      },
      {
        en: "diagnostic reporting",
        zh: "诊断上报"
      }
    ]
  }
];
