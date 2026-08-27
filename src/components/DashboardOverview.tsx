import { ArrowUpRight, CircleHelp, Cpu, Database, Gauge, HardDrive, MemoryStick, Thermometer } from "lucide-react";
import { useI18n } from "../i18n";
import { type DashboardCardConfig, type DashboardConfig } from "../dashboard/config";
import { formatMetricValue, metricItemDisplayName, metricValue, type MetricCatalogItem, type MetricId } from "../dashboard/metrics";
import type { SystemSample } from "../types/resource";
import { formatBytes, formatClock } from "../utils/time";
import { Badge } from "./ui/Badge";
import { Button } from "./ui/Button";
import { Card, CardContent, CardHeader, CardTitle } from "./ui/Card";
import { StatusBadge } from "./MetricExplorer";

type DashboardOverviewProps = {
  config: DashboardConfig;
  catalog: MetricCatalogItem[];
  samples: SystemSample[];
  onFocusMetric: (metricId: MetricId) => void;
};

export function DashboardOverview({ config, catalog, samples, onFocusMetric }: DashboardOverviewProps) {
  const { language, t } = useI18n();
  const visibleCards = [...config.cards].filter((card) => card.visible).sort((left, right) => left.order - right.order);
  const latest = samples[samples.length - 1] ?? null;

  return <section aria-labelledby="dashboard-overview-title" className="space-y-3">
    <div className="flex flex-wrap items-end justify-between gap-3">
      <div>
        <div className="eyebrow">{t("dashboardOverviewEyebrow")}</div>
        <h2 id="dashboard-overview-title" className="mt-1 text-lg font-semibold">{t("dashboardTitle")}</h2>
        <p className="mt-1 max-w-2xl text-sm text-muted-foreground">{t("dashboardOverviewDescription")}</p>
      </div>
      {latest && <Badge className="border-border bg-card text-muted-foreground">{t("lastSampleAt", { time: formatClock(latest.timestampMs, language) })}</Badge>}
    </div>
    {!visibleCards.length ? <Card><CardContent className="flex items-start gap-3 py-8 text-sm text-muted-foreground"><CircleHelp size={17} className="mt-0.5 shrink-0" aria-hidden="true" /><span>{t("dashboardNoPinnedMetrics")}</span></CardContent></Card> : <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
      {visibleCards.map((card) => <OverviewCard key={card.id} card={card} catalog={catalog} latest={latest} samples={samples} onFocusMetric={onFocusMetric} />)}
    </div>}
  </section>;
}

function OverviewCard({ card, catalog, latest, samples, onFocusMetric }: {
  card: DashboardCardConfig;
  catalog: MetricCatalogItem[];
  latest: SystemSample | null;
  samples: SystemSample[];
  onFocusMetric: (metricId: MetricId) => void;
}) {
  const { language, t } = useI18n();
  const items = card.metricIds
    .filter((metricId) => !card.hiddenMetricIds.includes(metricId))
    .map((metricId) => catalog.find((item) => item.id === metricId))
    .filter((item): item is MetricCatalogItem => item != null);
  const title = overviewCardTitle(card, items, t);
  const firstItem = items[0];
  const secondaryMemory = card.id === "memory" && latest?.memoryUsedBytes != null && latest.memoryTotalBytes != null
    ? `${formatBytes(latest.memoryUsedBytes, language)} / ${formatBytes(latest.memoryTotalBytes, language)}`
    : null;

  return <Card className="relative overflow-hidden">
    <div className={`absolute inset-x-0 top-0 h-0.5 ${cardTone(items)}`} />
    <CardHeader className="pb-3">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <CardTitle className="truncate" title={title}>{title}</CardTitle>
          <p className="mt-1 text-xs font-normal text-muted-foreground">{items.length ? t("dashboardCurrent") : t("dashboardMetricUnavailable")}</p>
        </div>
        {firstItem && firstItem.status !== "UNSUPPORTED" && firstItem.status !== "UNKNOWN" && <Button type="button" variant="ghost" size="icon" className="h-8 w-8 shrink-0" aria-label={t("dashboardOpenInTrends")} onClick={() => onFocusMetric(firstItem.id)}><ArrowUpRight size={15} aria-hidden="true" /></Button>}
      </div>
    </CardHeader>
    <CardContent className="space-y-3 pt-0">
      {!items.length ? <div className="flex items-start gap-2 text-sm text-muted-foreground"><CircleHelp size={16} className="mt-0.5 shrink-0" aria-hidden="true" /><span>{t("dashboardMetricUnavailable")}</span></div> : <>
        <div className="space-y-2.5">
          {items.map((item) => <OverviewMetric key={item.id} item={item} latest={latest} samples={samples} />)}
        </div>
        {secondaryMemory && <div className="border-t border-border/70 pt-2 text-[11px] text-muted-foreground">{t("metricMemoryUsed")} · {secondaryMemory}</div>}
      </>}
    </CardContent>
  </Card>;
}

