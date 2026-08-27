import type { EChartsOption } from "echarts";
import type { Language, TranslationKey } from "../i18n";
import type { SystemSample, TimelineGap } from "../types/resource";
import { formatBytes, formatClock } from "../utils/time";
import {
  getMetricDescriptor,
  getAvailableMetricDescriptors,
  metricDisplayName,
  metricValue,
  type MetricDescriptor,
  type MetricId,
  type UnitFamily,
  SYSTEM_METRIC_IDS
} from "./metrics";
import { timelineChartSamples } from "../utils/uiSemantics";

export type ChartPalette = {
  colors: string[];
  foreground: string;
  mutedForeground: string;
  border: string;
  muted: string;
  card: string;
  selected: string;
};

export type MetricLabel = (descriptor: MetricDescriptor) => string;

type TooltipItem = {
  axisValue?: unknown;
  axisValueLabel?: string;
  marker?: string;
  seriesId?: string;
  seriesName?: string;
  value?: unknown;
};

type ChartLineStyle = {
  color: string;
  width: number;
  opacity: 1;
  type?: "solid" | "dashed";
};

export type LineVisibilityOptions = {
  lineStyle: ChartLineStyle;
  emphasis: { lineStyle: ChartLineStyle };
  blur: { lineStyle: ChartLineStyle };
  select: { lineStyle: ChartLineStyle };
};

type LineSeries = {
  id: string;
  name: string;
  type: "line";
  yAxisIndex?: number;
  showSymbol: false;
  showAllSymbol: false;
  symbol: "circle";
  symbolSize: number;
  connectNulls: false;
  lineStyle: ChartLineStyle;
  emphasis: {
    focus: "none";
    scale: true;
    lineStyle: ChartLineStyle;
    itemStyle: { borderWidth: number; borderColor: string };
  };
  blur: { lineStyle: ChartLineStyle };
  select: { lineStyle: ChartLineStyle };
  data: Array<[number, number | null]>;
  markLine?: {
    symbol: "none";
    silent: true;
    label: { show: false };
    lineStyle: { color: string; width: number; type: "solid" | "dashed" };
    data: Array<{ xAxis: number }>;
  };
};

export function formatTooltipItems({
  parameters,
  descriptors,
  language,
  missingLabel
}: {
  parameters: unknown;
  descriptors: ReadonlyMap<string, MetricDescriptor>;
  language: Language;
  missingLabel: string;
}): string {
  const items = (Array.isArray(parameters) ? parameters : [parameters]).filter(Boolean) as TooltipItem[];
  const timestampValue = items[0]?.axisValue;
  const timestamp = toFiniteNumber(timestampValue);
  const timestampText = timestamp == null ? items[0]?.axisValueLabel ?? "" : formatClock(timestamp, language);
  const lines = items.flatMap((item) => {
    const raw = valueFromTooltip(item.value);
    if (raw == null) return [];
    const descriptor = (item.seriesId ? descriptors.get(item.seriesId) : undefined)
      ?? (item.seriesName ? [...descriptors.values()].find((candidate) => candidate.translationKey === item.seriesName) : undefined);
    const value = descriptor ? descriptor.formatter(raw, language) : String(raw);
    return [`${item.marker ?? ""}${escapeHtml(item.seriesName ?? "")}: ${escapeHtml(value)}`];
  });
  if (!lines.length) lines.push(missingLabel);
  return [`<strong>${escapeHtml(timestampText)}</strong>`, ...lines].join("<br/>\n");
}

