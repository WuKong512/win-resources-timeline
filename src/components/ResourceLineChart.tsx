import { useEffect, useId, useRef } from "react";
import type { ECharts } from "echarts";
import { useI18n } from "../i18n";
import { buildTimelineChartOption } from "../dashboard/chartOptions";
import { formatMetricValue, getMetricDescriptor, metricDisplayName, metricValue, type MetricId } from "../dashboard/metrics";
import type { SystemSample } from "../types/resource";
import { formatClock } from "../utils/time";
import { useUiStore } from "../stores/uiStore";
import { inferSampleGaps } from "../utils/uiSemantics";
import { useStableEcharts } from "./chartLifecycle";

function cssColor(name: string) {
  return `hsl(${getComputedStyle(document.documentElement).getPropertyValue(name).trim()})`;
}

function chartPalette() {
  const colors = ["--signal-cyan", "--signal-blue", "--signal-violet", "--signal-amber"].map(cssColor);
  return {
    colors,
    foreground: cssColor("--foreground"),
    mutedForeground: cssColor("--muted-foreground"),
    border: cssColor("--border"),
    muted: cssColor("--muted"),
    card: cssColor("--card"),
    selected: cssColor("--signal-violet")
  };
}

const RESOURCE_METRIC_IDS: MetricId[] = [
  "system.cpu.usage_pct",
  "system.memory.usage_pct",
  "system.disk.read_bps",
  "system.disk.write_bps"
];

export function ResourceLineChart({ samples, selectedTimestampMs, onSampleSelect }: {
  samples: SystemSample[];
  selectedTimestampMs: number | null;
  onSampleSelect: (sample: SystemSample) => void;
}) {
  const { language, t } = useI18n();
  const resolvedTheme = useUiStore((state) => state.resolvedTheme);
  const ref = useRef<HTMLDivElement | null>(null);
  const lifecycle = useStableEcharts(ref);
  const samplesRef = useRef(samples);
  const onSampleSelectRef = useRef(onSampleSelect);
  const summaryId = `resource-line-summary-${useId().replace(/:/g, "")}`;
  samplesRef.current = samples;
  onSampleSelectRef.current = onSampleSelect;

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
    lifecycle.update(buildTimelineChartOption({
      samples,
      gaps: inferSampleGaps(samples),
      startMs: samples[0]?.timestampMs ?? 0,
      endMs: samples[samples.length - 1]?.timestampMs ?? 1,
      selectedTimestampMs,
      language,
      palette: chartPalette(),
      metricLabel: (descriptor) => metricDisplayName(descriptor, t, samples),
      missingLabel: t("missingData"),
      metricIds: RESOURCE_METRIC_IDS
    }));
  }, [language, lifecycle, resolvedTheme, samples, selectedTimestampMs, t]);

  const selected = selectedTimestampMs == null ? null : samples.find((sample) => sample.timestampMs === selectedTimestampMs) ?? null;
  const summary = selected ? selectedSampleSummary(selected, language, t) : t("clickSampleHint");
  return <>
    <div ref={ref} role="group" tabIndex={0} aria-label={t("resourceCharts")} aria-describedby={summaryId} className="h-[430px] w-full cursor-crosshair outline-none focus-visible:ring-2 focus-visible:ring-ring/35" />
    <p id={summaryId} className="sr-only">{summary}</p>
  </>;
}

function selectedSampleSummary(sample: SystemSample, language: "en" | "zh-CN", t: ReturnType<typeof useI18n>["t"]): string {
  const values = RESOURCE_METRIC_IDS.flatMap((id) => {
    const descriptor = getMetricDescriptor(id);
    const value = descriptor ? formatMetricValue(descriptor, metricValue(descriptor, sample), language) : null;
    return descriptor && value ? [`${t(descriptor.translationKey)} ${value}`] : [];
  });
  return `${t("selectedTimestamp", { time: formatClock(sample.timestampMs, language) })}. ${values.join(", ")}`;
}
