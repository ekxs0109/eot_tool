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
import type { RuntimeLoadState } from "@/components/runtime/useFonttoolRuntime";

type RuntimePanelProps = {
  loadState: RuntimeLoadState;
  selectedStrategy: RuntimeStrategy;
  support: RuntimeSupport;
  onWarmRuntime: (strategy: RuntimeStrategy) => void;
};

export function RuntimePanel({
  loadState,
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

  return (
    <Card className="border-border/60 bg-white/90 shadow-xl shadow-slate-950/5">
      <CardHeader className="gap-3">
        <div className="flex flex-wrap items-center gap-2">
          <Badge variant="secondary">Runtime boundary</Badge>
          <Badge variant="outline">{support.runtimeKind}</Badge>
        </div>
        <CardTitle className="text-xl">Workspace runtime status</CardTitle>
        <CardDescription>
          Warm the package runtime from a non-UI hook and surface diagnostics as benchmark metadata.
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

        <Separator />

        <div className="flex flex-col gap-3">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <div>
              <p className="text-sm font-medium">Warmup strategy</p>
              <p className="text-sm text-muted-foreground">
                Keep this scaffold intentionally narrow: prove runtime loading before adding full benchmark flows.
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
              {loadState.status === "loading" ? "Loading runtime..." : "Warm single runtime"}
            </Button>
            <Button
              disabled={loadState.status === "loading"}
              onClick={() => {
                onWarmRuntime("auto");
              }}
              type="button"
              variant="outline"
            >
              Probe auto runtime
            </Button>
          </div>

          <Progress value={progressValue} />
        </div>

        {loadState.status === "idle" ? (
          <Alert>
            <AlertTitle>Runtime idle</AlertTitle>
            <AlertDescription>
              No wasm runtime has been loaded yet. Use the warmup controls to verify the package boundary.
            </AlertDescription>
          </Alert>
        ) : null}

        {loadState.status === "loading" ? (
          <Alert>
            <AlertTitle>Runtime loading</AlertTitle>
            <AlertDescription>
              The workspace package is loading staged wasm artifacts and resolving the runtime variant.
            </AlertDescription>
          </Alert>
        ) : null}

        {loadState.status === "error" ? (
          <Alert variant="destructive">
            <AlertTitle>Runtime failed</AlertTitle>
            <AlertDescription>{loadState.message}</AlertDescription>
          </Alert>
        ) : null}

        {diagnostics !== undefined ? (
          <Card className="border-border/60 bg-slate-50/70 shadow-none">
            <CardHeader className="gap-2">
              <CardTitle className="text-lg">Last runtime diagnostics</CardTitle>
              <CardDescription>
                This is the state that future benchmark runs will snapshot and compare.
              </CardDescription>
            </CardHeader>
            <CardContent>
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Field</TableHead>
                    <TableHead>Value</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  <TableRow>
                    <TableCell className="font-medium">Resolved mode</TableCell>
                    <TableCell>{diagnostics.resolvedMode}</TableCell>
                  </TableRow>
                  <TableRow>
                    <TableCell className="font-medium">Variant</TableCell>
                    <TableCell>{diagnostics.variant}</TableCell>
                  </TableRow>
                  <TableRow>
                    <TableCell className="font-medium">Requested threads</TableCell>
                    <TableCell>{String(diagnostics.requestedThreads)}</TableCell>
                  </TableRow>
                  <TableRow>
                    <TableCell className="font-medium">Effective threads</TableCell>
                    <TableCell>{String(diagnostics.effectiveThreads)}</TableCell>
                  </TableRow>
                  <TableRow>
                    <TableCell className="font-medium">Fallback</TableCell>
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
