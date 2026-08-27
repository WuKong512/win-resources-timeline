import type { Language, TranslationKey } from "../i18n";
import type { GpuSample, SystemSample } from "../types/resource";
import { formatBytes } from "../utils/time";

export type SystemMetricId =
  | "system.cpu.usage_pct"
  | "system.memory.usage_pct"
  | "system.memory.used_bytes"
  | "system.disk.read_bps"
  | "system.disk.write_bps";

export type GpuMetricField =
  | "utilization_pct"
  | "memory_controller_utilization_pct"
  | "temperature_c"
  | "board_power_w"
  | "graphics_clock_mhz"
  | "memory_clock_mhz"
  | "vram_used_bytes"
  | "vram_total_bytes";

export type MetricId = SystemMetricId | `gpu.${string}.${GpuMetricField}`;

export type UnitFamily = "percent" | "bytes" | "throughput" | "temperature" | "power" | "frequency";

export type MetricDimension = "system" | "gpu";

export type AvailabilityRequirement =
  | { kind: "system-field"; field: keyof Pick<SystemSample, "cpuPercent" | "memoryPercent" | "memoryUsedBytes" | "diskReadBytesPerSec" | "diskWriteBytesPerSec"> }
  | { kind: "gpu-field"; field: GpuMetricField };

export type MetricFormatter = (value: number, language: Language) => string;

export interface MetricDescriptor {
  id: MetricId;
  translationKey: TranslationKey;
  dimension: MetricDimension;
  unitFamily: UnitFamily;
  unitLabel: "%" | "bytes" | "B/s" | "°C" | "W" | "MHz";
  formatter: MetricFormatter;
  availabilityRequirement: AvailabilityRequirement;
  defaultChartEligibility: boolean;
  deviceKey?: string;
  gpuField?: GpuMetricField;
  getValue: (sample: SystemSample) => number | null;
}

export const SYSTEM_METRIC_IDS = {
  cpuUsage: "system.cpu.usage_pct",
  memoryUsage: "system.memory.usage_pct",
  memoryUsed: "system.memory.used_bytes",
  diskRead: "system.disk.read_bps",
  diskWrite: "system.disk.write_bps"
} as const satisfies Record<string, SystemMetricId>;

export const GPU_METRIC_FIELDS = [
  "utilization_pct",
  "memory_controller_utilization_pct",
  "temperature_c",
  "board_power_w",
  "graphics_clock_mhz",
  "memory_clock_mhz",
  "vram_used_bytes",
  "vram_total_bytes"
] as const satisfies readonly GpuMetricField[];

const percent: MetricFormatter = (value) => `${value.toFixed(1)}%`;
const bytes: MetricFormatter = (value, language) => formatBytes(value, language);
const throughput: MetricFormatter = (value, language) => `${formatBytes(value, language)}/s`;
const temperature: MetricFormatter = (value) => `${value.toFixed(1)} °C`;
const power: MetricFormatter = (value) => `${value.toFixed(1)} W`;
const frequency: MetricFormatter = (value) => `${value.toFixed(0)} MHz`;

type StaticMetricDefinition = Omit<MetricDescriptor, "id" | "deviceKey" | "gpuField">;

const SYSTEM_METRIC_DEFINITIONS: Record<SystemMetricId, StaticMetricDefinition> = {
  [SYSTEM_METRIC_IDS.cpuUsage]: {
    translationKey: "metricCpu",
    dimension: "system",
    unitFamily: "percent",
    unitLabel: "%",
    formatter: percent,
    availabilityRequirement: { kind: "system-field", field: "cpuPercent" },
    defaultChartEligibility: true,
    getValue: (sample) => sample.cpuPercent
  },
  [SYSTEM_METRIC_IDS.memoryUsage]: {
    translationKey: "metricMemory",
    dimension: "system",
    unitFamily: "percent",
    unitLabel: "%",
    formatter: percent,
    availabilityRequirement: { kind: "system-field", field: "memoryPercent" },
    defaultChartEligibility: true,
    getValue: (sample) => sample.memoryPercent
  },
  [SYSTEM_METRIC_IDS.memoryUsed]: {
    translationKey: "metricMemoryUsed",
    dimension: "system",
    unitFamily: "bytes",
    unitLabel: "bytes",
    formatter: bytes,
    availabilityRequirement: { kind: "system-field", field: "memoryUsedBytes" },
    defaultChartEligibility: false,
    getValue: (sample) => sample.memoryUsedBytes
  },
  [SYSTEM_METRIC_IDS.diskRead]: {
    translationKey: "metricDiskRead",
    dimension: "system",
    unitFamily: "throughput",
    unitLabel: "B/s",
    formatter: throughput,
    availabilityRequirement: { kind: "system-field", field: "diskReadBytesPerSec" },
    defaultChartEligibility: true,
    getValue: (sample) => sample.diskReadBytesPerSec
  },
  [SYSTEM_METRIC_IDS.diskWrite]: {
    translationKey: "metricDiskWrite",
    dimension: "system",
    unitFamily: "throughput",
    unitLabel: "B/s",
    formatter: throughput,
    availabilityRequirement: { kind: "system-field", field: "diskWriteBytesPerSec" },
    defaultChartEligibility: true,
    getValue: (sample) => sample.diskWriteBytesPerSec
  }
};

type GpuMetricDefinition = Omit<StaticMetricDefinition, "dimension" | "availabilityRequirement" | "getValue">;

