import { useEffect, useId, useMemo, useRef, useState } from "react";
import type { ECharts } from "echarts";
import { AlertTriangle, Check, CircleHelp, LineChart, RotateCcw } from "lucide-react";
import { useI18n, type TranslationKey } from "../i18n";
import { buildDashboardChartOption } from "../dashboard/chartOptions";
import { formatMetricValue, hasMetricData, metricItemDisplayName, metricValue, trendFamilies, type MetricCatalogItem, type MetricId, type UnitFamily } from "../dashboard/metrics";
import type { SystemSample, TimelineGap } from "../types/resource";
import { formatClock } from "../utils/time";
import { useUiStore } from "../stores/uiStore";
import { StatusBadge } from "./MetricExplorer";
import { Button } from "./ui/Button";
import { Card, CardContent, CardHeader, CardTitle } from "./ui/Card";
import { useStableEcharts } from "./chartLifecycle";

export type TrendMetricSelections = Partial<Record<UnitFamily, MetricId[]>>;

type DashboardTrendWorkspaceProps = {
  catalog: MetricCatalogItem[];
  samples: SystemSample[];
  gaps: TimelineGap[];
  startMs: number;
  endMs: number;
  selectedTimestampMs: number | null;
  trendSelections: TrendMetricSelections;
  activeFamily: UnitFamily | null;
  onFamilyChange: (family: UnitFamily) => void;
  onToggleMetric: (metricId: MetricId) => void;
  onSampleSelect: (sample: SystemSample) => void;
  onOpenExplorer: () => void;
};

const familyLabels: Record<UnitFamily, TranslationKey> = {
  percent: "dashboardGroupUtilization",
  throughput: "dashboardGroupDisk",
  bytes: "dashboardGroupMemory",
  temperature: "dashboardGroupTemperature",
  power: "dashboardGroupPower",
  frequency: "dashboardGroupFrequency"
};

export function DashboardTrendWorkspace({ catalog, samples, gaps, startMs, endMs, selectedTimestampMs, trendSelections, activeFamily, onFamilyChange, onToggleMetric, onSampleSelect, onOpenExplorer }: DashboardTrendWorkspaceProps) {
  const { t } = useI18n();
  const grouped = useMemo(() => {
    const map = new Map<UnitFamily, MetricCatalogItem[]>();
    for (const item of catalog) {
      const items = map.get(item.descriptor.unitFamily) ?? [];
      items.push(item);
      map.set(item.descriptor.unitFamily, items);
    }
    return map;
  }, [catalog]);
  const families = trendFamilies(catalog);
  const family = activeFamily && families.includes(activeFamily) ? activeFamily : families[0] ?? null;
  const familyItems = family ? grouped.get(family) ?? [] : [];
  const selectedIds = useMemo(() => family ? trendSelections[family] ?? [] : [], [family, trendSelections]);
  const selectedItems = useMemo(() => selectedIds.map((id) => catalog.find((item) => item.id === id)).filter((item): item is MetricCatalogItem => item != null), [catalog, selectedIds]);

  return <section aria-labelledby="dashboard-trend-title" className="space-y-3">
    <div className="flex flex-wrap items-end justify-between gap-3">
      <div>
        <div className="eyebrow">{t("dashboardTrendEyebrow")}</div>
        <h2 id="dashboard-trend-title" className="mt-1 text-lg font-semibold">{t("dashboardTrendEyebrow")}</h2>
        <p className="mt-1 max-w-2xl text-sm text-muted-foreground">{t("dashboardTrendDescription")}</p>
      </div>
      <Button type="button" variant="outline" className="h-8 px-2.5 text-xs" onClick={onOpenExplorer}><LineChart size={14} aria-hidden="true" />{t("dashboardExploreMetrics")}</Button>
    </div>
    <Card className="overflow-hidden">
      <CardHeader className="border-b border-border/70 bg-card/90 pb-3">
        <div className="flex flex-wrap items-center gap-2" role="group" aria-label={t("dashboardTrendEyebrow")}>
          {families.map((candidate) => <button key={candidate} type="button" aria-pressed={candidate === family} className={candidate === family ? "segmented-control-active" : "segmented-control-item"} onClick={() => onFamilyChange(candidate)}>{t(familyLabels[candidate])}</button>)}
          {!families.length && <span className="text-sm text-muted-foreground">{t("dashboardNoMetricsAvailable")}</span>}
        </div>
      </CardHeader>
      <CardContent className="space-y-4 pt-4">
        {!family ? <TrendEmpty onOpenExplorer={onOpenExplorer} /> : <>
          <TrendMetricPicker items={familyItems} selectedIds={selectedIds} samples={samples} onToggleMetric={onToggleMetric} />
          <TrendChart catalog={catalog} selectedItems={selectedItems} samples={samples} gaps={gaps} startMs={startMs} endMs={endMs} selectedTimestampMs={selectedTimestampMs} onSampleSelect={onSampleSelect} />
        </>}
      </CardContent>
    </Card>
  </section>;
}

