import type {
  CapabilityState,
  CollectionSettings,
  ComputerStateInterval,
  GpuSample,
  MetricCategory,
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
