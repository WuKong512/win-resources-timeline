import { Component, useEffect, useId, useMemo, useRef, useState, type ReactNode } from "react";
import type { ECharts } from "echarts";
import { useI18n } from "../i18n";
import { buildDashboardChartOption } from "../dashboard/chartOptions";
import {
  formatMetricValue,
  getMetricDescriptor,
  hasMetricData,
  metricDisplayName,
  metricValue,
  type MetricDescriptor
} from "../dashboard/metrics";
import type { DashboardCardConfig } from "../dashboard/config";
import type { SystemSample, TimelineGap } from "../types/resource";
import { formatBytes, formatClock } from "../utils/time";
import { useUiStore } from "../stores/uiStore";
import { Badge } from "./ui/Badge";
import { Button } from "./ui/Button";
import { Card, CardContent, CardHeader, CardTitle } from "./ui/Card";
import { useStableEcharts } from "./chartLifecycle";

type DashboardChartCardProps = {
  card: DashboardCardConfig;
  samples: SystemSample[];
  gaps: TimelineGap[];
  startMs: number;
  endMs: number;
  selectedTimestampMs: number | null;
  onSampleSelect: (sample: SystemSample) => void;
};

export function DashboardChartCard(props: DashboardChartCardProps) {
  const { t } = useI18n();
  return <DashboardCardErrorBoundary title={t("dashboardChartError")} retryLabel={t("dashboardRetry")}>
    <DashboardChartCardBody {...props} />
  </DashboardCardErrorBoundary>;
}

function DashboardChartCardBody({ card, samples, gaps, startMs, endMs, selectedTimestampMs, onSampleSelect }: DashboardChartCardProps) {
  const { language, t } = useI18n();
  const resolvedTheme = useUiStore((state) => state.resolvedTheme);
  const ref = useRef<HTMLDivElement | null>(null);
  const lifecycle = useStableEcharts(ref);
  const samplesRef = useRef(samples);
  const onSampleSelectRef = useRef(onSampleSelect);
  const summaryId = `dashboard-card-summary-${useId().replace(/:/g, "")}`;
  const [chartError, setChartError] = useState(false);
  const [retryToken, setRetryToken] = useState(0);
  samplesRef.current = samples;
  onSampleSelectRef.current = onSampleSelect;

  const visibleMetricIds = useMemo(() => card.metricIds.filter((id) => !card.hiddenMetricIds.includes(id)), [card.hiddenMetricIds, card.metricIds]);
  const descriptors = useMemo(() => visibleMetricIds
    .map((id) => getMetricDescriptor(id))
    .filter((descriptor): descriptor is MetricDescriptor => descriptor != null), [visibleMetricIds]);
  const unavailableDescriptors = descriptors.filter((descriptor) => !hasMetricData(descriptor.id, samples));
  const hasVisibleData = descriptors.some((descriptor) => hasMetricData(descriptor.id, samples));

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
    if (!descriptors.length || !hasVisibleData) return;
    try {
      lifecycle.update(buildDashboardChartOption({
        samples,
        gaps,
        metricIds: descriptors.map((descriptor) => descriptor.id),
        startMs,
        endMs,
        selectedTimestampMs,
        language,
        palette: chartPalette(),
        metricLabel: (descriptor) => metricDisplayName(descriptor, t, samples),
        missingLabel: t("missingData")
      }));
      setChartError(false);
    } catch (error) {
      setChartError(true);
      console.error("[dashboard-card] chart option failed", error);
    }
  }, [descriptors, endMs, gaps, hasVisibleData, language, lifecycle, resolvedTheme, retryToken, samples, selectedTimestampMs, startMs, t]);

  const title = dashboardCardTitle(card, descriptors, samples, t);
  const latest = samples[samples.length - 1];
  const selected = selectedTimestampMs == null ? null : samples.find((sample) => sample.timestampMs === selectedTimestampMs) ?? null;
  const summary = selected ? selectedSummary(selected, descriptors, language, t) : t("clickTimelineHint");

  const chartCanvas = <div ref={ref} role="group" tabIndex={0} aria-label={title} aria-describedby={summaryId} onKeyDown={(event) => {
    if ((event.key === "Enter" || event.key === " ") && samples.length) {
      event.preventDefault();
      onSampleSelect(samples[samples.length - 1]);
    }
  }} className={`${hasVisibleData && !chartError && visibleMetricIds.length ? "h-[250px]" : "h-0 overflow-hidden"} w-full cursor-crosshair outline-none focus-visible:ring-2 focus-visible:ring-ring/35`} />;
  const cardMessage = !visibleMetricIds.length
    ? <div className="py-8 text-sm text-muted-foreground">{t("dashboardNoVisibleMetrics")}</div>
    : !descriptors.length
      ? <div className="py-8 text-sm text-muted-foreground">{t("dashboardMetricUnavailable")}</div>
      : !hasVisibleData
        ? <div className="py-8 text-sm text-muted-foreground">{unavailableDescriptors.map((descriptor) => metricDisplayName(descriptor, t, samples)).join(" · ")}</div>
        : chartError
          ? <div className="flex flex-wrap items-center justify-between gap-3 py-8 text-sm text-muted-foreground"><span>{t("dashboardChartError")}</span><Button variant="outline" className="h-8 px-2.5 text-xs" onClick={() => setRetryToken((value) => value + 1)}>{t("dashboardRetry")}</Button></div>
          : null;

  return <Card className="overflow-hidden">
    <CardHeader className="border-b border-border/70 bg-card/90">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="min-w-0">
          <CardTitle>{title}</CardTitle>
          <p className="mt-1 truncate text-xs font-normal text-muted-foreground">{latest ? t("lastSampleAt", { time: formatClock(latest.timestampMs, language) }) : t("dashboardMetricUnavailable")}</p>
        </div>
        <div className="flex flex-wrap items-center justify-end gap-1.5">
          {unavailableDescriptors.length > 0 && <Badge className="border-border bg-muted text-muted-foreground">{t("dashboardSomeMetricsUnavailable")}</Badge>}
          {!hasVisibleData && descriptors.length > 0 && <Badge className="border-border bg-muted text-muted-foreground">{t("dashboardMetricUnavailable")}</Badge>}
          {chartError && <Badge className="border-[hsl(var(--danger)/0.35)] bg-[hsl(var(--danger-surface))] text-[hsl(var(--danger))]">{t("dashboardChartError")}</Badge>}
          {card.id === "memory" && latest?.memoryUsedBytes != null && latest.memoryTotalBytes != null && <Badge className="border-border bg-card text-muted-foreground">{formatBytes(latest.memoryUsedBytes, language)} / {formatBytes(latest.memoryTotalBytes, language)}</Badge>}
        </div>
      </div>
    </CardHeader>
    <CardContent className="pt-3">
      {chartCanvas}
      {cardMessage}
      <p id={summaryId} className="sr-only">{summary}</p>
    </CardContent>
  </Card>;
}

