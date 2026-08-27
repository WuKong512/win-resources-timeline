import { describe, expect, it } from "vitest";
import {
  buildMetricCatalog,
  currentReadingPresentation,
  getAvailableMetricDescriptors,
  getMetricDescriptor,
  gpuMetricId,
  metricValue,
  trendFamilies,
  type GpuMetricField,
  type MetricCatalogItem
} from "./metrics";
import { canRetryMetricCatalog, failMetricCatalogLoad, hasAuthoritativeMetricCatalog } from "./metricCatalogState";
import type { MetricCatalogEntry, MetricCatalogSnapshot, MetricCategory, ProviderStatus, SystemSample } from "../types/resource";

function sample(timestampMs: number, overrides: Partial<SystemSample> = {}): SystemSample {
  return {
    timestampMs,
    sampleDurationMs: 5_000,
    cpuPercent: 32.4,
    memoryPercent: 61.5,
    memoryUsedBytes: 8 * 1024 * 1024,
    memoryTotalBytes: 16 * 1024 * 1024,
    diskReadBytesPerSec: 12 * 1024 * 1024,
    diskWriteBytesPerSec: 4 * 1024 * 1024,
    gpus: [{
      deviceKey: "nvml:uuid-abc",
      vendor: "NVIDIA",
      model: "Test GPU",
      capacityBytes: 8 * 1024 * 1024 * 1024,
      utilizationPercent: 18.2,
      memoryControllerUtilizationPercent: 12,
      temperatureCelsius: 54,
      powerWatts: 120,
      graphicsClockMhz: 1_800,
      memoryClockMhz: 7_000,
      vramUsedBytes: 2 * 1024 * 1024 * 1024,
      vramTotalBytes: 8 * 1024 * 1024 * 1024,
      powerScope: "gpu_board",
      qualityMask: 0
    }],
    hasAppSnapshot: true,
    ...overrides
  };
}

function settings(enabledCategories: MetricCategory[] = ["cpu", "memory", "disk", "gpu"] ) {
  return { foregroundPollIntervalMs: 1_000, systemSampleIntervalMs: 5_000, idleThresholdSeconds: 60, systemSampleRetentionDays: 30, enabledCategories, disabledProviders: [] };
}

function provider(providerId: string, categories: MetricCategory[], overrides: Partial<ProviderStatus> = {}): ProviderStatus {
  return {
    providerId,
    displayName: providerId,
    supported: true,
    enabled: true,
    lifecycle: "running",
    capabilities: categories.map((category) => ({ providerId, category, supportStatus: "supported", enabled: true, canToggle: true, state: "supportedEnabled", reasonCode: null })),
    lastSuccessAtMs: null,
    failureCount: 0,
    lastError: null,
    ...overrides
  };
}

function entry(metricKey: string, category: MetricCategory, supportStatus: MetricCatalogEntry["supportStatus"] = "supported", overrides: Partial<MetricCatalogEntry> = {}): MetricCatalogEntry {
  return { metricKey, category, providerId: category === "gpu" ? "nvidia-nvml" : "windows-baseline", device: null, enabled: true, supportStatus, ...overrides };
}

function catalog(snapshot: MetricCatalogSnapshot, samples: SystemSample[] = [], providers: ProviderStatus[] = [provider("windows-baseline", ["cpu", "memory", "disk"]), provider("nvidia-nvml", ["gpu"])]) {
  return buildMetricCatalog({ snapshot, samples, providers, settings: settings() });
}

function systemEntry(metricKey: string, supportStatus: MetricCatalogEntry["supportStatus"] = "supported", overrides: Partial<MetricCatalogEntry> = {}) {
  return entry(metricKey, metricKey.startsWith("system.cpu") ? "cpu" : metricKey.startsWith("system.memory") ? "memory" : "disk", supportStatus, overrides);
}

