import { useMemo, useState } from "react";
import { AlertTriangle, ArrowDown, ArrowUp, Check, CircleHelp, LineChart, Pin, RotateCcw, Search } from "lucide-react";
import { useI18n, type TranslationKey } from "../i18n";
import { isMetricPinned, type DashboardConfig } from "../dashboard/config";
import type { MetricCatalogLoadPhase } from "../dashboard/metricCatalogState";
import { gpuDeviceLabel, metricItemDisplayName, type MetricCatalogItem, type MetricId, type MetricUiStatus } from "../dashboard/metrics";
import type { MetricCategory, SystemSample } from "../types/resource";
import { Badge } from "./ui/Badge";
import { Button } from "./ui/Button";

type MetricExplorerProps = {
  catalog: MetricCatalogItem[];
  samples: SystemSample[];
  config: DashboardConfig;
  trendMetricIds: readonly MetricId[];
  onTogglePin: (metricId: MetricId) => void;
  onToggleTrend: (metricId: MetricId) => void;
  onMoveCard: (cardId: string, direction: -1 | 1) => void;
};

type StatusFilter = "all" | "usable" | MetricUiStatus;

const categoryOrder: MetricCategory[] = ["cpu", "memory", "disk", "gpu"];
const categoryLabels: Record<MetricCategory, TranslationKey> = {
  cpu: "categoryCpu",
  memory: "categoryMemory",
  disk: "categoryDisk",
  gpu: "categoryGpu",
  network: "categoryNetwork",
  power: "categoryPower",
  battery: "categoryBattery",
  process: "categoryProcess"
};

