import type {
  CapabilityState,
  CollectionSettings,
  ComputerStateInterval,
  GpuSample,
  MetricCategory,
  ProviderStatus,
  TimelineGap,
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

export const MIN_INFERRED_SAMPLE_GAP_MS = 15_000;

export function inferSampleGaps(samples: readonly SystemSample[]): TimelineGap[] {
  const ordered = [...samples].sort((left, right) => left.timestampMs - right.timestampMs);
  const gaps: TimelineGap[] = [];
  for (let index = 1; index < ordered.length; index += 1) {
    const previous = ordered[index - 1];
    const current = ordered[index];
    const deltaMs = current.timestampMs - previous.timestampMs;
    const gapThresholdMs = Math.max(
      MIN_INFERRED_SAMPLE_GAP_MS,
      Math.max(0, previous.sampleDurationMs) * 3
    );
    if (deltaMs <= gapThresholdMs) continue;
    const startMs = previous.timestampMs + Math.max(0, previous.sampleDurationMs);
    if (startMs >= current.timestampMs) continue;
    gaps.push({
      startMs,
      endMs: current.timestampMs,
      durationMs: current.timestampMs - startMs
    });
  }
  return gaps;
}

export function timelineChartSamples(samples: readonly SystemSample[], gaps: readonly TimelineGap[]): SystemSample[] {
  if (!gaps.length) return [...samples];
  const gapMarkers = gaps.map((gap) => ({
    timestampMs: gap.startMs,
    sampleDurationMs: 0,
    cpuPercent: null,
    memoryPercent: null,
    memoryUsedBytes: null,
    memoryTotalBytes: null,
    diskReadBytesPerSec: null,
    diskWriteBytesPerSec: null,
    gpus: [],
    hasAppSnapshot: false
  } satisfies SystemSample));
  return [...samples, ...gapMarkers].sort((left, right) => left.timestampMs - right.timestampMs);
}

export function timelineRefreshIntervalMs(preset: 1 | 7 | 30, isCurrentDate: boolean): number | undefined {
  if (!isCurrentDate || preset === 30) return undefined;
  return preset === 1 ? 5_000 : 60_000;
}

export function timelineCoverageState(coverage: number): "complete" | "incomplete" {
  return coverage >= 0.999 ? "complete" : "incomplete";
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
