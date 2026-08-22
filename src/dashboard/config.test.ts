import { describe, expect, it } from "vitest";
import type { SystemSample } from "../types/resource";
import {
  createDefaultDashboardConfig,
  deserializeDashboardConfig,
  serializeDashboardConfig,
  validateDashboardConfig,
  validateMetricSelection
} from "./config";

const baseSample: SystemSample = {
  timestampMs: 1,
  sampleDurationMs: 5_000,
  cpuPercent: 20,
  memoryPercent: 40,
  memoryUsedBytes: 100,
  memoryTotalBytes: 200,
  diskReadBytesPerSec: 10,
  diskWriteBytesPerSec: 20,
  gpus: [],
  hasAppSnapshot: false
};

describe("dashboard config", () => {
  it("accepts same-family metrics and rejects incompatible metrics", () => {
    expect(validateMetricSelection(["system.cpu.usage_pct", "gpu.nvml:uuid.utilization_pct"]).ok).toBe(true);
    expect(validateMetricSelection(["system.disk.read_bps", "system.disk.write_bps"]).ok).toBe(true);
    expect(validateMetricSelection(["system.cpu.usage_pct", "system.memory.used_bytes"]).ok).toBe(false);
    expect(validateMetricSelection(["system.cpu.usage_pct", "gpu.nvml:uuid.temperature_c"]).ok).toBe(false);
    expect(validateMetricSelection(["system.cpu.not_real"]).ok).toBe(false);
  });

  it("generates adaptive defaults and does not create a permanent GPU card without GPU data", () => {
    const noGpu = createDefaultDashboardConfig([baseSample]);
    expect(noGpu.cards.map((card) => card.id)).toEqual(["compute-usage", "memory", "disk-io"]);
    const withGpu = {
      ...baseSample,
      gpus: [{ deviceKey: "uuid-1", vendor: "NVIDIA", model: "GPU", capacityBytes: null, utilizationPercent: 10, memoryControllerUtilizationPercent: null, temperatureCelsius: 50, powerWatts: null, graphicsClockMhz: null, memoryClockMhz: null, vramUsedBytes: null, vramTotalBytes: null, powerScope: null, qualityMask: 0 }]
    };
    const adaptive = createDefaultDashboardConfig([withGpu]);
    expect(adaptive.cards.find((card) => card.id === "compute-usage")?.metricIds).toContain("gpu.uuid-1.utilization_pct");
    expect(adaptive.cards.map((card) => card.id)).toContain("gpu-temperature");
  });

  it("round-trips, preserves unavailable metrics, and falls back for corrupt or unknown payloads", () => {
    const config = {
      version: 1 as const,
      cards: [{ id: "saved-gpu", metricIds: ["gpu.uuid-offline.temperature_c" as const], hiddenMetricIds: [], order: 0, visible: true }]
    };
    const roundTrip = deserializeDashboardConfig(serializeDashboardConfig(config));
    expect(roundTrip).toEqual(config);
    expect(deserializeDashboardConfig("{broken")).toBeNull();
    expect(deserializeDashboardConfig(JSON.stringify({ ...config, version: 2 }))).toBeNull();
    expect(deserializeDashboardConfig(JSON.stringify({ ...config, cards: [{ ...config.cards[0], metricIds: ["future.metric"] }] }))).toBeNull();
  });

  it("rejects incompatible persisted cards before they can reach the chart", () => {
    const invalid = validateDashboardConfig({ version: 1, cards: [{ id: "mixed", metricIds: ["system.cpu.usage_pct", "system.disk.read_bps"], hiddenMetricIds: [], order: 0, visible: true }] });
    expect(invalid.ok).toBe(false);
  });
});
