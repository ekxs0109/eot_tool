import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle
} from "@/components/ui/card";
import { Separator } from "@/components/ui/separator";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow
} from "@/components/ui/table";
import {
  Tabs,
  TabsContent
} from "@/components/ui/tabs";
import { RuntimePanel } from "@/components/runtime/RuntimePanel";
import { useFonttoolRuntime } from "@/components/runtime/useFonttoolRuntime";
import {
  benchmarkScaffoldSummary,
  scaffoldBenchmarkScenarios
} from "@/lib/benchmark/scenarios";
import {
  readInitialLocale,
  type AppLocale,
  writeLocale
} from "@/lib/formatting/i18n";
import { useState } from "react";

export function BenchmarkPage() {
  const { loadState, selectedStrategy, support, warmRuntime } = useFonttoolRuntime();
  const [locale, setLocale] = useState<AppLocale>(readInitialLocale);
  const [selectedScenario, setSelectedScenario] = useState(
    scaffoldBenchmarkScenarios[0]?.id ?? "decode-parity"
  );

  const copy = {
    browserRuntime: locale === "zh" ? "浏览器运行时" : "browser runtime",
    intent:
      locale === "zh"
        ? "在不把 loader 逻辑塞进 UI 的前提下，观测浏览器端运行时行为。"
        : "Measure browser runtime behavior without mixing loader code into the UI layer.",
    plannedSuites:
      locale === "zh" ? "已脚手架化的 benchmark 场景" : "Scaffolded benchmark surfaces",
    runtimeBoundary:
      locale === "zh"
        ? "运行时逻辑保留在 @/lib 下；UI 只负责渲染状态与控制。"
        : "Runtime logic stays under @/lib; the UI only renders state & controls.",
    scenarios: locale === "zh" ? "个场景" : "scenarios",
    focusArea: locale === "zh" ? "关注点" : "Focus area",
    intentColumn: locale === "zh" ? "目的" : "Intent",
    rowIntent:
      locale === "zh"
        ? "把这类关注点保留在 benchmark harness 内，而不是散落到展示组件里。"
        : "Capture this concern inside the benchmark harness rather than in presentational components."
  };

  return (
    <>
      <a className="sr-only focus:not-sr-only focus:absolute focus:left-4 focus:top-4 focus:z-50 focus:rounded-md focus:bg-white focus:px-3 focus:py-2 focus:text-sm focus:font-medium focus:text-foreground" href="#main-content">
        Skip to main content
      </a>

      <main
        className="mx-auto flex w-full max-w-6xl flex-col gap-6 px-4 py-6 md:px-6 md:py-8"
        id="main-content"
      >
      <Card className="overflow-hidden border-border/60 bg-white/85 shadow-xl shadow-slate-950/5 backdrop-blur">
        <CardHeader className="gap-3">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <div className="flex flex-wrap items-center gap-2">
            <Badge variant="secondary">Benchmark web</Badge>
            <Badge variant="outline">fonttool-wasm</Badge>
            <Badge variant="outline">{copy.browserRuntime}</Badge>
            </div>
            <div className="flex items-center gap-2">
              <Button
                onClick={() => {
                  setLocale("en");
                  writeLocale("en");
                }}
                size="sm"
                type="button"
                variant={locale === "en" ? "default" : "outline"}
              >
                EN
              </Button>
              <Button
                onClick={() => {
                  setLocale("zh");
                  writeLocale("zh");
                }}
                size="sm"
                type="button"
                variant={locale === "zh" ? "default" : "outline"}
              >
                中文
              </Button>
            </div>
          </div>
          <CardTitle className="max-w-3xl text-3xl leading-tight tracking-tight md:text-5xl">
            {copy.intent}
          </CardTitle>
          <CardDescription className="max-w-3xl text-base leading-relaxed text-slate-600">
            {benchmarkScaffoldSummary[locale]}
          </CardDescription>
        </CardHeader>
      </Card>

      <div className="grid gap-6 lg:grid-cols-[minmax(0,0.95fr)_minmax(0,1.25fr)]">
        <RuntimePanel
          loadState={loadState}
          locale={locale}
          selectedStrategy={selectedStrategy}
          support={support}
          onWarmRuntime={(strategy) => {
            void warmRuntime(strategy);
          }}
        />

        <Card className="border-border/60 bg-white/90 shadow-xl shadow-slate-950/5">
          <CardHeader className="gap-2">
            <div className="flex items-center justify-between gap-3">
              <div>
                <CardTitle className="text-xl">{copy.plannedSuites}</CardTitle>
                <CardDescription>
                  {copy.runtimeBoundary.replace("@/lib", "")}
                  <code translate="no">@/lib</code>
                  {locale === "zh"
                    ? "；UI 只负责渲染状态与控制。"
                    : "; the UI only renders state & controls."}
                </CardDescription>
              </div>
              <Badge variant="secondary">
                {scaffoldBenchmarkScenarios.length} {copy.scenarios}
              </Badge>
            </div>
          </CardHeader>
          <CardContent className="flex flex-col gap-5">
            <div className="flex flex-wrap gap-2">
              {scaffoldBenchmarkScenarios.map((scenario) => (
                <Button
                  key={scenario.id}
                  onClick={() => {
                    setSelectedScenario(scenario.id);
                  }}
                  type="button"
                  variant={selectedScenario === scenario.id ? "default" : "outline"}
                >
                  {scenario.name[locale]}
                </Button>
              ))}
            </div>

            <Tabs className="gap-4" value={selectedScenario}>
              {scaffoldBenchmarkScenarios.map((scenario) => (
                <TabsContent className="space-y-4" key={scenario.id} value={scenario.id}>
                  <Card className="border-border/60 bg-slate-50/70 shadow-none">
                    <CardHeader className="gap-2">
                      <div className="flex items-center justify-between gap-3">
                        <CardTitle className="text-lg">{scenario.name[locale]}</CardTitle>
                        <Badge variant="outline">{scenario.expectedOutput}</Badge>
                      </div>
                      <CardDescription>{scenario.description[locale]}</CardDescription>
                    </CardHeader>
                    <CardContent className="space-y-4">
                      <Separator />
                      <Table>
                        <TableHeader>
                          <TableRow>
                            <TableHead>{copy.focusArea}</TableHead>
                            <TableHead>{copy.intentColumn}</TableHead>
                          </TableRow>
                        </TableHeader>
                        <TableBody>
                          {scenario.focusAreas.map((area) => (
                            <TableRow key={`${scenario.id}-${area.en}`}>
                              <TableCell className="font-medium">{area[locale]}</TableCell>
                              <TableCell className="text-muted-foreground">
                                {copy.rowIntent}
                              </TableCell>
                            </TableRow>
                          ))}
                        </TableBody>
                      </Table>
                    </CardContent>
                  </Card>
                </TabsContent>
              ))}
            </Tabs>
          </CardContent>
        </Card>
      </div>
      </main>
    </>
  );
}
