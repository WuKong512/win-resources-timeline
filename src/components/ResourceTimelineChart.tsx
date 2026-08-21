import { useEffect, useRef } from "react";
import * as echarts from "echarts";
import { useI18n } from "../i18n";
import type { GpuSample, SystemSample } from "../types/resource";
import { formatBytes, formatClock } from "../utils/time";
import { useUiStore } from "../stores/uiStore";

type ResourceTimelineChartProps = {
  samples: SystemSample[];
  selectedTimestampMs: number | null;
  onSampleSelect: (sample: SystemSample) => void;
  ariaLabel: string;
};

function cssColor(name: string) {
  return `hsl(${getComputedStyle(document.documentElement).getPropertyValue(name).trim()})`;
}

function gapSamples(samples: SystemSample[]) {
  return samples.flatMap((sample, index) => {
    if (index === 0) return [sample];
    const previous = samples[index - 1];
    const gapThreshold = Math.max(15_000, previous.sampleDurationMs * 3);
    if (sample.timestampMs - previous.timestampMs <= gapThreshold) return [sample];
    return [{
      ...sample,
      timestampMs: previous.timestampMs + previous.sampleDurationMs,
      cpuPercent: null,
      memoryPercent: null,
      memoryUsedBytes: null,
      memoryTotalBytes: null,
      diskReadBytesPerSec: null,
      diskWriteBytesPerSec: null,
      gpus: []
    }, sample];
  });
}

function gpuLabel(gpu: GpuSample) {
  return [gpu.vendor, gpu.model].filter(Boolean).join(" ") || gpu.deviceKey;
}