function TrendMetricPicker({ items, selectedIds, samples, onToggleMetric }: { items: MetricCatalogItem[]; selectedIds: readonly MetricId[]; samples: SystemSample[]; onToggleMetric: (metricId: MetricId) => void }) {
  const { t } = useI18n();
  const [expanded, setExpanded] = useState(false);
  const selected = new Set(selectedIds);
  const selectedItems = items.filter((item) => selected.has(item.id));
  const visible = expanded ? items : items.slice(0, 8);
  const visibleIds = new Set(visible.map((item) => item.id));
  for (const item of selectedItems) if (!visibleIds.has(item.id)) visible.push(item);
  const hiddenCount = Math.max(0, items.length - 8);
  return <div className="space-y-2" aria-label={t("dashboardTrendSelectMetric")}>
    <div className="flex flex-wrap items-center justify-between gap-2"><div className="text-xs font-medium text-muted-foreground">{t("dashboardTrendSelectMetric")}</div><span className="text-[11px] tabular-nums text-muted-foreground">{selectedIds.length}/{items.length}</span></div>
    <div className="flex flex-wrap gap-2">
      {visible.map((item) => {
        const disabled = (item.status === "UNSUPPORTED" || item.status === "UNKNOWN") && !selected.has(item.id);
        const label = metricItemDisplayName(item, t, samples);
        return <button key={item.id} type="button" aria-pressed={selected.has(item.id)} disabled={disabled} title={label} className={`inline-flex max-w-full items-center gap-2 rounded-full border px-3 py-1.5 text-xs transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/35 ${selected.has(item.id) ? "border-[hsl(var(--signal-cyan)/0.55)] bg-[hsl(var(--signal-cyan)/0.12)] text-foreground" : "border-border bg-card text-muted-foreground hover:bg-muted"}`} onClick={() => onToggleMetric(item.id)}><span className="max-w-[240px] truncate">{label}</span>{selected.has(item.id) ? <Check size={12} aria-hidden="true" /> : <StatusBadge status={item.status} />}</button>;
      })}
    </div>
    {hiddenCount > 0 && <Button type="button" variant="ghost" className="h-7 px-2 text-xs" onClick={() => setExpanded((value) => !value)}>{expanded ? t("dashboardLessMetrics") : `${t("dashboardMoreMetrics")} · ${hiddenCount}`}</Button>}
  </div>;
}

