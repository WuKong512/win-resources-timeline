import { useEffect, useRef } from "react";
import * as echarts from "echarts";
import type { SystemSample } from "../types/resource";
import { useI18n } from "../i18n";
import { formatBytes, formatClock } from "../utils/time";
import { useUiStore } from "../stores/uiStore";

function cssColor(name: string) {
  return `hsl(${getComputedStyle(document.documentElement).getPropertyValue(name).trim()})`;
}

export function ResourceLineChart({ samples, selectedTimestampMs, onSampleSelect }: {
  samples: SystemSample[];
  selectedTimestampMs: number | null;
  onSampleSelect: (sample: SystemSample) => void;
}) {
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
    const colors = ["--signal-cyan", "--signal-blue", "--signal-violet", "--signal-amber"].map(cssColor);
    const chartSamples = samples.flatMap((sample, index) => {
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
        diskWriteBytesPerSec: null
      }, sample];
    });
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
            const value = typeof raw === "number"
              ? item.seriesName === "CPU" || item.seriesName === t("memory")
                ? `${raw.toFixed(1)}%`
                : `${formatBytes(raw, language)}/s`
              : t("noSample");
            return `${item.marker ?? ""}${item.seriesName ?? ""}: ${value}`;
          });
          return [`<strong>${typeof timestamp === "number" ? formatClock(timestamp, language) : ""}</strong>`, ...lines].join("<br/>");
        }
      },
      legend: { top: 0, itemWidth: 18, itemHeight: 3, icon: "roundRect", textStyle: { color: mutedForeground, fontSize: 11 } },
      grid: { left: 52, right: 72, top: 44, bottom: 54 },
      dataZoom: [
        { type: "inside" },
        { type: "slider", height: 18, borderColor: "transparent", backgroundColor: muted, fillerColor: colors[1], opacity: 0.28, handleStyle: { color: colors[1], borderColor: colors[1] }, dataBackground: { lineStyle: { color: mutedForeground }, areaStyle: { color: border } } }
      ],
      xAxis: { type: "time", axisLine: { lineStyle: { color: border } }, axisTick: { show: false }, axisLabel: { color: mutedForeground, fontSize: 11 }, splitLine: { show: false } },
      yAxis: [
        { type: "value", name: "%", min: 0, max: 100, nameTextStyle: { color: mutedForeground }, axisLine: { show: false }, axisTick: { show: false }, axisLabel: { color: mutedForeground, fontSize: 11 }, splitLine: { lineStyle: { color: border, type: "dashed" } } },
        { type: "value", name: "B/s", nameTextStyle: { color: mutedForeground }, axisLine: { show: false }, axisTick: { show: false }, axisLabel: { color: mutedForeground, fontSize: 11, formatter: (v: number) => formatBytes(v, language) }, splitLine: { show: false } }
      ],
      series: [
        { name: "CPU", type: "line", showSymbol: false, connectNulls: false, lineStyle: { width: 2 }, emphasis: { focus: "series" }, markLine: selectedTimestampMs == null ? undefined : { symbol: "none", silent: true, label: { show: false }, lineStyle: { color: "#26332f", width: 1.5, type: "dashed" }, data: [{ xAxis: selectedTimestampMs }] }, data: chartSamples.map((s) => [s.timestampMs, s.cpuPercent]) },
        { name: t("memory"), type: "line", showSymbol: false, connectNulls: false, lineStyle: { width: 1.8 }, emphasis: { focus: "series" }, data: chartSamples.map((s) => [s.timestampMs, s.memoryPercent]) },
        { name: t("diskRead"), type: "line", yAxisIndex: 1, showSymbol: false, connectNulls: false, lineStyle: { width: 1.5 }, emphasis: { focus: "series" }, data: chartSamples.map((s) => [s.timestampMs, s.diskReadBytesPerSec]) },
        { name: t("diskWrite"), type: "line", yAxisIndex: 1, showSymbol: false, connectNulls: false, lineStyle: { width: 1.5 }, emphasis: { focus: "series" }, data: chartSamples.map((s) => [s.timestampMs, s.diskWriteBytesPerSec]) }
      ]
    });
    const selectNearestSample = (event: { offsetX: number; offsetY: number }) => {
      const point = [event.offsetX, event.offsetY];
      if (!chart.containPixel({ gridIndex: 0 }, point)) return;
      const converted = chart.convertFromPixel({ xAxisIndex: 0 }, point) as number | number[];
      const timestamp = Array.isArray(converted) ? converted[0] : converted;
      if (!Number.isFinite(timestamp)) return;
      const nearest = samples.reduce((best, sample) =>
        Math.abs(sample.timestampMs - timestamp) < Math.abs(best.timestampMs - timestamp) ? sample : best
      );
      onSampleSelect(nearest);
    };
    chart.getZr().on("click", selectNearestSample);
    const selectSeriesSample = (parameters: { value?: unknown }) => {
      const value = parameters.value;
      const timestamp = Array.isArray(value) ? value[0] : null;
      if (typeof timestamp !== "number") return;
      const selected = samples.find((sample) => sample.timestampMs === timestamp);
      if (selected) onSampleSelect(selected);
    };
    chart.on("click", selectSeriesSample);
    const resize = () => chart.resize(); window.addEventListener("resize", resize);
    return () => { chart.getZr().off("click", selectNearestSample); chart.off("click", selectSeriesSample); window.removeEventListener("resize", resize); chart.dispose(); chartRef.current = null; };
  }, [language, onSampleSelect, samples, t, resolvedTheme]);
  useEffect(() => {
    chartRef.current?.setOption({
      series: [{
        name: "CPU",
        markLine: selectedTimestampMs == null
          ? { data: [] }
          : { symbol: "none", silent: true, label: { show: false }, lineStyle: { color: cssColor("--signal-violet"), width: 2 }, data: [{ xAxis: selectedTimestampMs }] }
      }]
    });
  }, [selectedTimestampMs]);
  return <div ref={ref} className="h-[430px] w-full cursor-crosshair" />;
}
