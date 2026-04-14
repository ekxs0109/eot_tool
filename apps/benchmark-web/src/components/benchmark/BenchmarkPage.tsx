import { Badge } from "@/components/ui/badge";
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
  TabsContent,
  TabsList,
  TabsTrigger
} from "@/components/ui/tabs";
import { RuntimePanel } from "@/components/runtime/RuntimePanel";
import { useFonttoolRuntime } from "@/components/runtime/useFonttoolRuntime";
import {
  benchmarkScaffoldSummary,
  scaffoldBenchmarkScenarios
} from "@/lib/benchmark/scenarios";

export function BenchmarkPage() {
  const { loadState, selectedStrategy, support, warmRuntime } = useFonttoolRuntime();

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
          <div className="flex flex-wrap items-center gap-2">
            <Badge variant="secondary">Benchmark web</Badge>
            <Badge variant="outline">fonttool-wasm</Badge>
            <Badge variant="outline">browser runtime</Badge>
          </div>
          <CardTitle className="max-w-3xl text-3xl leading-tight tracking-tight md:text-5xl">
            Measure browser runtime behavior without mixing loader code into the UI layer.
          </CardTitle>
          <CardDescription className="max-w-3xl text-base leading-relaxed text-slate-600">
            {benchmarkScaffoldSummary}
          </CardDescription>
        </CardHeader>
      </Card>

      <div className="grid gap-6 lg:grid-cols-[minmax(0,0.95fr)_minmax(0,1.25fr)]">
        <RuntimePanel
          loadState={loadState}
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
                <CardTitle className="text-xl">Scaffolded benchmark surfaces</CardTitle>
                <CardDescription>
                  Runtime logic stays under <code translate="no">@/lib</code>; the UI only renders state &amp; controls.
                </CardDescription>
              </div>
              <Badge variant="secondary">{scaffoldBenchmarkScenarios.length} scenarios</Badge>
            </div>
          </CardHeader>
          <CardContent className="flex flex-col gap-5">
            <Tabs className="gap-4" defaultValue={scaffoldBenchmarkScenarios[0]?.id ?? "decode-parity"}>
              <TabsList className="w-full justify-start overflow-x-auto">
                {scaffoldBenchmarkScenarios.map((scenario) => (
                  <TabsTrigger key={scenario.id} value={scenario.id}>
                    {scenario.name}
                  </TabsTrigger>
                ))}
              </TabsList>

              {scaffoldBenchmarkScenarios.map((scenario) => (
                <TabsContent className="space-y-4" key={scenario.id} value={scenario.id}>
                  <Card className="border-border/60 bg-slate-50/70 shadow-none">
                    <CardHeader className="gap-2">
                      <div className="flex items-center justify-between gap-3">
                        <CardTitle className="text-lg">{scenario.name}</CardTitle>
                        <Badge variant="outline">{scenario.expectedOutput}</Badge>
                      </div>
                      <CardDescription>{scenario.description}</CardDescription>
                    </CardHeader>
                    <CardContent className="space-y-4">
                      <Separator />
                      <Table>
                        <TableHeader>
                          <TableRow>
                            <TableHead>Focus area</TableHead>
                            <TableHead>Intent</TableHead>
                          </TableRow>
                        </TableHeader>
                        <TableBody>
                          {scenario.focusAreas.map((area) => (
                            <TableRow key={area}>
                              <TableCell className="font-medium">{area}</TableCell>
                              <TableCell className="text-muted-foreground">
                                Capture this concern inside the benchmark harness rather than in presentational components.
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
