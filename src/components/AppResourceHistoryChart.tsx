import { useEffect, useRef } from "react";
import * as echarts from "echarts";
import { useI18n } from "../i18n";
import type { AppResourceHistoryPoint } from "../types/resource";
import { formatBytes, formatClock } from "../utils/time";
import { useUiStore } from "../stores/uiStore";
import { useStableEcharts } from "./chartLifecycle";

function cssColor(name: string) {
  return `hsl(${getComputedStyle(document.documentElement).getPropertyValue(name).trim()})`;
}

export function AppResourceHistoryChart({ points }: { points: AppResourceHistoryPoint[] }) {
  const { language, t } = useI18n();
  const resolvedTheme = useUiStore((state) => state.resolvedTheme);
  const ref = useRef<HTMLDivElement | null>(null);
  const lifecycle = useStableEcharts(ref);

  useEffect(() => {
    const foreground = cssColor("--foreground");
    const mutedForeground = cssColor("--muted-foreground");
    const border = cssColor("--border");
    const muted = cssColor("--muted");
    const card = cssColor("--card");
    const colors = ["--signal-cyan", "--signal-blue", "--signal-violet", "--signal-amber"].map(cssColor);
    const descriptors = new Map([
      ["app.cpu.usage_pct", { name: "CPU", format: (value: number) => `${value.toFixed(1)}%` }],
      ["app.memory.used_bytes", { name: t("memory"), format: (value: number) => formatBytes(value, language) }],
      ["app.io.read_bps", { name: t("ioRead"), format: (value: number) => `${formatBytes(value, language)}/s` }],
      ["app.io.write_bps", { name: t("ioWrite"), format: (value: number) => `${formatBytes(value, language)}/s` }]
    ]);
    lifecycle.update({
      animation: false,
      color: colors,
      textStyle: { fontFamily: '"Segoe UI Variable Text", "Segoe UI", sans-serif', color: mutedForeground },
      axisPointer: { link: [{ xAxisIndex: [0, 1] }] },
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
          const items = (Array.isArray(parameters) ? parameters : [parameters]) as Array<{ axisValue?: unknown; seriesId?: string; marker?: string; value?: unknown }>;
          const timestamp = typeof items[0]?.axisValue === "number" ? formatClock(items[0].axisValue, language) : "";
          const lines = items.flatMap((item) => {
            const raw = Array.isArray(item.value) ? item.value[1] : item.value;
            if (typeof raw !== "number" || !Number.isFinite(raw)) return [];
            const descriptor = item.seriesId ? descriptors.get(item.seriesId) : undefined;
            return [`${item.marker ?? ""}${descriptor?.name ?? ""}: ${descriptor?.format(raw) ?? String(raw)}`];
          });
          if (!lines.length) lines.push(t("missingData"));
          return [`<strong>${timestamp}</strong>`, ...lines].join("<br/>");
        }
      },
      legend: { top: 0, itemWidth: 18, itemHeight: 3, icon: "roundRect", textStyle: { color: mutedForeground, fontSize: 11 } },
      grid: [
        { left: 56, right: 78, top: 48, height: "36%" },
        { left: 56, right: 78, top: "58%", bottom: 58 }
      ],
      dataZoom: [
        { type: "inside", xAxisIndex: [0, 1], filterMode: "none", zoomOnMouseWheel: false, moveOnMouseMove: false, moveOnMouseWheel: false },
        { type: "slider", xAxisIndex: [0, 1], height: 18, bottom: 8, borderColor: "transparent", backgroundColor: muted, fillerColor: colors[1], opacity: 0.28, handleStyle: { color: colors[1], borderColor: colors[1] } }
      ],
      xAxis: [
        { type: "time", gridIndex: 0, axisLabel: { show: false }, axisLine: { lineStyle: { color: border } }, axisTick: { show: false }, splitLine: { show: false } },
        { type: "time", gridIndex: 1, axisLine: { lineStyle: { color: border } }, axisTick: { show: false }, axisLabel: { color: mutedForeground, fontSize: 11 }, splitLine: { show: false } }
      ],
      yAxis: [
        { type: "value", gridIndex: 0, name: "%", min: 0, axisLine: { show: false }, axisTick: { show: false }, axisLabel: { color: mutedForeground, fontSize: 11 }, splitLine: { lineStyle: { color: border, type: "dashed" } } },
        { type: "value", gridIndex: 0, name: t("memory"), position: "right", axisLine: { show: false }, axisTick: { show: false }, axisLabel: { color: mutedForeground, fontSize: 11, formatter: (value: number) => formatBytes(value, language) }, splitLine: { show: false } },
        { type: "value", gridIndex: 1, name: "I/O", axisLine: { show: false }, axisTick: { show: false }, axisLabel: { color: mutedForeground, fontSize: 11, formatter: (value: number) => `${formatBytes(value, language)}/s` }, splitLine: { lineStyle: { color: border, type: "dashed" } } }
      ],
      series: [
        { id: "app.cpu.usage_pct", name: "CPU", type: "line", xAxisIndex: 0, yAxisIndex: 0, showSymbol: false, connectNulls: false, lineStyle: { width: 1.8 }, emphasis: { focus: "none", scale: true }, data: points.map((point) => [point.timestampMs, point.cpuPercent]) },
        { id: "app.memory.used_bytes", name: t("memory"), type: "line", xAxisIndex: 0, yAxisIndex: 1, showSymbol: false, connectNulls: false, lineStyle: { width: 1.8 }, emphasis: { focus: "none", scale: true }, data: points.map((point) => [point.timestampMs, point.memoryUsedBytes]) },
        { id: "app.io.read_bps", name: t("ioRead"), type: "line", xAxisIndex: 1, yAxisIndex: 2, showSymbol: false, connectNulls: false, lineStyle: { width: 1.6 }, emphasis: { focus: "none", scale: true }, data: points.map((point) => [point.timestampMs, point.ioReadBytesPerSec]) },
        { id: "app.io.write_bps", name: t("ioWrite"), type: "line", xAxisIndex: 1, yAxisIndex: 2, showSymbol: false, connectNulls: false, lineStyle: { width: 1.6 }, emphasis: { focus: "none", scale: true }, data: points.map((point) => [point.timestampMs, point.ioWriteBytesPerSec]) }
      ]
    } as echarts.EChartsOption);
  }, [language, lifecycle, points, resolvedTheme, t]);

  return <div ref={ref} className="h-[390px] w-full" />;
}