export function MetricExplorer({ catalog, samples, config, trendMetricIds, onTogglePin, onToggleTrend, onMoveCard }: MetricExplorerProps) {
  const { t } = useI18n();
  const [query, setQuery] = useState("");
  const [statusFilter, setStatusFilter] = useState<StatusFilter>("all");
  const normalizedQuery = query.trim().toLowerCase();
  const filtered = useMemo(() => catalog.filter((item) => {
    const label = metricItemDisplayName(item, t, samples).toLowerCase();
    const matchesQuery = !normalizedQuery || `${label} ${item.descriptor.unitLabel} ${item.providerId}`.toLowerCase().includes(normalizedQuery);
    const matchesStatus = statusFilter === "all"
      || (statusFilter === "usable" && (item.status === "AVAILABLE" || item.status === "NO_DATA_IN_RANGE" || item.status === "DEGRADED"))
      || item.status === statusFilter;
    return matchesQuery && matchesStatus;
  }), [catalog, normalizedQuery, samples, statusFilter, t]);

  const groups = useMemo(() => {
    const grouped = new Map<string, { category: MetricCategory; deviceKey: string | null; label: string; items: MetricCatalogItem[] }>();
    for (const item of filtered) {
      const deviceKey = item.category === "gpu" ? item.device?.stableKey ?? null : null;
      const label = item.category === "gpu"
        ? item.device ? gpuDeviceLabel(item.device) : item.providerDisplayName
        : t(categoryLabels[item.category]);
      const key = `${item.category}:${deviceKey ?? "system"}`;
      const existing = grouped.get(key);
      if (existing) existing.items.push(item);
      else grouped.set(key, { category: item.category, deviceKey, label, items: [item] });
    }
    return [...grouped.values()].sort((left, right) => {
      const categoryDelta = categoryOrder.indexOf(left.category) - categoryOrder.indexOf(right.category);
      return categoryDelta || left.label.localeCompare(right.label);
    });
  }, [filtered, t]);

  return <div className="space-y-4" aria-label={t("dashboardMetricExplorer")}>
    <div className="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
      <div className="flex min-w-0 flex-1 items-center gap-2 rounded-lg border border-input bg-card px-3 focus-within:ring-2 focus-within:ring-ring/25">
        <Search size={16} className="shrink-0 text-muted-foreground" aria-hidden="true" />
        <label className="sr-only" htmlFor="dashboard-metric-search">{t("dashboardMetricSearch")}</label>
        <input id="dashboard-metric-search" type="search" className="h-10 min-w-0 flex-1 border-0 bg-transparent text-sm outline-none" value={query} onChange={(event) => setQuery(event.target.value)} placeholder={t("dashboardMetricSearch")} />
        <span className="shrink-0 text-[11px] tabular-nums text-muted-foreground" aria-live="polite">{filtered.length}/{catalog.length}</span>
      </div>
      <label className="flex items-center gap-2 text-xs text-muted-foreground">
        <span>{t("dashboardMetricFilter")}</span>
        <select className="h-10 rounded-lg border border-input bg-card px-3 text-sm text-foreground outline-none focus:ring-2 focus:ring-ring/35" value={statusFilter} onChange={(event) => setStatusFilter(event.target.value as StatusFilter)} aria-label={t("dashboardMetricFilter")}>
          <option value="all">{t("dashboardAllStatuses")}</option>
          <option value="usable">{t("dashboardUsableMetrics")}</option>
          <option value="AVAILABLE">{t("dashboardMetricStatusAvailable")}</option>
          <option value="NO_DATA_IN_RANGE">{t("dashboardMetricStatusNoData")}</option>
          <option value="DISABLED">{t("dashboardMetricStatusDisabled")}</option>
          <option value="UNSUPPORTED">{t("dashboardMetricStatusUnsupported")}</option>
          <option value="FAILED">{t("dashboardMetricStatusFailed")}</option>
          <option value="DEGRADED">{t("dashboardMetricStatusDegraded")}</option>
          <option value="UNKNOWN">{t("dashboardMetricStatusUnknown")}</option>
        </select>
      </label>
    </div>
    <p className="text-xs leading-5 text-muted-foreground">{t("dashboardMetricStatusHint")}</p>
    {config.cards.length >= 12 && <p className="text-xs leading-5 text-muted-foreground">{t("dashboardOverviewLimit")}</p>}
    {!groups.length ? <div className="flex flex-wrap items-center justify-center gap-3 rounded-lg border border-dashed border-border px-4 py-8 text-center text-sm text-muted-foreground"><span>{t("dashboardNoMetricsAvailable")}</span>{(normalizedQuery || statusFilter !== "all") && <Button type="button" variant="outline" className="h-8 px-2.5 text-xs" onClick={() => { setQuery(""); setStatusFilter("all"); }}>{t("dashboardClearFilters")}</Button>}</div> : <div className="space-y-4">
      {groups.map((group) => <section key={`${group.category}:${group.deviceKey ?? "system"}`} aria-labelledby={`metric-group-${group.category}-${group.deviceKey ?? "system"}`}>
        <div className="mb-2 flex items-center gap-2">
          <div className="eyebrow" id={`metric-group-${group.category}-${group.deviceKey ?? "system"}`}>{group.category === "gpu" ? t("categoryGpu") : group.label}</div>
          {group.category === "gpu" && <span className="min-w-0 truncate text-xs font-medium text-foreground" title={group.label}>{group.label}</span>}
          <span className="text-[11px] tabular-nums text-muted-foreground">{group.items.length}</span>
        </div>
        <div className="overflow-hidden rounded-lg border border-border/80 bg-muted/10">
          {group.items.map((item) => {
            const pinned = isMetricPinned(config, item.id);
            const belongsToOverview = config.cards.some((card) => card.metricIds.includes(item.id));
            const canActivelySelect = item.status !== "UNSUPPORTED" && item.status !== "UNKNOWN";
            return <MetricExplorerRow key={item.id} item={item} samples={samples} pinned={pinned} inTrend={trendMetricIds.includes(item.id)} canPin={pinned || (canActivelySelect && (belongsToOverview || config.cards.length < 12))} canTrend={canActivelySelect} onTogglePin={onTogglePin} onToggleTrend={onToggleTrend} />;
          })}
        </div>
      </section>)}
    </div>}
    <OverviewPins config={config} catalog={catalog} samples={samples} onMoveCard={onMoveCard} onTogglePin={onTogglePin} />
  </div>;
}