export function ResourceTimelineChart({
  samples,
  selectedTimestampMs,
  onSampleSelect,
  ariaLabel
}: ResourceTimelineChartProps) {
  const { language, t } = useI18n();
  const resolvedTheme = useUiStore((state) => state.resolvedTheme);
  const ref = useRef<HTMLDivElement | null>(null);
  const chartRef = useRef<echarts.ECharts | null>(null);

  useEffect(() => {
    if (!ref.current) return;
    const chart = echarts.init(ref.current);
    chartRef.current = chart;
    const foreground = cssColor("--foreground");
    const mutedForeground = cssColor("--muted-foreground");
    const border = cssColor("--border");
    const muted = cssColor("--muted");
    const card = cssColor("--card");
    const colors = ["--signal-cyan", "--signal-blue", "--signal-violet", "--signal-amber", "--signal-coral"].map(cssColor);
    const chartSamples = gapSamples(samples);
    const devices = [...new Map(samples.flatMap((sample) => sample.gpus).map((gpu) => [gpu.deviceKey, gpu])).values()];
    const gpuSeries = devices.map((gpu, index) => ({
      name: `${t("metricGpuUsage")} · ${gpuLabel(gpu)}`,
      type: "line" as const,
      yAxisIndex: 0,
      showSymbol: false,
      connectNulls: false,
      lineStyle: { width: 1.6, type: index % 2 ? "dashed" as const : "solid" as const },
      emphasis: { focus: "series" as const },
      data: chartSamples.map((sample) => [sample.timestampMs, sample.gpus.find((item) => item.deviceKey === gpu.deviceKey)?.utilizationPercent ?? null])
    }));
    const seriesUnits = new Map<string, "percent" | "rate">([
      [t("metricCpu"), "percent"],
      [t("metricMemory"), "percent"],
      [t("metricDiskRead"), "rate"],
      [t("metricDiskWrite"), "rate"],
      ...gpuSeries.map((series) => [series.name, "percent"] as const)
    ]);
    chart.setOption({
      animation: false,
      color: colors,
      textStyle: { fontFamily: '"Segoe UI Variable Text", "Segoe UI", sans-serif', color: mutedForeground },
      tooltip: {
        trigger: "axis",
        backgroundColor: card,
        borderColor: border,
        borderWidth: 1,
        padding: [10, 12],
        textStyle: { color: foreground, fontSize: 12 },
        extraCssText: "box-shadow: 0 18px 48px rgba(0,0,0,.24); border-radius: 8px;",
        axisPointer: { type: "line", lineStyle: { color: colors[2], type: "dashed" } },
        formatter: (parameters: unknown) => {
          const items = (Array.isArray(parameters) ? parameters : [parameters]) as Array<{ axisValue?: number; marker?: string; seriesName?: string; value?: unknown }>;
          const timestamp = items[0]?.axisValue;
          const lines = items.map((item) => {
            const raw = Array.isArray(item.value) ? item.value[1] : item.value;
            if (typeof raw !== "number") return `${item.marker ?? ""}${item.seriesName ?? ""}: ${t("missingData")}`;
            const unit = seriesUnits.get(item.seriesName ?? "");
            return `${item.marker ?? ""}${item.seriesName ?? ""}: ${unit === "rate" ? `${formatBytes(raw, language)}/s` : `${raw.toFixed(1)}%`}`;
          });
          return [`<strong>${typeof timestamp === "number" ? formatClock(timestamp, language) : ""}</strong>`, ...lines].join("<br/>\n");
        }
      },
      legend: { top: 0, type: "scroll", itemWidth: 18, itemHeight: 3, icon: "roundRect", textStyle: { color: mutedForeground, fontSize: 11 } },
      grid: { left: 56, right: 78, top: 48, bottom: 62 },
      dataZoom: [
        { type: "inside" },
        { type: "slider", height: 18, borderColor: "transparent", backgroundColor: muted, fillerColor: colors[1], opacity: 0.28, handleStyle: { color: colors[1], borderColor: colors[1] }, dataBackground: { lineStyle: { color: mutedForeground }, areaStyle: { color: border } } }
      ],
      xAxis: { type: "time", axisLine: { lineStyle: { color: border } }, axisTick: { show: false }, axisLabel: { color: mutedForeground, fontSize: 11 }, splitLine: { show: false } },
      yAxis: [
        { type: "value", name: "%", min: 0, max: 100, nameTextStyle: { color: mutedForeground }, axisLine: { show: false }, axisTick: { show: false }, axisLabel: { color: mutedForeground, fontSize: 11 }, splitLine: { lineStyle: { color: border, type: "dashed" } } },
        { type: "value", name: "B/s", nameTextStyle: { color: mutedForeground }, axisLine: { show: false }, axisTick: { show: false }, axisLabel: { color: mutedForeground, fontSize: 11, formatter: (value: number) => formatBytes(value, language) }, splitLine: { show: false } }
      ],
      series: [
        { name: t("metricCpu"), type: "line", showSymbol: false, connectNulls: false, lineStyle: { width: 2 }, emphasis: { focus: "series" }, data: chartSamples.map((sample) => [sample.timestampMs, sample.cpuPercent]) },
        { name: t("metricMemory"), type: "line", showSymbol: false, connectNulls: false, lineStyle: { width: 1.8 }, emphasis: { focus: "series" }, data: chartSamples.map((sample) => [sample.timestampMs, sample.memoryPercent]) },
        { name: t("metricDiskRead"), type: "line", yAxisIndex: 1, showSymbol: false, connectNulls: false, lineStyle: { width: 1.5 }, emphasis: { focus: "series" }, data: chartSamples.map((sample) => [sample.timestampMs, sample.diskReadBytesPerSec]) },
        { name: t("metricDiskWrite"), type: "line", yAxisIndex: 1, showSymbol: false, connectNulls: false, lineStyle: { width: 1.5 }, emphasis: { focus: "series" }, data: chartSamples.map((sample) => [sample.timestampMs, sample.diskWriteBytesPerSec]) },
        ...gpuSeries
      ]
    });
    const selectNearestSample = (event: { offsetX: number; offsetY: number }) => {
      const point = [event.offsetX, event.offsetY];
      if (!chart.containPixel({ gridIndex: 0 }, point)) return;
      const converted = chart.convertFromPixel({ xAxisIndex: 0 }, point) as number | number[];
      const timestamp = Array.isArray(converted) ? converted[0] : converted;
      if (!Number.isFinite(timestamp)) return;
      const nearest = samples.reduce((best, sample) => Math.abs(sample.timestampMs - timestamp) < Math.abs(best.timestampMs - timestamp) ? sample : best);
      onSampleSelect(nearest);
    };
    const selectSeriesSample = (parameters: { value?: unknown }) => {
      const value = parameters.value;
      const timestamp = Array.isArray(value) ? value[0] : null;
      const selected = typeof timestamp === "number" ? samples.find((sample) => sample.timestampMs === timestamp) : undefined;
      if (selected) onSampleSelect(selected);
    };
    chart.getZr().on("click", selectNearestSample);
    chart.on("click", selectSeriesSample);
    const resize = () => chart.resize();
    window.addEventListener("resize", resize);
    return () => {
      chart.getZr().off("click", selectNearestSample);
      chart.off("click", selectSeriesSample);
      window.removeEventListener("resize", resize);
      chart.dispose();
      chartRef.current = null;
    };
  }, [language, onSampleSelect, resolvedTheme, samples, t]);

  useEffect(() => {
    chartRef.current?.setOption({
      series: [{
        name: t("metricCpu"),
        markLine: selectedTimestampMs == null
          ? { data: [] }
          : { symbol: "none", silent: true, label: { show: false }, lineStyle: { color: cssColor("--signal-coral"), width: 2 }, data: [{ xAxis: selectedTimestampMs }] }
      }]
    });
  }, [selectedTimestampMs, t]);

  return <div ref={ref} role="img" aria-label={ariaLabel} className="h-[390px] w-full cursor-crosshair" />;
}
