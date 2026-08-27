import { useEffect, useId, useRef } from "react";
import type { ECharts } from "echarts";
import { useI18n } from "../i18n";
import { buildTimelineChartOption } from "../dashboard/chartOptions";
import { formatMetricValue, getMetricDescriptor, metricDisplayName, metricValue, type MetricId } from "../dashboard/metrics";
import type { GpuSample, SystemSample, TimelineGap } from "../types/resource";
import { formatClock } from "../utils/time";
import { useStableEcharts } from "./chartLifecycle";
import { useUiStore } from "../stores/uiStore";

type ResourceTimelineChartProps = {
  samples: SystemSample[];
  gaps: TimelineGap[];
  startMs: number;
  endMs: number;
  selectedTimestampMs: number | null;
  onSampleSelect: (sample: SystemSample) => void;
  ariaLabel: string;
};

function cssColor(name: string) {
  return `hsl(${getComputedStyle(document.documentElement).getPropertyValue(name).trim()})`;
}

function chartPalette() {
  const colors = ["--signal-cyan", "--signal-blue", "--signal-violet", "--signal-amber", "--signal-coral"].map(cssColor);
  return {
    colors,
    foreground: cssColor("--foreground"),
    mutedForeground: cssColor("--muted-foreground"),
    border: cssColor("--border"),
    muted: cssColor("--muted"),
    card: cssColor("--card"),
    selected: cssColor("--signal-coral")
  };
}

export function ResourceTimelineChart({
  samples,
  gaps,
  startMs,
  endMs,
  selectedTimestampMs,
  onSampleSelect,
  ariaLabel
}: ResourceTimelineChartProps) {
  const { language, t } = useI18n();
  const resolvedTheme = useUiStore((state) => state.resolvedTheme);
  const ref = useRef<HTMLDivElement | null>(null);
  const lifecycle = useStableEcharts(ref);
  const samplesRef = useRef(samples);
  const onSampleSelectRef = useRef(onSampleSelect);
  const summaryId = `resource-timeline-summary-${useId().replace(/:/g, "")}`;
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
      gaps,
      startMs,
      endMs,
      selectedTimestampMs,
      language,
      palette: chartPalette(),
      metricLabel: (descriptor) => metricDisplayName(descriptor, t, samples),
      missingLabel: t("missingData")
    }));
  }, [endMs, gaps, language, lifecycle, resolvedTheme, samples, selectedTimestampMs, startMs, t]);

  const selected = selectedTimestampMs == null ? null : samples.find((sample) => sample.timestampMs === selectedTimestampMs) ?? null;
  const summary = selected ? selectedSampleSummary(selected, language, t) : t("clickTimelineHint");

  return <>
    <div
      ref={ref}
      role="group"
      tabIndex={0}
      aria-label={ariaLabel}
      aria-describedby={summaryId}
      onKeyDown={(event) => {
        if ((event.key === "Enter" || event.key === " ") && samples.length) {
          event.preventDefault();
          onSampleSelect(samples[samples.length - 1]);
        }
      }}
      className="h-[390px] w-full cursor-crosshair outline-none focus-visible:ring-2 focus-visible:ring-ring/35"
    />
    <p id={summaryId} className="sr-only">{summary}</p>
  </>;
}

function selectedSampleSummary(sample: SystemSample, language: "en" | "zh-CN", t: ReturnType<typeof useI18n>["t"]): string {
  const ids: MetricId[] = [
    "system.cpu.usage_pct",
    "system.memory.usage_pct",
    "system.disk.read_bps",
    "system.disk.write_bps"
  ];
  const values = ids.flatMap((id) => {
    const descriptor = getMetricDescriptor(id);
    const value = descriptor ? formatMetricValue(descriptor, metricValue(descriptor, sample), language) : null;
    return descriptor && value ? [`${t(descriptor.translationKey)} ${value}`] : [];
  });
  return `${t("selectedTimestamp", { time: formatClock(sample.timestampMs, language) })}. ${values.join(", ")}`;
}

export function gpuChartLabel(gpu: GpuSample) {
  return [gpu.vendor, gpu.model].filter(Boolean).join(" ") || gpu.deviceKey;
}