export function MetricCatalogLoadNotice({ phase, onRetry }: { phase: MetricCatalogLoadPhase; onRetry: () => void }) {
  const { t } = useI18n();
  if (phase === "loaded") return null;
  const failed = phase === "failed";
  const className = failed
    ? "border-[hsl(var(--warning)/0.4)] bg-[hsl(var(--warning-surface))] flex flex-wrap items-center justify-between gap-3 rounded-lg border px-3 py-2.5 text-xs"
    : "border-border bg-muted/35 flex flex-wrap items-center justify-between gap-3 rounded-lg border px-3 py-2.5 text-xs";
  return <div role={failed ? "alert" : "status"} aria-live={failed ? "assertive" : "polite"} aria-busy={!failed} className={className}>
    <div className="flex min-w-0 items-start gap-2">
      {failed ? <AlertTriangle size={15} className="mt-0.5 shrink-0 text-[hsl(var(--warning))]" aria-hidden="true" /> : <CircleHelp size={15} className="mt-0.5 shrink-0 text-muted-foreground" aria-hidden="true" />}
      <div className="min-w-0"><div className="font-medium text-foreground">{failed ? t("dashboardMetricCatalogDegradedTitle") : t("dashboardMetricCatalogLoading")}</div><div className="mt-0.5 text-muted-foreground">{failed ? t("dashboardMetricCatalogDegradedMessage") : t("dashboardMetricCatalogLoadingMessage")}</div></div>
    </div>
    {failed && <Button type="button" variant="outline" className="h-8 shrink-0 px-2.5 text-xs" onClick={onRetry}><RotateCcw size={13} aria-hidden="true" />{t("dashboardRetryMetricCatalog")}</Button>}
  </div>;
}

function MetricExplorerRow({ item, samples, pinned, inTrend, canPin, canTrend, onTogglePin, onToggleTrend }: {
  item: MetricCatalogItem;
  samples: SystemSample[];
  pinned: boolean;
  inTrend: boolean;
  canPin: boolean;
  canTrend: boolean;
  onTogglePin: (metricId: MetricId) => void;
  onToggleTrend: (metricId: MetricId) => void;
}) {
  const { t } = useI18n();
  const label = metricItemDisplayName(item, t, samples);
  return <div className="flex flex-col gap-3 border-b border-border/70 px-4 py-3 last:border-b-0 sm:flex-row sm:items-center sm:justify-between">
    <div className="min-w-0">
      <div className="flex min-w-0 flex-wrap items-center gap-2">
        <span className="truncate text-sm font-medium" title={label}>{label}</span>
        <StatusBadge status={item.status} />
      </div>
      <div className="mt-1 flex flex-wrap items-center gap-x-3 gap-y-1 text-[11px] text-muted-foreground">
        <span>{item.descriptor.unitLabel}</span>
        <span>{item.providerDisplayName}</span>
      </div>
    </div>
    <div className="flex shrink-0 flex-wrap items-center gap-2">
      <Button type="button" variant={pinned ? "default" : "outline"} className="h-8 px-2.5 text-xs" disabled={!canPin} aria-pressed={pinned} aria-label={pinned ? t("dashboardUnpinOverview") : t("dashboardPinOverview")} onClick={() => onTogglePin(item.id)}>
        <Pin size={13} aria-hidden="true" />{pinned ? t("dashboardUnpinOverview") : t("dashboardPinOverview")}
      </Button>
      <Button type="button" variant={inTrend ? "default" : "ghost"} className="h-8 px-2.5 text-xs" disabled={!canTrend} aria-pressed={inTrend} aria-label={inTrend ? t("dashboardRemoveFromTrend") : t("dashboardAddToTrend")} onClick={() => onToggleTrend(item.id)}>
        <LineChart size={13} aria-hidden="true" />{inTrend ? t("dashboardInTrends") : t("dashboardAddToTrend")}
      </Button>
    </div>
  </div>;
}

