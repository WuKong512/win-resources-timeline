import { describe, expect, it } from "vitest";
import { buildDashboardChartOption, buildTimelineChartOption, formatTooltipItems } from "./chartOptions";
import { getMetricDescriptor } from "./metrics";
import type { SystemSample } from "../types/resource";
import { inferSampleGaps } from "../utils/uiSemantics";

const palette = { colors: ["#0ff", "#08f", "#80f"], foreground: "#fff", mutedForeground: "#aaa", border: "#333", muted: "#222", card: "#111", selected: "#f55" };
const sampleA: SystemSample = { timestampMs: 1_000, sampleDurationMs: 5_000, cpuPercent: 32.4, memoryPercent: 61.5, memoryUsedBytes: 100, memoryTotalBytes: 200, diskReadBytesPerSec: 12_400_000, diskWriteBytesPerSec: null, gpus: [{ deviceKey: "uuid-1", vendor: "NVIDIA", model: "GPU", capacityBytes: null, utilizationPercent: 18.2, memoryControllerUtilizationPercent: null, temperatureCelsius: 50, powerWatts: null, graphicsClockMhz: null, memoryClockMhz: null, vramUsedBytes: null, vramTotalBytes: null, powerScope: null, qualityMask: 0 }], hasAppSnapshot: false };
const sampleB: SystemSample = { ...sampleA, timestampMs: 2_000, cpuPercent: null, diskReadBytesPerSec: null, gpus: [] };

function labels(descriptor: { id: string }) {
  return descriptor.id;
}

describe("chart option builder", () => {
  it("keeps line visibility invariant across ECharts interactive states", () => {
    const option = buildTimelineChartOption({ samples: [sampleA, sampleB], gaps: [], startMs: 0, endMs: 3_000, selectedTimestampMs: 1_000, language: "en", palette, metricLabel: labels, missingLabel: "No sample" });
    const series = option.series as Array<Record<string, unknown>>;
    expect(series.map((item) => item.id)).toEqual(expect.arrayContaining(["system.cpu.usage_pct", "system.memory.usage_pct", "system.disk.read_bps", "system.disk.write_bps", "gpu.uuid-1.utilization_pct"]));
    expect(series.every((item) => (item.emphasis as { focus?: string }).focus === "none")).toBe(true);
    for (const item of series) {
      const normal = item.lineStyle as { color: string; width: number; opacity: number; type?: string };
      for (const state of ["emphasis", "blur", "select"]) {
        const stateLine = (item[state] as { lineStyle: typeof normal }).lineStyle;
        expect(stateLine).toEqual(normal);
        expect(stateLine.opacity).toBe(1);
      }
    }
    const cpu = series.find((item) => item.id === "system.cpu.usage_pct")!;
    expect(cpu.data).toEqual([[1_000, 32.4], [2_000, null]]);
    expect((cpu.markLine as { data: unknown[] }).data).toEqual([{ xAxis: 1_000 }]);
  });

  it("keeps custom cards single-axis and formats multi-series tooltip units", () => {
    const option = buildDashboardChartOption({ samples: [sampleA, sampleB], gaps: [], metricIds: ["system.disk.read_bps", "system.disk.write_bps"], startMs: 0, endMs: 3_000, selectedTimestampMs: null, language: "en", palette, metricLabel: labels, missingLabel: "No sample" });
    expect((option.yAxis as unknown[]).length).toBe(1);
    const descriptors = new Map([getMetricDescriptor("system.cpu.usage_pct"), getMetricDescriptor("system.disk.read_bps")].map((descriptor) => [descriptor!.id, descriptor!]));
    const tooltip = formatTooltipItems({ parameters: [{ axisValue: 1_000, seriesId: "system.cpu.usage_pct", seriesName: "CPU", marker: "•", value: [1_000, 32.4] }, { axisValue: 1_000, seriesId: "system.disk.read_bps", seriesName: "Disk read", marker: "•", value: [1_000, 12_400_000] }, { axisValue: 1_000, seriesId: "system.disk.write_bps", seriesName: "Disk write", marker: "•", value: [1_000, null] }], descriptors, language: "en", missingLabel: "No sample" });
    expect(tooltip).toContain("32.4%");
    expect(tooltip).toContain("MB/s");
    expect(tooltip).not.toMatch(/undefined|null|NaN/);
  });

  it("inserts an authoritative null discontinuity into dashboard series", () => {
    const sampleAfterGap = { ...sampleA, timestampMs: 56_000, cpuPercent: 44.4 };
    const option = buildDashboardChartOption({
      samples: [sampleA, sampleAfterGap],
      gaps: [{ startMs: 6_000, endMs: 56_000, durationMs: 50_000 }],
      metricIds: ["system.cpu.usage_pct"],
      startMs: 0,
      endMs: 60_000,
      selectedTimestampMs: null,
      language: "en",
      palette,
      metricLabel: labels,
      missingLabel: "No sample"
    });
    const series = option.series as Array<Record<string, unknown>>;
    expect(series[0].data).toEqual([[1_000, 32.4], [6_000, null], [56_000, 44.4]]);
  });

  it("infers a conservative long-sample gap for the ResourceLineChart path", () => {
    const sampleAfterGap = { ...sampleA, timestampMs: 56_000, cpuPercent: 44.4 };
    const gaps = inferSampleGaps([sampleA, sampleAfterGap]);
    expect(gaps).toEqual([{ startMs: 6_000, endMs: 56_000, durationMs: 50_000 }]);

    const jittered = { ...sampleA, timestampMs: 6_007 };
    expect(inferSampleGaps([sampleA, jittered])).toEqual([]);
  });
});
