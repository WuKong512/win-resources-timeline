import { describe, expect, it } from "vitest";
import { buildDashboardChartOption, buildTimelineChartOption, formatTooltipItems } from "./chartOptions";
import { getMetricDescriptor } from "./metrics";
import type { SystemSample } from "../types/resource";

const palette = { colors: ["#0ff", "#08f", "#80f"], foreground: "#fff", mutedForeground: "#aaa", border: "#333", muted: "#222", card: "#111", selected: "#f55" };
const sampleA: SystemSample = { timestampMs: 1_000, sampleDurationMs: 5_000, cpuPercent: 32.4, memoryPercent: 61.5, memoryUsedBytes: 100, memoryTotalBytes: 200, diskReadBytesPerSec: 12_400_000, diskWriteBytesPerSec: null, gpus: [{ deviceKey: "uuid-1", vendor: "NVIDIA", model: "GPU", capacityBytes: null, utilizationPercent: 18.2, memoryControllerUtilizationPercent: null, temperatureCelsius: 50, powerWatts: null, graphicsClockMhz: null, memoryClockMhz: null, vramUsedBytes: null, vramTotalBytes: null, powerScope: null, qualityMask: 0 }], hasAppSnapshot: false };
const sampleB: SystemSample = { ...sampleA, timestampMs: 2_000, cpuPercent: null, diskReadBytesPerSec: null, gpus: [] };

function labels(descriptor: { id: string }) {
  return descriptor.id;
}

describe("chart option builder", () => {
  it("creates stable series ids, preserves null data, and never requests series blur", () => {
    const option = buildTimelineChartOption({ samples: [sampleA, sampleB], gaps: [], startMs: 0, endMs: 3_000, selectedTimestampMs: 1_000, language: "en", palette, metricLabel: labels, missingLabel: "No sample" });
    const series = option.series as Array<Record<string, unknown>>;
    expect(series.map((item) => item.id)).toEqual(expect.arrayContaining(["system.cpu.usage_pct", "system.memory.usage_pct", "system.disk.read_bps", "system.disk.write_bps", "gpu.uuid-1.utilization_pct"]));
    expect(series.every((item) => (item.emphasis as { focus?: string }).focus === "none")).toBe(true);
    const cpu = series.find((item) => item.id === "system.cpu.usage_pct")!;
    expect(cpu.data).toEqual([[1_000, 32.4], [2_000, null]]);
    expect((cpu.markLine as { data: unknown[] }).data).toEqual([{ xAxis: 1_000 }]);
  });

  it("keeps custom cards single-axis and formats multi-series tooltip units", () => {
    const option = buildDashboardChartOption({ samples: [sampleA, sampleB], metricIds: ["system.disk.read_bps", "system.disk.write_bps"], startMs: 0, endMs: 3_000, selectedTimestampMs: null, language: "en", palette, metricLabel: labels, missingLabel: "No sample" });
    expect((option.yAxis as unknown[]).length).toBe(1);
    const descriptors = new Map([getMetricDescriptor("system.cpu.usage_pct"), getMetricDescriptor("system.disk.read_bps")].map((descriptor) => [descriptor!.id, descriptor!]));
    const tooltip = formatTooltipItems({ parameters: [{ axisValue: 1_000, seriesId: "system.cpu.usage_pct", seriesName: "CPU", marker: "•", value: [1_000, 32.4] }, { axisValue: 1_000, seriesId: "system.disk.read_bps", seriesName: "Disk read", marker: "•", value: [1_000, 12_400_000] }, { axisValue: 1_000, seriesId: "system.disk.write_bps", seriesName: "Disk write", marker: "•", value: [1_000, null] }], descriptors, language: "en", missingLabel: "No sample" });
    expect(tooltip).toContain("32.4%");
    expect(tooltip).toContain("MB/s");
    expect(tooltip).not.toMatch(/undefined|null|NaN/);
  });
});
