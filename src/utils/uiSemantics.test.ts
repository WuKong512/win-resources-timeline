import { describe, expect, it } from "vitest";
import { mainNavigation } from "../navigation";
import type { CapabilityState, CollectionSettings, ComputerStateInterval, MetricCategory, ProviderStatus, SystemSample, TimelineGap } from "../types/resource";
import { aggregateCategoryCapability, evidenceStatusTone, gpuDevices, metricDataState, stateDurations, timelineChartSamples, timelineCoverageState, timelineRefreshIntervalMs, toggleCategory } from "./uiSemantics";

function sample(timestampMs: number, gpus: SystemSample["gpus"] = []): SystemSample {
  return {
    timestampMs,
    sampleDurationMs: 5_000,
    cpuPercent: 0,
    memoryPercent: null,
    memoryUsedBytes: null,
    memoryTotalBytes: null,
    diskReadBytesPerSec: null,
    diskWriteBytesPerSec: null,
    gpus,
    hasAppSnapshot: false
  };
}

function provider(providerId: string, state: CapabilityState, category: MetricCategory = "gpu"): ProviderStatus {
  return {
    providerId,
    displayName: providerId,
    supported: state !== "unsupported",
    enabled: state === "supportedEnabled",
    lifecycle: state === "failed" ? "failed" : "running",
    capabilities: [{
      providerId,
      category,
      supportStatus: state === "unsupported" ? "unsupported" : "supported",
      enabled: state === "supportedEnabled",
      canToggle: true,
      state,
      reasonCode: null
    }],
    lastSuccessAtMs: null,
    failureCount: state === "failed" ? 1 : 0,
    lastError: null
  };
}