const GPU_METRIC_DEFINITIONS: Record<GpuMetricField, GpuMetricDefinition> = {
  utilization_pct: { translationKey: "metricGpuUsage", unitFamily: "percent", unitLabel: "%", formatter: percent, defaultChartEligibility: true },
  memory_controller_utilization_pct: { translationKey: "metricGpuMemoryController", unitFamily: "percent", unitLabel: "%", formatter: percent, defaultChartEligibility: false },
  temperature_c: { translationKey: "metricGpuTemp", unitFamily: "temperature", unitLabel: "°C", formatter: temperature, defaultChartEligibility: true },
  board_power_w: { translationKey: "metricGpuPower", unitFamily: "power", unitLabel: "W", formatter: power, defaultChartEligibility: false },
  graphics_clock_mhz: { translationKey: "metricGpuGraphicsClock", unitFamily: "frequency", unitLabel: "MHz", formatter: frequency, defaultChartEligibility: false },
  memory_clock_mhz: { translationKey: "metricGpuMemoryClock", unitFamily: "frequency", unitLabel: "MHz", formatter: frequency, defaultChartEligibility: false },
  vram_used_bytes: { translationKey: "metricGpuVram", unitFamily: "bytes", unitLabel: "bytes", formatter: bytes, defaultChartEligibility: false },
  vram_total_bytes: { translationKey: "metricGpuVramTotal", unitFamily: "bytes", unitLabel: "bytes", formatter: bytes, defaultChartEligibility: false }
};

function gpuValue(gpu: GpuSample, field: GpuMetricField): number | null {
  switch (field) {
    case "utilization_pct": return gpu.utilizationPercent;
    case "memory_controller_utilization_pct": return gpu.memoryControllerUtilizationPercent;
    case "temperature_c": return gpu.temperatureCelsius;
    case "board_power_w": return gpu.powerWatts;
    case "graphics_clock_mhz": return gpu.graphicsClockMhz;
    case "memory_clock_mhz": return gpu.memoryClockMhz;
    case "vram_used_bytes": return gpu.vramUsedBytes;
    case "vram_total_bytes": return gpu.vramTotalBytes;
  }
}

export function gpuMetricId(deviceKey: string, field: GpuMetricField): MetricId {
  return `gpu.${deviceKey}.${field}`;
}

export function parseGpuMetricId(id: string): { deviceKey: string; field: GpuMetricField } | null {
  if (!id.startsWith("gpu.")) return null;
  const body = id.slice("gpu.".length);
  for (const field of GPU_METRIC_FIELDS) {
    const suffix = `.${field}`;
    if (!body.endsWith(suffix)) continue;
    const deviceKey = body.slice(0, -suffix.length);
    return deviceKey.trim() ? { deviceKey, field } : null;
  }
  return null;
}

export function getMetricDescriptor(id: string): MetricDescriptor | null {
  const staticDefinition = SYSTEM_METRIC_DEFINITIONS[id as SystemMetricId];
  if (staticDefinition) return { id: id as SystemMetricId, ...staticDefinition };
  const parsed = parseGpuMetricId(id);
  if (!parsed) return null;
  const definition = GPU_METRIC_DEFINITIONS[parsed.field];
  return {
    id: id as MetricId,
    ...definition,
    dimension: "gpu",
    availabilityRequirement: { kind: "gpu-field", field: parsed.field },
    deviceKey: parsed.deviceKey,
    gpuField: parsed.field,
    getValue: (sample) => {
      const gpu = sample.gpus.find((item) => item.deviceKey === parsed.deviceKey);
      return gpu ? gpuValue(gpu, parsed.field) : null;
    }
  };
}

export function isMetricId(value: string): value is MetricId {
  return getMetricDescriptor(value) != null;
}

export function metricValue(descriptor: MetricDescriptor, sample: SystemSample): number | null {
  const value = descriptor.getValue(sample);
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

export function hasMetricData(id: string, samples: readonly SystemSample[]): boolean {
  const descriptor = getMetricDescriptor(id);
  return descriptor != null && samples.some((sample) => metricValue(descriptor, sample) != null);
}

export function getAvailableMetricDescriptors(samples: readonly SystemSample[]): MetricDescriptor[] {
  const descriptors = Object.keys(SYSTEM_METRIC_DEFINITIONS)
    .filter((id) => hasMetricData(id, samples))
    .map((id) => getMetricDescriptor(id) as MetricDescriptor);
  const deviceKeys = [...new Set(samples.flatMap((sample) => sample.gpus.map((gpu) => gpu.deviceKey)))];
  for (const deviceKey of deviceKeys) {
    for (const field of GPU_METRIC_FIELDS) {
      const id = gpuMetricId(deviceKey, field);
      if (hasMetricData(id, samples)) descriptors.push(getMetricDescriptor(id) as MetricDescriptor);
    }
  }
  return descriptors;
}

export function formatMetricValue(descriptor: MetricDescriptor, value: number | null, language: Language): string | null {
  return value == null || !Number.isFinite(value) ? null : descriptor.formatter(value, language);
}

export function gpuDeviceLabel(gpu: Pick<GpuSample, "deviceKey" | "vendor" | "model">): string {
  return [gpu.vendor, gpu.model].filter(Boolean).join(" ") || gpu.deviceKey;
}

export function metricDisplayName(
  descriptor: MetricDescriptor,
  t: (key: TranslationKey) => string,
  samples: readonly SystemSample[] = []
): string {
  const base = t(descriptor.translationKey);
  if (!descriptor.deviceKey) return base;
  const gpu = samples.flatMap((sample) => sample.gpus).find((item) => item.deviceKey === descriptor.deviceKey);
  return `${base} · ${gpu ? gpuDeviceLabel(gpu) : descriptor.deviceKey}`;
}

export function systemMetricDescriptors(): MetricDescriptor[] {
  return Object.keys(SYSTEM_METRIC_DEFINITIONS).map((id) => getMetricDescriptor(id) as MetricDescriptor);
}
