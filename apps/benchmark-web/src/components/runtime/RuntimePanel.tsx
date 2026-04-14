import type { RuntimeStrategy, RuntimeSupport } from "fonttool-wasm";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle
} from "@/components/ui/card";
import { Progress } from "@/components/ui/progress";
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
  ToggleGroup,
  ToggleGroupItem
} from "@/components/ui/toggle-group";
import {
  formatCapability,
  formatFallbackReason
} from "@/lib/formatting/runtime";
import type { AppLocale } from "@/lib/formatting/i18n";
import type { RuntimeLoadState } from "@/components/runtime/useFonttoolRuntime";

type RuntimePanelProps = {
  loadState: RuntimeLoadState;
  locale: AppLocale;
  selectedStrategy: RuntimeStrategy;
  support: RuntimeSupport;
  onWarmRuntime: (strategy: RuntimeStrategy) => void;
};

export function RuntimePanel({
  loadState,
  locale,
  selectedStrategy,
  support,
  onWarmRuntime
}: RuntimePanelProps) {
  const diagnostics =
    loadState.status === "ready" ? loadState.diagnostics : undefined;
  const progressValue =
    loadState.status === "idle"
      ? 0
      : loadState.status === "loading"
        ? 45
        : loadState.status === "ready"
          ? 100
          : 100;
  const copy = {
    runtimeBoundary: locale === "zh" ? "运行时边界" : "Runtime boundary",
    workspaceRuntimeStatus:
      locale === "zh" ? "工作区运行时状态" : "Workspace runtime status",
    runtimeDescription:
      locale === "zh"
        ? "从非 UI hook 预热包运行时，并把诊断信息作为 benchmark 元数据暴露出来。"
        : "Warm the package runtime from a non-UI hook and surface diagnostics as benchmark metadata.",
    pthreadHint:
      locale === "zh"
        ? "如需启用 pthreads，当前页面必须运行在 cross-origin isolated 环境（COOP/COEP）下。"
        : "To enable pthreads, this page must run in a cross-origin isolated environment with COOP/COEP headers.",
    warmupStrategy: locale === "zh" ? "预热策略" : "Warmup strategy",
    warmupDescription:
      locale === "zh"
        ? "先验证运行时装载链路，再继续往完整 benchmark 流程扩展。"
        : "Keep this scaffold intentionally narrow: prove runtime loading before adding full benchmark flows.",
    warmSingle: locale === "zh" ? "预热单线程运行时" : "Warm single runtime",
    probeAuto: locale === "zh" ? "探测自动模式" : "Probe auto runtime",
    loading: locale === "zh" ? "正在加载运行时…" : "Loading runtime…",
    idleTitle: locale === "zh" ? "运行时空闲" : "Runtime idle",
    idleDescription:
      locale === "zh"
        ? "当前还没有加载 wasm 运行时。先点击预热按钮验证包边界。"
        : "No wasm runtime has been loaded yet. Use the warmup controls to verify the package boundary.",
    loadingTitle: locale === "zh" ? "运行时加载中" : "Runtime loading",
    loadingDescription:
      locale === "zh"
        ? "工作区包正在装载 staged wasm 产物并解析运行时变体。"
        : "The workspace package is loading staged wasm artifacts and resolving the runtime variant.",
    errorTitle: locale === "zh" ? "运行时失败" : "Runtime failed",
    diagnosticsTitle: locale === "zh" ? "最近一次运行时诊断" : "Last runtime diagnostics",
    diagnosticsDescription:
      locale === "zh"
        ? "后续 benchmark 运行会基于这里的状态做快照与比较。"
        : "This is the state that future benchmark runs will snapshot and compare.",
    resolvedMode: locale === "zh" ? "解析模式" : "Resolved mode",
    variant: locale === "zh" ? "变体" : "Variant",
    requestedThreads: locale === "zh" ? "请求线程数" : "Requested threads",
    effectiveThreads: locale === "zh" ? "实际线程数" : "Effective threads",
    fallback: locale === "zh" ? "回退原因" : "Fallback"
  };

  return (
    <Card className="border-border/60 bg-white/90 shadow-xl shadow-slate-950/5">
      <CardHeader className="gap-3">
        <div className="flex flex-wrap items-center gap-2">
          <Badge variant="secondary">{copy.runtimeBoundary}</Badge>
          <Badge variant="outline">{support.runtimeKind}</Badge>
        </div>
        <CardTitle className="text-xl">{copy.workspaceRuntimeStatus}</CardTitle>
        <CardDescription>
          {copy.runtimeDescription}
        </CardDescription>
      </CardHeader>

      <CardContent className="flex flex-col gap-5">
        <div className="grid gap-3 sm:grid-cols-2">
          <CapabilityTile
            label="SharedArrayBuffer"
            value={formatCapability(support.sharedArrayBuffer)}
          />
          <CapabilityTile
            label="Cross-origin isolated"
            value={formatCapability(support.crossOriginIsolated)}
          />
          <CapabilityTile
            label="Pthreads possible"
            value={formatCapability(support.pthreadsPossible)}
          />
          <CapabilityTile
            label="Runtime kind"
            value={support.runtimeKind}
          />
        </div>

        {!support.pthreadsPossible ? (
          <Alert>
            <AlertTitle>{locale === "zh" ? "Pthreads 当前不可用" : "Pthreads currently unavailable"}</AlertTitle>
            <AlertDescription>{copy.pthreadHint}</AlertDescription>
          </Alert>
        ) : null}

        <Separator />

        <div className="flex flex-col gap-3">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <div>
              <p className="text-sm font-medium">{copy.warmupStrategy}</p>
              <p className="text-sm text-muted-foreground">
                {copy.warmupDescription}
              </p>
            </div>

            <ToggleGroup
              className="justify-start"
              value={selectedStrategy}
              onValueChange={(value: string) => {
                if (value === "single" || value === "auto" || value === "pthreads") {
                  onWarmRuntime(value);
                }
              }}
              type="single"
            >
              <ToggleGroupItem value="single">Single</ToggleGroupItem>
              <ToggleGroupItem value="auto">Auto</ToggleGroupItem>
              <ToggleGroupItem value="pthreads">Pthreads</ToggleGroupItem>
            </ToggleGroup>
          </div>

          <div className="flex flex-wrap items-center gap-3">
            <Button
              disabled={loadState.status === "loading"}
              onClick={() => {
                onWarmRuntime("single");
              }}
              type="button"
            >
              {loadState.status === "loading" ? copy.loading : copy.warmSingle}
            </Button>
            <Button
              disabled={loadState.status === "loading"}
              onClick={() => {
                onWarmRuntime("auto");
              }}
              type="button"
              variant="outline"
            >
              {copy.probeAuto}
            </Button>
          </div>

          <Progress value={progressValue} />
        </div>

        {loadState.status === "idle" ? (
          <Alert>
            <AlertTitle>{copy.idleTitle}</AlertTitle>
            <AlertDescription>
              {copy.idleDescription}
            </AlertDescription>
          </Alert>
        ) : null}

        {loadState.status === "loading" ? (
          <Alert>
            <AlertTitle>{copy.loadingTitle}</AlertTitle>
            <AlertDescription>
              {copy.loadingDescription}
            </AlertDescription>
          </Alert>
        ) : null}

        {loadState.status === "error" ? (
          <Alert variant="destructive">
            <AlertTitle>{copy.errorTitle}</AlertTitle>
            <AlertDescription>{loadState.message}</AlertDescription>
          </Alert>
        ) : null}

        {diagnostics !== undefined ? (
          <Card className="border-border/60 bg-slate-50/70 shadow-none">
            <CardHeader className="gap-2">
              <CardTitle className="text-lg">{copy.diagnosticsTitle}</CardTitle>
              <CardDescription>
                {copy.diagnosticsDescription}
              </CardDescription>
            </CardHeader>
            <CardContent>
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>{locale === "zh" ? "字段" : "Field"}</TableHead>
                    <TableHead>{locale === "zh" ? "值" : "Value"}</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  <TableRow>
                    <TableCell className="font-medium">{copy.resolvedMode}</TableCell>
                    <TableCell>{diagnostics.resolvedMode}</TableCell>
                  </TableRow>
                  <TableRow>
                    <TableCell className="font-medium">{copy.variant}</TableCell>
                    <TableCell>{diagnostics.variant}</TableCell>
                  </TableRow>
                  <TableRow>
                    <TableCell className="font-medium">{copy.requestedThreads}</TableCell>
                    <TableCell>{String(diagnostics.requestedThreads)}</TableCell>
                  </TableRow>
                  <TableRow>
                    <TableCell className="font-medium">{copy.effectiveThreads}</TableCell>
                    <TableCell>{String(diagnostics.effectiveThreads)}</TableCell>
                  </TableRow>
                  <TableRow>
                    <TableCell className="font-medium">{copy.fallback}</TableCell>
                    <TableCell>{formatFallbackReason(diagnostics.fallbackReason)}</TableCell>
                  </TableRow>
                </TableBody>
              </Table>
            </CardContent>
          </Card>
        ) : null}
      </CardContent>
    </Card>
  );
}

function CapabilityTile({
  label,
  value
}: {
  label: string;
  value: string;
}) {
  return (
    <Card className="border-border/60 bg-slate-50/70 shadow-none">
      <CardContent className="flex flex-col gap-1 px-5 py-4">
        <p className="text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">
          {label}
        </p>
        <p className="text-base font-semibold text-foreground">{value}</p>
      </CardContent>
    </Card>
  );
}