function chartPalette() {
  const cssColor = (name: string) => `hsl(${getComputedStyle(document.documentElement).getPropertyValue(name).trim()})`;
  const colors = ["--signal-cyan", "--signal-blue", "--signal-violet", "--signal-amber", "--signal-coral"].map(cssColor);
  return { colors, foreground: cssColor("--foreground"), mutedForeground: cssColor("--muted-foreground"), border: cssColor("--border"), muted: cssColor("--muted"), card: cssColor("--card"), selected: cssColor("--signal-coral") };
}

function dashboardCardTitle(card: DashboardCardConfig, descriptors: MetricDescriptor[], samples: SystemSample[], t: ReturnType<typeof useI18n>["t"]): string {
  if (card.id === "compute-usage") return t("dashboardComputeUsage");
  if (card.id === "memory") return t("dashboardMemory");
  if (card.id === "disk-io") return t("dashboardDiskIo");
  if (card.id === "gpu-temperature") return t("dashboardGpuTemperature");
  return descriptors[0] ? metricDisplayName(descriptors[0], t, samples) : t("dashboardTitle");
}

function selectedSummary(sample: SystemSample, descriptors: MetricDescriptor[], language: "en" | "zh-CN", t: ReturnType<typeof useI18n>["t"]): string {
  const values = descriptors.flatMap((descriptor) => {
    const formatted = formatMetricValue(descriptor, metricValue(descriptor, sample), language);
    return formatted ? [`${metricDisplayName(descriptor, t)} ${formatted}`] : [];
  });
  return `${t("selectedTimestamp", { time: formatClock(sample.timestampMs, language) })}. ${values.join(", ")}`;
}

class DashboardCardErrorBoundary extends Component<{ children: ReactNode; title: string; retryLabel: string }, { hasError: boolean }> {
  state = { hasError: false };

  static getDerivedStateFromError() {
    return { hasError: true };
  }

  componentDidCatch(error: unknown) {
    console.error("[dashboard-card] render failed", error);
  }

  render() {
    if (!this.state.hasError) return this.props.children;
    return <Card role="alert"><CardContent className="flex flex-wrap items-center justify-between gap-3 py-8 text-sm"><span>{this.props.title}</span><Button variant="outline" className="h-8 px-2.5 text-xs" onClick={() => this.setState({ hasError: false })}>{this.props.retryLabel}</Button></CardContent></Card>;
  }
}