function TrendChart({ catalog, selectedItems, samples, gaps, startMs, endMs, selectedTimestampMs, onSampleSelect }: { catalog: MetricCatalogItem[]; selectedItems: MetricCatalogItem[]; samples: SystemSample[]; gaps: TimelineGap[]; startMs: number; endMs: number; selectedTimestampMs: number | null; onSampleSelect: (sample: SystemSample) => void }) {
  const { language, t } = useI18n();
  const resolvedTheme = useUiStore((state) => state.resolvedTheme);
  const ref = useRef<HTMLDivElement | null>(null);
  const lifecycle = useStableEcharts(ref);
  const samplesRef = useRef(samples);
  const onSampleSelectRef = useRef(onSampleSelect);
  const summaryId = `dashboard-trend-summary-${useId().replace(/:/g, "")}`;
  const [chartError, setChartError] = useState(false);
  const [retryToken, setRetryToken] = useState(0);
  samplesRef.current = samples;
  onSampleSelectRef.current = onSampleSelect;
  const selectedIds = selectedItems.map((item) => item.id);
  const hasData = selectedItems.some((item) => hasMetricData(item.id, samples));

  useEffect(() => {
    const chart = lifecycle.get() as ECharts | null;
    if (!chart) return;
    const zr = chart.getZr();
    if (!zr) return;
    const selectNearestSample = (event: { offsetX?: number; offsetY?: number }) => {
      if (event.offsetX == null || event.offsetY == null) return;
      const point = [event.offsetX, event.offsetY];
      if (!chart.containPixel({ gridIndex: 0 }, point)) return;
      const converted = chart.convertFromPixel({ xAxisIndex: 0 }, point) as number | number[];
      const timestamp = Array.isArray(converted) ? converted[0] : converted;
      if (!Number.isFinite(timestamp) || !samplesRef.current.length) return;
      const nearest = samplesRef.current.reduce((best, sample) => Math.abs(sample.timestampMs - timestamp) < Math.abs(best.timestampMs - timestamp) ? sample : best);
      onSampleSelectRef.current(nearest);
    };
    zr.on("click", selectNearestSample);
    return () => zr.off("click", selectNearestSample);
  }, [lifecycle]);

  useEffect(() => {
    if (!selectedIds.length || !hasData) return;
    try {
      lifecycle.update(buildDashboardChartOption({
        samples,
        gaps,
        metricIds: selectedIds,
        startMs,
        endMs,
        selectedTimestampMs,
        language,
        palette: chartPalette(),
        metricLabel: (descriptor) => {
          const item = catalog.find((candidate) => candidate.id === descriptor.id);
          return item ? metricItemDisplayName(item, t, samples) : descriptor.id;
        },
        missingLabel: t("missingData")
      }));
      setChartError(false);
    } catch (error) {
      setChartError(true);
      console.error("[dashboard-trend] chart option failed", error);
    }
  }, [catalog, endMs, gaps, hasData, language, lifecycle, retryToken, samples, selectedIds, selectedTimestampMs, startMs, t, resolvedTheme]);

  const selected = selectedTimestampMs == null ? null : samples.find((sample) => sample.timestampMs === selectedTimestampMs) ?? null;
  const summary = selected ? selectedSummary(selected, selectedItems, language, t) : t("clickTimelineHint");
  const chartCanvas = <div ref={ref} role="group" tabIndex={0} aria-label={t("dashboardTrendEyebrow")} aria-describedby={summaryId} onKeyDown={(event) => {
    if ((event.key === "Enter" || event.key === " ") && samples.length) {
      event.preventDefault();
      onSampleSelect(samples[samples.length - 1]);
    }
  }} className={`${selectedIds.length && hasData && !chartError ? "h-[340px]" : "h-0 overflow-hidden"} w-full cursor-crosshair outline-none focus-visible:ring-2 focus-visible:ring-ring/35`} />;
  let message = <></>;
  if (!selectedIds.length) message = <TrendMessage icon={<LineChart size={18} aria-hidden="true" />} title={t("dashboardTrendNoSelection")} hint={t("dashboardTrendSelectMetric")} />;
  else if (chartError) message = <div className="flex flex-wrap items-center justify-between gap-3 rounded-lg border border-[hsl(var(--danger)/0.35)] bg-[hsl(var(--danger-surface))] px-4 py-4 text-sm"><div className="flex items-start gap-2"><AlertTriangle size={17} className="mt-0.5 shrink-0 text-[hsl(var(--danger))]" aria-hidden="true" /><span>{t("dashboardChartError")}</span></div><Button type="button" variant="outline" className="h-8 px-2.5 text-xs" onClick={() => setRetryToken((value) => value + 1)}><RotateCcw size={13} aria-hidden="true" />{t("dashboardRetry")}</Button></div>;
  else if (!hasData) message = <TrendStatusMessage items={selectedItems} />;
  return <div className="space-y-2">
    {chartCanvas}
    {message}
    <p id={summaryId} className="sr-only">{summary}</p>
  </div>;
}