function OverviewPins({ config, catalog, samples, onMoveCard, onTogglePin }: {
  config: DashboardConfig;
  catalog: MetricCatalogItem[];
  samples: SystemSample[];
  onMoveCard: (cardId: string, direction: -1 | 1) => void;
  onTogglePin: (metricId: MetricId) => void;
}) {
  const { t } = useI18n();
  const sortedCards = [...config.cards].filter((card) => card.visible).sort((left, right) => left.order - right.order);
  return <div className="border-t border-border/70 pt-4">
    <div className="mb-2 flex items-center justify-between gap-3"><div className="eyebrow">{t("dashboardOverviewPins")}</div><span className="text-[11px] text-muted-foreground">{sortedCards.length}/12</span></div>
    {!sortedCards.length ? <div className="rounded-lg border border-dashed border-border px-4 py-5 text-sm text-muted-foreground">{t("dashboardNoPinnedMetrics")}</div> : <div className="space-y-2">
      {sortedCards.map((card, index) => {
        const visible = card.metricIds.filter((metricId) => !card.hiddenMetricIds.includes(metricId));
        const labels = visible.map((metricId) => {
          const item = catalog.find((candidate) => candidate.id === metricId);
          return item ? metricItemDisplayName(item, t, samples) : t("dashboardMetricUnavailable");
        });
        return <div key={card.id} className="flex items-center gap-3 rounded-lg border border-border/80 bg-card px-3 py-2.5">
          <div className="min-w-0 flex-1"><div className="truncate text-sm font-medium">{labels.join(" · ") || t("dashboardNoPinnedMetrics")}</div><div className="mt-1 text-[11px] text-muted-foreground">{visible.length} {t("dashboardMetricSearch").toLowerCase()}</div></div>
          <div className="flex shrink-0 items-center gap-1">
            <Button type="button" variant="ghost" size="icon" className="h-8 w-8" disabled={index === 0} aria-label={t("dashboardMoveUp")} onClick={() => onMoveCard(card.id, -1)}><ArrowUp size={14} aria-hidden="true" /></Button>
            <Button type="button" variant="ghost" size="icon" className="h-8 w-8" disabled={index === sortedCards.length - 1} aria-label={t("dashboardMoveDown")} onClick={() => onMoveCard(card.id, 1)}><ArrowDown size={14} aria-hidden="true" /></Button>
            {visible.length === 1 && <Button type="button" variant="ghost" className="h-8 px-2 text-xs text-[hsl(var(--danger))]" onClick={() => onTogglePin(visible[0])}>{t("dashboardUnpinOverview")}</Button>}
          </div>
        </div>;
      })}
    </div>}
  </div>;
}

export function StatusBadge({ status }: { status: MetricUiStatus }) {
  const { t } = useI18n();
  const label = status === "AVAILABLE" ? t("dashboardMetricStatusAvailable")
    : status === "NO_DATA_IN_RANGE" ? t("dashboardMetricStatusNoData")
      : status === "DISABLED" ? t("dashboardMetricStatusDisabled")
        : status === "UNSUPPORTED" ? t("dashboardMetricStatusUnsupported")
          : status === "FAILED" ? t("dashboardMetricStatusFailed")
            : status === "DEGRADED" ? t("dashboardMetricStatusDegraded")
              : t("dashboardMetricStatusUnknown");
  const Icon = status === "AVAILABLE" ? Check : status === "FAILED" || status === "DEGRADED" ? AlertTriangle : CircleHelp;
  const className = status === "AVAILABLE" ? "border-[hsl(var(--success)/0.3)] bg-[hsl(var(--success-surface))] text-[hsl(var(--success))]"
    : status === "FAILED" ? "border-[hsl(var(--danger)/0.35)] bg-[hsl(var(--danger-surface))] text-[hsl(var(--danger))]"
      : status === "DEGRADED" ? "border-[hsl(var(--warning)/0.35)] bg-[hsl(var(--warning-surface))] text-[hsl(var(--warning))]"
        : status === "UNSUPPORTED" || status === "DISABLED" ? "border-border bg-muted text-muted-foreground"
          : "border-[hsl(var(--signal-blue)/0.3)] bg-card text-muted-foreground";
  return <Badge className={`gap-1 ${className}`}><Icon size={11} aria-hidden="true" />{label}</Badge>;
}
