import type {
  CapabilityState,
  CollectionSettings,
  ComputerStateInterval,
  GpuSample,
  MetricCategory,
  ProviderStatus,
  TimelineSample,
  SystemSample
} from "../types/resource";

export type MetricDataState =
  | "value"
  | "zero"
  | "missing"
  | "disabled"
  | "unsupported"
  | "failed";

export const metricDataState = (
  value: number | null | undefined,
  capability: CapabilityState | undefined
): MetricDataState => {
  if (capability === "supportedDisabled") return "disabled";
  if (capability === "unsupported") return "unsupported";
  if (capability === "failed") return "failed";
  if (value == null) return "missing";
  return value === 0 ? "zero" : "value";
};

export function gpuDevices(samples: SystemSample[]): GpuSample[] {
  const devices = new Map<string, GpuSample>();
  for (const sample of samples) {
    for (const gpu of sample.gpus) {
      if (!devices.has(gpu.deviceKey)) devices.set(gpu.deviceKey, gpu);
    }
  }
  return [...devices.values()];
}

export function timelineChartSamples(samples: TimelineSample[]): TimelineSample[] {
  return samples.flatMap((sample) => {
    if (sample.sourceGapBeforeMs <= 0) return [sample];
    return [{
      ...sample,
      timestampMs: sample.timestampMs - sample.sourceGapBeforeMs,
      sourceGapBeforeMs: 0,
      cpuPercent: null,
      memoryPercent: null,
      memoryUsedBytes: null,
      memoryTotalBytes: null,
      diskReadBytesPerSec: null,
      diskWriteBytesPerSec: null,
      gpus: [],
      hasAppSnapshot: false
    }, sample];
  });
}

export function aggregateCategoryCapability(
  providers: ProviderStatus[],
  settings: CollectionSettings | null | undefined,
  category: MetricCategory
): CapabilityState | undefined {
  if (settings && !settings.enabledCategories.includes(category)) return "supportedDisabled";
  const capabilities = providers.flatMap((provider) => provider.capabilities.filter((item) => item.category === category));
  if (!capabilities.length) return undefined;
  if (capabilities.some((item) => item.state === "supportedEnabled")) return "supportedEnabled";
  if (capabilities.some((item) => item.state === "supportedDisabled")) return "supportedDisabled";
  if (capabilities.some((item) => item.state === "failed")) return "failed";
  return "unsupported";
}

export function stateDurations(intervals: ComputerStateInterval[]) {
  return intervals.reduce<Record<string, number>>((totals, interval) => {
    totals[interval.state] = (totals[interval.state] ?? 0) + interval.durationMs;
    return totals;
  }, {});
}

export function toggleCategory(settings: CollectionSettings, category: MetricCategory): CollectionSettings {
  const enabled = new Set(settings.enabledCategories);
  if (enabled.has(category)) enabled.delete(category);
  else enabled.add(category);
  return { ...settings, enabledCategories: [...enabled] };
}

export function evidenceStatusTone(status: string): "pending" | "partial" | "complete" | "failed" {
  if (status === "complete") return "complete";
  if (status === "failed") return "failed";
  if (status === "partial") return "partial";
  return "pending";
}