function OverviewMetric({ item, latest, samples }: { item: MetricCatalogItem; latest: SystemSample | null; samples: SystemSample[] }) {
  const { language, t } = useI18n();
  const label = metricItemDisplayName(item, t, samples);
  const value = latest ? metricValue(item.descriptor, latest) : null;
  const formatted = item.status === "AVAILABLE" || item.status === "DEGRADED"
    ? formatMetricValue(item.descriptor, value, language)
    : null;
  return <div className="flex items-start justify-between gap-3">
    <div className="flex min-w-0 items-start gap-2">
      <MetricIcon item={item} />
      <div className="min-w-0"><div className="truncate text-xs font-medium" title={label}>{label}</div><div className="mt-1"><StatusBadge status={item.status} /></div></div>
    </div>
    <div className={`shrink-0 text-right font-mono text-[17px] font-semibold tabular-nums ${formatted ? "text-foreground" : "text-muted-foreground"}`}>
      {formatted ?? statusValue(item.status, t)}
    </div>
  </div>;
}

function MetricIcon({ item }: { item: MetricCatalogItem }) {
  const Icon = item.category === "cpu" ? Cpu
    : item.category === "memory" ? MemoryStick
      : item.category === "disk" ? HardDrive
        : item.descriptor.gpuField === "temperature_c" ? Thermometer
          : item.descriptor.gpuField === "vram_used_bytes" || item.descriptor.gpuField === "vram_total_bytes" ? Database
            : Gauge;
  return <span className="mt-0.5 flex h-7 w-7 shrink-0 items-center justify-center rounded-lg bg-muted text-[hsl(var(--signal-cyan))]"><Icon size={15} aria-hidden="true" /></span>;
}

function overviewCardTitle(card: DashboardCardConfig, items: MetricCatalogItem[], t: ReturnType<typeof useI18n>["t"]): string {
  if (card.id === "compute-usage") return t("dashboardComputeUsage");
  if (card.id === "memory") return t("dashboardMemory");
  if (card.id === "disk-io") return t("dashboardDiskIo");
  if (card.id === "gpu-temperature") return `${t("categoryGpu")} · ${t("dashboardGroupTemperature")}`;
  if (card.id === "gpu-utilization") return `${t("categoryGpu")} · ${t("dashboardGroupUtilization")}`;
  return items[0] ? t(items[0].descriptor.translationKey) : t("dashboardTitle");
}

function statusValue(status: MetricCatalogItem["status"], t: ReturnType<typeof useI18n>["t"]): string {
  if (status === "NO_DATA_IN_RANGE") return t("dashboardNoCurrentSample");
  if (status === "DISABLED") return t("dashboardMetricStatusDisabled");
  if (status === "UNSUPPORTED") return t("dashboardMetricStatusUnsupported");
  if (status === "FAILED") return t("dashboardMetricStatusFailed");
  if (status === "DEGRADED") return t("dashboardNoCurrentSample");
  return t("dashboardNoCurrentSample");
}

function cardTone(items: MetricCatalogItem[]): string {
  if (items.some((item) => item.status === "FAILED")) return "bg-[hsl(var(--danger))]";
  if (items.some((item) => item.status === "DEGRADED")) return "bg-[hsl(var(--warning))]";
  if (items.every((item) => item.status === "UNSUPPORTED" || item.status === "DISABLED")) return "bg-[hsl(var(--muted-foreground)/0.35)]";
  return "bg-[hsl(var(--signal-cyan))]";
}