describe("PR-06 information architecture semantics", () => {
  it("exposes exactly the four first-class destinations in order", () => {
    expect(mainNavigation.map((item) => item.id)).toEqual(["timeline", "usage", "crashes", "settings"]);
  });

  it("keeps real zero, missing, disabled, unsupported, and failed distinct", () => {
    expect(metricDataState(0, "supportedEnabled")).toBe("zero");
    expect(metricDataState(null, "supportedEnabled")).toBe("missing");
    expect(metricDataState(0, "supportedDisabled")).toBe("disabled");
    expect(metricDataState(null, "unsupported")).toBe("unsupported");
    expect(metricDataState(null, "failed")).toBe("failed");
  });

  it("preserves multi-GPU device identity instead of merging series", () => {
    const first = { deviceKey: "gpu:0", vendor: "NVIDIA", model: "A", capacityBytes: null, utilizationPercent: 0, memoryControllerUtilizationPercent: null, temperatureCelsius: null, powerWatts: null, graphicsClockMhz: null, memoryClockMhz: null, vramUsedBytes: null, vramTotalBytes: null, powerScope: null, qualityMask: 0 };
    const second = { ...first, deviceKey: "gpu:1", model: "B", utilizationPercent: 42 };
    expect(gpuDevices([sample(1, [first]), sample(2, [second, first])]).map((gpu) => gpu.deviceKey)).toEqual(["gpu:0", "gpu:1"]);
  });

  it("uses backend gap markers without inferring gaps from bounded point spacing", () => {
    const continuous = timelineChartSamples([sample(1_000), sample(6_000)], []);
    expect(continuous).toHaveLength(2);

    const withRealGap: TimelineGap = { startMs: 6_000, endMs: 56_000, durationMs: 50_000 };
    const withRealGapSamples = timelineChartSamples([sample(1_000), sample(56_000)], [withRealGap]);
    expect(withRealGapSamples).toHaveLength(3);
    expect(withRealGapSamples[1].timestampMs).toBe(6_000);
    expect(withRealGapSamples[1].cpuPercent).toBeNull();

    const widelySpacedWithoutBackendGap = timelineChartSamples([sample(1_000), sample(56_000)], []);
    expect(widelySpacedWithoutBackendGap).toHaveLength(2);

    const jittered = [sample(1_000), sample(6_007), sample(11_003), sample(16_011), sample(21_002)];
    expect(timelineChartSamples(jittered, [])).toEqual(jittered);

    const clippedGap: TimelineGap = { startMs: 10_000, endMs: 20_000, durationMs: 10_000 };
    const requestedStart = 10_000;
    const requestedEnd = 30_000;
    expect(clippedGap.startMs).toBeGreaterThanOrEqual(requestedStart);
    expect(clippedGap.endMs).toBeLessThanOrEqual(requestedEnd);
    expect(timelineChartSamples([sample(10_000), sample(25_000)], [clippedGap])[1].timestampMs).toBe(10_000);
  });

  it("limits current-window refresh to the smallest useful range", () => {
    expect(timelineRefreshIntervalMs(1, true)).toBe(5_000);
    expect(timelineRefreshIntervalMs(7, true)).toBe(60_000);
    expect(timelineRefreshIntervalMs(30, true)).toBeUndefined();
    expect(timelineRefreshIntervalMs(1, false)).toBeUndefined();
  });

  it("keeps incomplete timeline coverage separate from a provider failure", () => {
    expect(timelineCoverageState(1)).toBe("complete");
    expect(timelineCoverageState(0.5)).toBe("incomplete");
    expect(timelineCoverageState(0)).not.toBe("failed");
  });

  it("keeps an enabled provider visible when another provider is unsupported or failed", () => {
    const settings: CollectionSettings = {
      foregroundPollIntervalMs: 1_000,
      systemSampleIntervalMs: 5_000,
      idleThresholdSeconds: 300,
      systemSampleRetentionDays: 7,
      enabledCategories: ["gpu"],
      disabledProviders: []
    };
    expect(aggregateCategoryCapability([provider("unsupported", "unsupported"), provider("enabled", "supportedEnabled")], settings, "gpu")).toBe("supportedEnabled");
    expect(aggregateCategoryCapability([provider("failed", "failed"), provider("enabled", "supportedEnabled")], settings, "gpu")).toBe("supportedEnabled");
    expect(aggregateCategoryCapability([provider("unsupported", "unsupported")], settings, "gpu")).toBe("unsupported");
    expect(aggregateCategoryCapability([provider("failed", "failed")], settings, "gpu")).toBe("failed");
    expect(aggregateCategoryCapability([provider("enabled", "supportedEnabled")], { ...settings, enabledCategories: [] }, "gpu")).toBe("supportedDisabled");
  });

  it("keeps computer states separate for usage presentation", () => {
    const intervals: ComputerStateInterval[] = [
      { state: "active", startTimeMs: 0, endTimeMs: 10_000, durationMs: 10_000 },
      { state: "idle", startTimeMs: 10_000, endTimeMs: 20_000, durationMs: 10_000 },
      { state: "locked", startTimeMs: 20_000, endTimeMs: 30_000, durationMs: 10_000 },
      { state: "sleep", startTimeMs: 30_000, endTimeMs: 40_000, durationMs: 10_000 },
      { state: "unknown", startTimeMs: 40_000, endTimeMs: 45_000, durationMs: 5_000 }
    ];
    expect(stateDurations(intervals)).toEqual({ active: 10_000, idle: 10_000, locked: 10_000, sleep: 10_000, unknown: 5_000 });
  });

  it("maps evidence lifecycle without making partial equivalent to complete", () => {
    expect(evidenceStatusTone("pending")).toBe("pending");
    expect(evidenceStatusTone("post_pending")).toBe("pending");
    expect(evidenceStatusTone("partial")).toBe("partial");
    expect(evidenceStatusTone("complete")).toBe("complete");
    expect(evidenceStatusTone("failed")).toBe("failed");
  });

  it("updates the actual CollectionSettings enabledCategories contract", () => {
    const settings: CollectionSettings = {
      foregroundPollIntervalMs: 1_000,
      systemSampleIntervalMs: 5_000,
      idleThresholdSeconds: 300,
      systemSampleRetentionDays: 7,
      enabledCategories: ["cpu", "memory"],
      disabledProviders: ["example"]
    };
    expect(toggleCategory(settings, "gpu")).toEqual({ ...settings, enabledCategories: ["cpu", "memory", "gpu"] });
    expect(toggleCategory(settings, "cpu")).toEqual({ ...settings, enabledCategories: ["memory"] });
  });
});