export function buildDashboardChartOption({
  samples,
  gaps,
  metricIds,
  startMs,
  endMs,
  selectedTimestampMs,
  language,
  palette,
  metricLabel,
  missingLabel
}: {
  samples: readonly SystemSample[];
  gaps: readonly TimelineGap[];
  metricIds: readonly MetricId[];
  startMs: number;
  endMs: number;
  selectedTimestampMs: number | null;
  language: Language;
  palette: ChartPalette;
  metricLabel: MetricLabel;
  missingLabel: string;
}): EChartsOption {
  const chartSamples = timelineChartSamples(samples, gaps);
  const descriptors = metricIds.map((id) => getMetricDescriptor(id)).filter((descriptor): descriptor is MetricDescriptor => descriptor != null);
  const descriptorMap = new Map(descriptors.map((descriptor) => [descriptor.id, descriptor]));
  const family = descriptors[0]?.unitFamily ?? "percent";
  const series = descriptors.map((descriptor, index) => buildLineSeries({
    descriptor,
    samples: chartSamples,
    name: metricLabel(descriptor),
    color: palette.colors[index % Math.max(palette.colors.length, 1)] ?? palette.selected,
    selectedTimestampMs: index === 0 ? selectedTimestampMs : null,
    selectedColor: palette.selected,
    dashed: false
  }));
  const tooltip = buildTooltip(descriptorMap, language, palette, missingLabel);
  return {
    animation: false,
    color: palette.colors,
    textStyle: { fontFamily: '"Segoe UI Variable Text", "Segoe UI", sans-serif', color: palette.mutedForeground },
    tooltip,
    legend: { top: 0, type: "scroll", itemWidth: 18, itemHeight: 3, icon: "roundRect", textStyle: { color: palette.mutedForeground, fontSize: 11 } },
    grid: { left: 58, right: 24, top: 44, bottom: 50 },
    dataZoom: [{ type: "inside", filterMode: "none", zoomOnMouseWheel: false, moveOnMouseMove: false, moveOnMouseWheel: false }],
    xAxis: { type: "time", min: startMs, max: endMs, axisLine: { lineStyle: { color: palette.border } }, axisTick: { show: false }, axisLabel: { color: palette.mutedForeground, fontSize: 11 }, splitLine: { show: false } },
    yAxis: [buildYAxis(family, language, palette)],
    series
  } as EChartsOption;
}

export function buildTimelineChartOption({
  samples,
  gaps,
  startMs,
  endMs,
  selectedTimestampMs,
  language,
  palette,
  metricLabel,
  missingLabel,
  metricIds
}: {
  samples: readonly SystemSample[];
  gaps: readonly TimelineGap[];
  startMs: number;
  endMs: number;
  selectedTimestampMs: number | null;
  language: Language;
  palette: ChartPalette;
  metricLabel: MetricLabel;
  missingLabel: string;
  metricIds?: readonly MetricId[];
}): EChartsOption {
  const chartSamples = timelineChartSamples(samples, gaps);
  const ids = metricIds ? [...metricIds] : timelineMetricIds(samples);
  const descriptors = ids.map((id) => getMetricDescriptor(id)).filter((descriptor): descriptor is MetricDescriptor => descriptor != null);
  const descriptorMap = new Map(descriptors.map((descriptor) => [descriptor.id, descriptor]));
  const series = descriptors.map((descriptor, index) => buildLineSeries({
    descriptor,
    samples: chartSamples,
    name: metricLabel(descriptor),
    color: palette.colors[index % Math.max(palette.colors.length, 1)] ?? palette.selected,
    selectedTimestampMs: index === 0 ? selectedTimestampMs : null,
    selectedColor: palette.selected,
    yAxisIndex: descriptor.unitFamily === "throughput" ? 1 : 0,
    dashed: descriptor.dimension === "gpu"
  }));
  return {
    animation: false,
    color: palette.colors,
    textStyle: { fontFamily: '"Segoe UI Variable Text", "Segoe UI", sans-serif', color: palette.mutedForeground },
    tooltip: buildTooltip(descriptorMap, language, palette, missingLabel),
    legend: { top: 0, type: "scroll", itemWidth: 18, itemHeight: 3, icon: "roundRect", textStyle: { color: palette.mutedForeground, fontSize: 11 } },
    grid: { left: 58, right: 34, top: 48, bottom: 62 },
    dataZoom: [{ type: "inside", filterMode: "none", zoomOnMouseWheel: false, moveOnMouseMove: false, moveOnMouseWheel: false }, { type: "slider", height: 18, borderColor: "transparent", backgroundColor: palette.muted, fillerColor: palette.colors[1] ?? palette.selected, opacity: 0.28, handleStyle: { color: palette.colors[1] ?? palette.selected, borderColor: palette.colors[1] ?? palette.selected } }],
    xAxis: { type: "time", min: startMs, max: endMs, axisLine: { lineStyle: { color: palette.border } }, axisTick: { show: false }, axisLabel: { color: palette.mutedForeground, fontSize: 11 }, splitLine: { show: false } },
    yAxis: [
      buildYAxis("percent", language, palette),
      buildYAxis("throughput", language, palette)
    ],
    series
  } as EChartsOption;
}

export function timelineMetricIds(samples: readonly SystemSample[]): MetricId[] {
  const ids: MetricId[] = [
    SYSTEM_METRIC_IDS.cpuUsage,
    SYSTEM_METRIC_IDS.memoryUsage,
    SYSTEM_METRIC_IDS.diskRead,
    SYSTEM_METRIC_IDS.diskWrite
  ];
  const gpuIds = getAvailableMetricDescriptors(samples)
    .filter((descriptor) => descriptor.dimension === "gpu" && descriptor.gpuField === "utilization_pct")
    .map((descriptor) => descriptor.id);
  ids.push(...gpuIds);
  return ids;
}