describe("metric registry", () => {
  it("uses stable GPU device keys for every GPU metric", () => {
    const id = gpuMetricId("nvml:uuid-abc", "temperature_c");
    const descriptor = getMetricDescriptor(id);
    expect(descriptor?.id).toBe(id);
    expect(descriptor?.deviceKey).toBe("nvml:uuid-abc");
    expect(metricValue(descriptor!, sample(1))).toBe(54);
  });

  it("exposes only metrics with actual finite data", () => {
    const available = getAvailableMetricDescriptors([sample(1)]);
    expect(available.map((descriptor) => descriptor.id)).toContain("system.cpu.usage_pct");
    expect(available.map((descriptor) => descriptor.id)).toContain(gpuMetricId("nvml:uuid-abc", "vram_total_bytes"));
    const noTemperature = sample(2, { gpus: [{ ...sample(1).gpus[0], temperatureCelsius: null }] });
    expect(getAvailableMetricDescriptors([noTemperature]).map((descriptor) => descriptor.id)).not.toContain(gpuMetricId("nvml:uuid-abc", "temperature_c"));
  });

  it("registers all currently collected GPU fields without index-based identity", () => {
    const fields: GpuMetricField[] = ["utilization_pct", "memory_controller_utilization_pct", "temperature_c", "board_power_w", "graphics_clock_mhz", "memory_clock_mhz", "vram_used_bytes", "vram_total_bytes"];
    const available = getAvailableMetricDescriptors([sample(1)]).map((descriptor) => descriptor.id);
    for (const field of fields) expect(available).toContain(gpuMetricId("nvml:uuid-abc", field));
  });

  it("keeps the complete system catalog when the selected range has no samples", () => {
    const snapshot: MetricCatalogSnapshot = {
      metrics: [
        systemEntry("system.cpu.usage_pct"),
        systemEntry("system.memory.usage_pct"),
        systemEntry("system.memory.used_bytes"),
        systemEntry("system.disk.read_bps"),
        systemEntry("system.disk.write_bps")
      ],
      devices: []
    };
    const items = catalog(snapshot, []);
    expect(items.map((item) => item.id)).toEqual(expect.arrayContaining([
      "system.cpu.usage_pct",
      "system.memory.usage_pct",
      "system.memory.used_bytes",
      "system.disk.read_bps",
      "system.disk.write_bps"
    ]));
    expect(items.find((item) => item.id === "system.cpu.usage_pct")?.status).toBe("NO_DATA_IN_RANGE");
  });

  it("generates separate GPU metric groups from stable identities, including multiple GPUs", () => {
    const devices = [
      { stableKey: "nvml:uuid-a", vendor: "NVIDIA", model: "A", capacityBytes: 8 },
      { stableKey: "nvml:uuid-b", vendor: "NVIDIA", model: "B", capacityBytes: 12 }
    ];
    const items = catalog({ metrics: [], devices });
    expect(items.filter((item) => item.category === "gpu")).toHaveLength(16);
    expect(items.map((item) => item.id)).toContain(gpuMetricId("nvml:uuid-a", "temperature_c"));
    expect(items.map((item) => item.id)).toContain(gpuMetricId("nvml:uuid-b", "temperature_c"));
    expect(items.find((item) => item.id === gpuMetricId("nvml:uuid-a", "temperature_c"))?.device?.stableKey).toBe("nvml:uuid-a");
    expect(items.find((item) => item.id === gpuMetricId("nvml:uuid-b", "temperature_c"))?.device?.stableKey).toBe("nvml:uuid-b");
    expect(items.find((item) => item.id === gpuMetricId("nvml:uuid-a", "temperature_c"))?.status).toBe("NO_DATA_IN_RANGE");
  });

  it("does not add a sample-only GPU identity to an authoritative catalog", () => {
    const items = catalog({ metrics: [], devices: [] }, [sample(1)]);

    expect(items.some((item) => item.device?.stableKey === "nvml:uuid-abc")).toBe(false);
    expect(items.some((item) => item.id === gpuMetricId("nvml:uuid-abc", "temperature_c"))).toBe(false);
  });

  it("keeps a degraded catalog observable without inventing a GPU identity", () => {
    const failed = failMetricCatalogLoad();
    const items = buildMetricCatalog({
      snapshot: failed.snapshot,
      samples: [sample(1, { gpus: [] })],
      providers: [provider("windows-baseline", ["cpu", "memory", "disk"]), provider("nvidia-nvml", ["gpu"])],
      settings: settings()
    });

    expect(hasAuthoritativeMetricCatalog(failed)).toBe(false);
    expect(canRetryMetricCatalog(failed)).toBe(true);
    expect(items.some((item) => item.id === "system.cpu.usage_pct")).toBe(true);
    expect(items.some((item) => item.category === "gpu")).toBe(false);

    const observedFallback = buildMetricCatalog({
      snapshot: failed.snapshot,
      samples: [sample(1)],
      providers: [provider("windows-baseline", ["cpu", "memory", "disk"]), provider("nvidia-nvml", ["gpu"])],
      settings: settings()
    });
    expect(observedFallback.some((item) => item.device?.stableKey === "nvml:uuid-abc")).toBe(true);
  });

  it("treats numeric zero as available and null as no-data, never as unsupported", () => {
    const snapshot = { metrics: [systemEntry("system.cpu.usage_pct")], devices: [] };
    expect(catalog(snapshot, [sample(1, { cpuPercent: 0 })]).find((item) => item.id === "system.cpu.usage_pct")?.status).toBe("AVAILABLE");
    expect(catalog(snapshot, [sample(1, { cpuPercent: null })]).find((item) => item.id === "system.cpu.usage_pct")?.status).toBe("NO_DATA_IN_RANGE");
  });

  it("keeps range availability separate from a missing latest reading", () => {
    const snapshot = { metrics: [systemEntry("system.cpu.usage_pct")], devices: [] };
    const samples = [sample(1, { cpuPercent: 42 }), sample(2, { cpuPercent: null })];
    const item = catalog(snapshot, samples).find((candidate) => candidate.id === "system.cpu.usage_pct")!;

    expect(item.status).toBe("AVAILABLE");
    expect(metricValue(item.descriptor, samples[1])).toBeNull();
    expect(currentReadingPresentation(item.status, metricValue(item.descriptor, samples[1]))).toBe("NO_CURRENT_READING");
    expect(currentReadingPresentation(item.status, metricValue(item.descriptor, samples[0]))).toBe("VALUE");
  });

  it("hides unsupported-only trend families while keeping their Explorer rows", () => {
    const device = { stableKey: "nvml:uuid-temp", vendor: "NVIDIA", model: "Temperature GPU", capacityBytes: 8 };
    const unsupportedMetrics = [entry("gpu.temperature_celsius", "gpu", "unsupported", { device })];
    const items = catalog({ metrics: unsupportedMetrics, devices: [device] }, [], [provider("windows-baseline", ["cpu", "memory", "disk"]), provider("nvidia-nvml", ["gpu"])])
      .filter((item) => item.descriptor.unitFamily === "temperature");

    expect(items).toHaveLength(1);
    expect(items[0].status).toBe("UNSUPPORTED");
    expect(trendFamilies(items)).not.toContain("temperature");
    expect(items.map((item) => item.id)).toContain(gpuMetricId(device.stableKey, "temperature_c"));
  });

  it("hides unknown-only trend families while keeping their Explorer rows", () => {
    const device = { stableKey: "nvml:uuid-unknown", vendor: "NVIDIA", model: "Unknown GPU", capacityBytes: 8 };
    const items = buildMetricCatalog({
      snapshot: { metrics: [], devices: [device] },
      samples: [],
      providers: [provider("windows-baseline", ["cpu", "memory", "disk"])],
      settings: settings()
    }).filter((item) => item.descriptor.unitFamily === "temperature");

    expect(items).toHaveLength(1);
    expect(items[0].status).toBe("UNKNOWN");
    expect(trendFamilies(items)).not.toContain("temperature");
    expect(items.map((item) => item.id)).toContain(gpuMetricId(device.stableKey, "temperature_c"));
  });

  it("keeps a no-data trend family visible for explanation", () => {
    const device = { stableKey: "nvml:uuid-no-data", vendor: "NVIDIA", model: "No Data GPU", capacityBytes: 8 };
    const items = catalog({ metrics: [], devices: [device] }, []);

    expect(items.find((item) => item.id === gpuMetricId(device.stableKey, "temperature_c"))?.status).toBe("NO_DATA_IN_RANGE");
    expect(trendFamilies(items)).toContain("temperature");
  });

  it("keeps a failed trend family visible while its Explorer row explains the failure", () => {
    const device = { stableKey: "nvml:uuid-failed", vendor: "NVIDIA", model: "Failed GPU", capacityBytes: 8 };
    const items = catalog({ metrics: [entry("gpu.temperature_celsius", "gpu", "failed", { device })], devices: [device] }, []);

    expect(items.find((item) => item.id === gpuMetricId(device.stableKey, "temperature_c"))?.status).toBe("FAILED");
    expect(trendFamilies(items)).toContain("temperature");
  });

  it("keeps a disabled trend family visible without enabling collection", () => {
    const device = { stableKey: "nvml:uuid-disabled", vendor: "NVIDIA", model: "Disabled GPU", capacityBytes: 8 };
    const items = buildMetricCatalog({
      snapshot: { metrics: [], devices: [device] },
      samples: [],
      providers: [provider("windows-baseline", ["cpu", "memory", "disk"]), provider("nvidia-nvml", ["gpu"])],
      settings: settings(["cpu", "memory", "disk"])
    });

    expect(items.find((item) => item.id === gpuMetricId(device.stableKey, "temperature_c"))?.status).toBe("DISABLED");
    expect(trendFamilies(items)).toContain("temperature");
  });

  it.each([
    ["unsupported", "UNSUPPORTED"],
    ["permission_denied", "UNSUPPORTED"],
    ["provider_missing", "UNSUPPORTED"],
    ["probe_failed", "FAILED"],
    ["failed", "FAILED"]
  ] as const)("projects runtime support %s as %s", (supportStatus, expected) => {
    const items = catalog({ metrics: [systemEntry("system.cpu.usage_pct", supportStatus)], devices: [] }, [sample(1)]);
    expect(items.find((item) => item.id === "system.cpu.usage_pct")?.status).toBe(expected);
  });

  it("keeps a provider lifecycle failure distinct from unsupported capability", () => {
    const items = buildMetricCatalog({
      snapshot: { metrics: [systemEntry("system.cpu.usage_pct")], devices: [] },
      samples: [],
      providers: [provider("windows-baseline", ["cpu", "memory", "disk"], { supported: false, lifecycle: "failed" })],
      settings: settings()
    });
    expect(items.find((item) => item.id === "system.cpu.usage_pct")?.status).toBe("FAILED");
  });

  it("preserves disabled and degraded truth from settings, entries, and provider health", () => {
    const disabled = buildMetricCatalog({
      snapshot: { metrics: [systemEntry("system.cpu.usage_pct", "supported", { enabled: false })], devices: [] },
      samples: [sample(1)],
      providers: [provider("windows-baseline", ["cpu", "memory", "disk"])],
      settings: settings(["memory", "disk", "gpu"])
    });
    expect(disabled.find((item) => item.id === "system.cpu.usage_pct")?.status).toBe("DISABLED");

    const disabledProvider = buildMetricCatalog({
      snapshot: { metrics: [systemEntry("system.cpu.usage_pct")], devices: [] },
      samples: [sample(1)],
      providers: [provider("windows-baseline", ["cpu", "memory", "disk"]), provider("nvidia-nvml", ["gpu"])],
      settings: { ...settings(), disabledProviders: ["WINDOWS-BASELINE"] }
    });
    expect(disabledProvider.find((item) => item.id === "system.cpu.usage_pct")?.status).toBe("DISABLED");

    const degraded = catalog(
      { metrics: [systemEntry("system.cpu.usage_pct")], devices: [] },
      [sample(1)],
      [provider("windows-baseline", ["cpu", "memory", "disk"], { lifecycle: "paused", lastError: { code: "paused", message: "paused" } }), provider("nvidia-nvml", ["gpu"])]
    );
    expect(degraded.find((item) => item.id === "system.cpu.usage_pct")?.status).toBe("DEGRADED");
  });

  it("does not depend on a rendered sample to expose the current metric registry", () => {
    const items: MetricCatalogItem[] = catalog({ metrics: [systemEntry("system.cpu.usage_pct")], devices: [] }, []);
    expect(items.some((item) => item.id === "system.cpu.usage_pct")).toBe(true);
    expect(items.some((item) => item.id === "system.memory.usage_pct")).toBe(true);
  });
});
