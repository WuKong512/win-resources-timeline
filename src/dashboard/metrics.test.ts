import { describe, expect, it } from "vitest";
import {
  getAvailableMetricDescriptors,
  getMetricDescriptor,
  gpuMetricId,
  metricValue,
  type GpuMetricField
} from "./metrics";
import type { SystemSample } from "../types/resource";

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
});