function TrendStatusMessage({ items }: { items: MetricCatalogItem[] }) {
  const { t } = useI18n();
  const status = items.some((item) => item.status === "FAILED") ? "FAILED"
    : items.some((item) => item.status === "DISABLED") ? "DISABLED"
      : items.some((item) => item.status === "DEGRADED") ? "DEGRADED"
        : "NO_DATA_IN_RANGE";
  const hint = status === "FAILED" ? t("dashboardTrendFailedHint") : status === "DEGRADED" ? t("dashboardTrendDegradedHint") : status === "DISABLED" ? t("dashboardTrendUnavailableHint") : t("dashboardTrendNoDataHint");
  const title = status === "NO_DATA_IN_RANGE" ? t("dashboardTrendNoData") : status === "DISABLED" ? t("dashboardMetricStatusDisabled") : status === "FAILED" ? t("dashboardMetricStatusFailed") : t("dashboardMetricStatusDegraded");
  return <TrendMessage icon={status === "FAILED" || status === "DEGRADED" ? <AlertTriangle size={18} aria-hidden="true" /> : <CircleHelp size={18} aria-hidden="true" />} title={title} hint={hint} />;
}

function TrendMessage({ icon, title, hint }: { icon: React.ReactNode; title: string; hint: string }) {
  return <div className="flex items-start gap-3 rounded-lg border border-dashed border-border bg-muted/15 px-4 py-6 text-sm"><span className="mt-0.5 shrink-0 text-muted-foreground">{icon}</span><div><div className="font-medium text-foreground">{title}</div><div className="mt-1 text-xs text-muted-foreground">{hint}</div></div></div>;
}

function TrendEmpty({ onOpenExplorer }: { onOpenExplorer: () => void }) {
  const { t } = useI18n();
  return <div className="flex flex-wrap items-start justify-between gap-3 rounded-lg border border-dashed border-border bg-muted/15 px-4 py-6 text-sm"><div className="flex items-start gap-3"><CircleHelp size={18} className="mt-0.5 shrink-0 text-muted-foreground" aria-hidden="true" /><div><div className="font-medium text-foreground">{t("dashboardNoMetricsAvailable")}</div><div className="mt-1 text-xs text-muted-foreground">{t("dashboardMetricExplorerDescription")}</div></div></div><Button type="button" variant="outline" className="h-8 px-2.5 text-xs" onClick={onOpenExplorer}>{t("dashboardExploreMetrics")}</Button></div>;
}

function selectedSummary(sample: SystemSample, items: MetricCatalogItem[], language: "en" | "zh-CN", t: ReturnType<typeof useI18n>["t"]): string {
  const values = items.flatMap((item) => {
    const formatted = formatMetricValue(item.descriptor, metricValue(item.descriptor, sample), language);
    return formatted ? [`${metricItemDisplayName(item, t, [sample])} ${formatted}`] : [];
  });
  return `${t("selectedTimestamp", { time: formatClock(sample.timestampMs, language) })}. ${values.join(", ")}`;
}

function cssColor(name: string) {
  return `hsl(${getComputedStyle(document.documentElement).getPropertyValue(name).trim()})`;
}

function chartPalette() {
  const colors = ["--signal-cyan", "--signal-blue", "--signal-violet", "--signal-amber", "--signal-coral"].map(cssColor);
  return { colors, foreground: cssColor("--foreground"), mutedForeground: cssColor("--muted-foreground"), border: cssColor("--border"), muted: cssColor("--muted"), card: cssColor("--card"), selected: cssColor("--signal-coral") };
}