function buildLineSeries({
  descriptor,
  samples,
  name,
  color,
  selectedTimestampMs,
  selectedColor,
  yAxisIndex,
  dashed
}: {
  descriptor: MetricDescriptor;
  samples: readonly SystemSample[];
  name: string;
  color: string;
  selectedTimestampMs: number | null;
  selectedColor: string;
  yAxisIndex?: number;
  dashed: boolean;
}): LineSeries {
  const lineWidth = descriptor.dimension === "gpu" ? 1.6 : 1.9;
  const lineVisibility = buildLineVisibilityOptions({ color, width: lineWidth, dashed });
  return {
    id: descriptor.id,
    name,
    type: "line",
    ...(yAxisIndex == null ? {} : { yAxisIndex }),
    showSymbol: false,
    showAllSymbol: false,
    symbol: "circle",
    symbolSize: 7,
    connectNulls: false,
    ...lineVisibility,
    emphasis: { ...lineVisibility.emphasis, focus: "none", scale: true, itemStyle: { borderWidth: 2, borderColor: selectedColor } },
    data: samples.map((sample) => [sample.timestampMs, metricValue(descriptor, sample)]),
    markLine: selectedTimestampMs == null
      ? { symbol: "none", silent: true, label: { show: false }, lineStyle: { color: selectedColor, width: 2, type: "solid" }, data: [] }
      : { symbol: "none", silent: true, label: { show: false }, lineStyle: { color: selectedColor, width: 2, type: "solid" }, data: [{ xAxis: selectedTimestampMs }] }
  };
}

export function buildLineVisibilityOptions({ color, width, dashed = false }: { color: string; width: number; dashed?: boolean }): LineVisibilityOptions {
  const normal: ChartLineStyle = { color, width, opacity: 1, ...(dashed ? { type: "dashed" } : {}) };
  return {
    lineStyle: { ...normal },
    emphasis: { lineStyle: { ...normal } },
    blur: { lineStyle: { ...normal } },
    select: { lineStyle: { ...normal } }
  };
}

function buildTooltip(descriptors: ReadonlyMap<string, MetricDescriptor>, language: Language, palette: ChartPalette, missingLabel: string) {
  return {
    trigger: "axis" as const,
    triggerOn: "mousemove|click",
    backgroundColor: palette.card,
    borderColor: palette.border,
    borderWidth: 1,
    padding: [10, 12],
    textStyle: { color: palette.foreground, fontSize: 12 },
    extraCssText: "box-shadow: 0 18px 48px rgba(0,0,0,.24); border-radius: 8px;",
    axisPointer: { type: "line" as const, lineStyle: { color: palette.selected, type: "dashed" as const } },
    formatter: (parameters: unknown) => formatTooltipItems({ parameters, descriptors, language, missingLabel })
  };
}

function buildYAxis(family: UnitFamily, language: Language, palette: ChartPalette) {
  return {
    type: "value" as const,
    name: family === "percent" ? "%" : family === "throughput" ? "B/s" : family === "temperature" ? "°C" : family === "power" ? "W" : family === "frequency" ? "MHz" : "bytes",
    min: family === "percent" ? 0 : undefined,
    max: family === "percent" ? 100 : undefined,
    nameTextStyle: { color: palette.mutedForeground },
    axisLine: { show: false },
    axisTick: { show: false },
    axisLabel: { color: palette.mutedForeground, fontSize: 11, formatter: (value: number) => family === "percent" ? `${value}%` : family === "throughput" || family === "bytes" ? formatBytes(value, language) : String(value) },
    splitLine: { lineStyle: { color: palette.border, type: "dashed" as const } }
  };
}

function valueFromTooltip(value: unknown): number | null {
  const raw = Array.isArray(value) ? value[1] : value;
  return toFiniteNumber(raw);
}

function toFiniteNumber(value: unknown): number | null {
  if (typeof value === "number" && Number.isFinite(value)) return value;
  if (typeof value === "string" && value.trim() !== "" && Number.isFinite(Number(value))) return Number(value);
  return null;
}

function escapeHtml(value: string): string {
  return value.replace(/[&<>"']/g, (character) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[character] ?? character);
}

export function defaultMetricLabel(descriptor: MetricDescriptor, t: (key: TranslationKey) => string, samples: readonly SystemSample[] = []): string {
  return metricDisplayName(descriptor, t, samples);
}
