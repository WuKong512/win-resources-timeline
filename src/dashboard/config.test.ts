import { describe, expect, it } from "vitest";
import { buildMetricCatalog, getMetricDescriptor, type MetricCatalogItem } from "./metrics";
import type { MetricCategory, ProviderStatus, SystemSample } from "../types/resource";
import {
  canAddMetricToCard,
  createDefaultDashboardConfig,
  deserializeDashboardConfig,
  MAX_METRICS_PER_CARD,
  reorderDashboardCards,
  serializeDashboardConfig,
  toggleMetricPin,
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

function provider(providerId: string, categories: MetricCategory[]): ProviderStatus {
  return {
    providerId,
    displayName: providerId,
    supported: true,
    enabled: true,
    lifecycle: "running",
    capabilities: categories.map((category) => ({ providerId, category, supportStatus: "supported", enabled: true, canToggle: true, state: "supportedEnabled", reasonCode: null })),
    lastSuccessAtMs: null,
    failureCount: 0,
    lastError: null
  };
}

function catalogFor(samples: SystemSample[]): MetricCatalogItem[] {
  const providers = [provider("windows-baseline", ["cpu", "memory", "disk"]), ...samples.some((sample) => sample.gpus.length) ? [provider("nvidia-nvml", ["gpu"])] : []];
  return buildMetricCatalog({
    samples,
    providers,
    settings: { foregroundPollIntervalMs: 1_000, systemSampleIntervalMs: 5_000, idleThresholdSeconds: 60, systemSampleRetentionDays: 30, enabledCategories: ["cpu", "memory", "disk", "gpu"], disabledProviders: [] },
    snapshot: null
  });
}

describe("dashboard config", () => {
  it("accepts same-family metrics and rejects incompatible metrics", () => {
    expect(validateMetricSelection(["system.cpu.usage_pct", "gpu.nvml:uuid.utilization_pct"]).ok).toBe(true);
    expect(validateMetricSelection(["system.disk.read_bps", "system.disk.write_bps"]).ok).toBe(true);
    expect(validateMetricSelection(["system.cpu.usage_pct", "system.memory.used_bytes"]).ok).toBe(false);
    expect(validateMetricSelection(["system.cpu.usage_pct", "gpu.nvml:uuid.temperature_c"]).ok).toBe(false);
    expect(validateMetricSelection(["system.cpu.not_real"]).ok).toBe(false);
  });

  it("generates adaptive defaults from the catalog and keeps GPU detail progressive", () => {
    const noGpu = createDefaultDashboardConfig(catalogFor([baseSample]));
    expect(noGpu.cards.map((card) => card.id)).toEqual(["compute-usage", "memory", "disk-io"]);
    const withGpu = {
      ...baseSample,
      gpus: [{ deviceKey: "uuid-1", vendor: "NVIDIA", model: "GPU", capacityBytes: null, utilizationPercent: 10, memoryControllerUtilizationPercent: null, temperatureCelsius: 50, powerWatts: null, graphicsClockMhz: null, memoryClockMhz: null, vramUsedBytes: null, vramTotalBytes: null, powerScope: null, qualityMask: 0 }]
    };
    const adaptive = createDefaultDashboardConfig(catalogFor([withGpu]));
    expect(adaptive.cards.find((card) => card.id === "compute-usage")?.metricIds).not.toContain("gpu.uuid-1.utilization_pct");
    expect(adaptive.cards.find((card) => card.id === "gpu-utilization")?.metricIds).toContain("gpu.uuid-1.utilization_pct");
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

  it("does not offer a ninth metric once the card limit is reached", () => {
    const metricIds = Array.from({ length: MAX_METRICS_PER_CARD }, (_, index) => `gpu.uuid-${index}.utilization_pct` as const);
    const card = { id: "full", metricIds, hiddenMetricIds: [], order: 0, visible: true };
    expect(metricIds).toHaveLength(8);
    expect(canAddMetricToCard(card, "gpu.uuid-8.utilization_pct")).toBe(false);
  });

  it("pins, unpins, and reorders overview items without exposing card IDs", () => {
    const config = createDefaultDashboardConfig(catalogFor([baseSample]));
    const pinned = toggleMetricPin(config, "system.memory.used_bytes");
    expect(pinned.cards.some((card) => card.metricIds.includes("system.memory.used_bytes"))).toBe(true);
    const unpinned = toggleMetricPin(pinned, "system.memory.used_bytes");
    expect(unpinned.cards.some((card) => card.visible && card.metricIds.includes("system.memory.used_bytes"))).toBe(false);
    const moved = reorderDashboardCards(config, "disk-io", -1);
    expect(moved.cards.find((card) => card.id === "disk-io")?.order).toBe(1);
  });

  it("does not manufacture a descriptor when a catalog item is missing", () => {
    const catalog = catalogFor([baseSample]).filter((item) => item.id !== "system.disk.read_bps");
    expect(createDefaultDashboardConfig(catalog).cards.find((card) => card.id === "disk-io")?.metricIds).toEqual(["system.disk.write_bps"]);
    expect(getMetricDescriptor("system.disk.read_bps")?.id).toBe("system.disk.read_bps");
  });
});
