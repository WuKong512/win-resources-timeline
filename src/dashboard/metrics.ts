import type { Language, TranslationKey } from "../i18n";
import type {
  CollectionSettings,
  GpuSample,
  MetricCatalogDevice,
  MetricCatalogEntry,
  MetricCatalogSnapshot,
  MetricCategory,
  ProviderStatus,
  SystemSample
} from "../types/resource";
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

export type MetricUiStatus = "AVAILABLE" | "NO_DATA_IN_RANGE" | "DISABLED" | "UNSUPPORTED" | "FAILED" | "DEGRADED" | "UNKNOWN";

export type CurrentReadingPresentation = "VALUE" | "NO_CURRENT_READING" | "STATUS";

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

export interface MetricCatalogItem {
  id: MetricId;
  descriptor: MetricDescriptor;
  category: MetricCategory;
  providerId: string;
  providerDisplayName: string;
  device: MetricCatalogDevice | null;
  status: MetricUiStatus;
  rawSupportStatus: MetricCatalogEntry["supportStatus"] | "unknown";
}

export const UNIT_FAMILY_ORDER: readonly UnitFamily[] = ["percent", "throughput", "bytes", "temperature", "power", "frequency"];

export function isTrendMetricSelectable(item: Pick<MetricCatalogItem, "status">): boolean {
  return item.status !== "UNSUPPORTED" && item.status !== "UNKNOWN";
}

export function trendFamilies(catalog: readonly MetricCatalogItem[]): UnitFamily[] {
  const families = new Set(catalog.filter(isTrendMetricSelectable).map((item) => item.descriptor.unitFamily));
  return UNIT_FAMILY_ORDER.filter((family) => families.has(family));
}

/**
 * Range availability and the latest reading are separate facts. A metric can have a valid
 * sample earlier in the range while the most recent sample has no value.
 */
export function currentReadingPresentation(status: MetricUiStatus, value: number | null): CurrentReadingPresentation {
  if ((status === "AVAILABLE" || status === "DEGRADED") && value != null && Number.isFinite(value)) return "VALUE";
  if (status === "AVAILABLE" || status === "DEGRADED") return "NO_CURRENT_READING";
  return "STATUS";
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

export const SYSTEM_METRIC_PROVIDER_ID = "windows-baseline";
export const GPU_METRIC_PROVIDER_ID = "nvidia-nvml";

const GPU_RUNTIME_METRIC_KEYS: Record<GpuMetricField, string> = {
  utilization_pct: "gpu.utilization_percent",
  memory_controller_utilization_pct: "gpu.memory_controller_utilization_percent",
  temperature_c: "gpu.temperature_celsius",
  board_power_w: "gpu.power_watts",
  graphics_clock_mhz: "gpu.graphics_clock_mhz",
  memory_clock_mhz: "gpu.memory_clock_mhz",
  vram_used_bytes: "gpu.vram_used_bytes",
  vram_total_bytes: "gpu.vram_total_bytes"
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

/**
 * Data-only helper for chart construction. It answers “which descriptors have a value in these
 * samples?” and must not be used as the product metric catalog or customization source.
 */
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

export function gpuDeviceLabel(gpu: Pick<GpuSample, "deviceKey" | "vendor" | "model"> | MetricCatalogDevice): string {
  const stableKey = "deviceKey" in gpu ? gpu.deviceKey : gpu.stableKey;
  return [gpu.vendor, gpu.model].filter(Boolean).join(" ") || stableKey;
}

export function metricDisplayName(
  descriptor: MetricDescriptor,
  t: (key: TranslationKey) => string,
  samples: readonly SystemSample[] = [],
  devices: readonly MetricCatalogDevice[] = []
): string {
  const base = t(descriptor.translationKey);
  if (!descriptor.deviceKey) return base;
  const gpu = samples.flatMap((sample) => sample.gpus).find((item) => item.deviceKey === descriptor.deviceKey);
  const catalogDevice = devices.find((item) => item.stableKey === descriptor.deviceKey);
  return `${base} · ${gpu ? gpuDeviceLabel(gpu) : catalogDevice ? gpuDeviceLabel(catalogDevice) : descriptor.deviceKey}`;
}

export function systemMetricDescriptors(): MetricDescriptor[] {
  return Object.keys(SYSTEM_METRIC_DEFINITIONS).map((id) => getMetricDescriptor(id) as MetricDescriptor);
}

export type BuildMetricCatalogOptions = {
  snapshot?: MetricCatalogSnapshot | null;
  samples: readonly SystemSample[];
  providers: readonly ProviderStatus[];
  settings?: CollectionSettings | null;
};

/**
 * Maps a frontend descriptor to the provider-neutral key stored in
 * `collection_session_metric`. This is deliberately separate from the dashboard MetricId:
 * device identity belongs in the MetricId, while the provider key remains stable across devices.
 */
export function runtimeMetricKey(descriptor: MetricDescriptor): string {
  return descriptor.gpuField ? GPU_RUNTIME_METRIC_KEYS[descriptor.gpuField] : descriptor.id;
}

/**
 * Builds the product metric catalog from capability metadata plus the current range. The range
 * is used only for the final `AVAILABLE`/`NO_DATA_IN_RANGE` distinction; it never controls which
 * descriptors are present.
 */
export function buildMetricCatalog({ snapshot, samples, providers, settings }: BuildMetricCatalogOptions): MetricCatalogItem[] {
  const entries = snapshot?.metrics ?? [];
  // Sample-only identities are a deliberate degraded fallback. Once the authoritative snapshot
  // is loaded, it remains the source of device existence instead of the selected range.
  const devices = mergeGpuDevices(snapshot?.devices ?? [], entries, snapshot == null ? samples : []);
  const items: MetricCatalogItem[] = [];

  for (const descriptor of systemMetricDescriptors()) {
    const category = systemMetricCategory(descriptor.id);
    const entry = entries.find((candidate) => candidate.metricKey === runtimeMetricKey(descriptor) && candidate.device == null);
    items.push(createCatalogItem({ descriptor, category, providerId: entry?.providerId ?? SYSTEM_METRIC_PROVIDER_ID, device: null, entry, providers, settings, samples }));
  }

  const gpuEntries = entries.filter((entry) => entry.category === "gpu");
  const gpuKnown = devices.length > 0
    || gpuEntries.length > 0
    || providers.some((provider) => provider.capabilities.some((capability) => capability.category === "gpu"))
    || samples.some((sample) => sample.gpus.length > 0);
  // A failed catalog request may still leave provider capability truth and current system
  // samples. Do not turn that into a synthetic provider/device identity: only observed device
  // keys may seed a degraded GPU fallback. An authoritative snapshot may intentionally expose
  // provider-level rows before a device is discovered, so that path remains available.
  if (gpuKnown && (devices.length > 0 || snapshot != null)) {
    const gpuProviderId = gpuEntries[0]?.providerId ?? GPU_METRIC_PROVIDER_ID;
    if (devices.length) {
      for (const device of devices) {
        for (const field of GPU_METRIC_FIELDS) {
          const descriptor = getMetricDescriptor(gpuMetricId(device.stableKey, field));
          if (!descriptor) continue;
          const runtimeKey = GPU_RUNTIME_METRIC_KEYS[field];
          const entry = gpuEntries.find((candidate) => candidate.metricKey === runtimeKey && candidate.device?.stableKey === device.stableKey)
            ?? gpuEntries.find((candidate) => candidate.metricKey === runtimeKey && candidate.device == null);
          items.push(createCatalogItem({ descriptor, category: "gpu", providerId: entry?.providerId ?? gpuProviderId, device, entry, providers, settings, samples }));
        }
      }
    } else {
      // A provider can report the registered field set without exposing a device (disabled,
      // unsupported, or not yet discovered). Keep those rows visible as provider-level entries.
      const providerDeviceKey = `provider:${gpuProviderId}`;
      for (const field of GPU_METRIC_FIELDS) {
        const descriptor = getMetricDescriptor(gpuMetricId(providerDeviceKey, field));
        if (!descriptor) continue;
        const runtimeKey = GPU_RUNTIME_METRIC_KEYS[field];
        const entry = gpuEntries.find((candidate) => candidate.metricKey === runtimeKey && candidate.device == null)
          ?? gpuEntries.find((candidate) => candidate.metricKey === runtimeKey);
        items.push(createCatalogItem({ descriptor, category: "gpu", providerId: entry?.providerId ?? gpuProviderId, device: null, entry, providers, settings, samples }));
      }
    }
  }

  return items.filter((item, index, all) => all.findIndex((candidate) => candidate.id === item.id) === index);
}

export function metricItemDisplayName(item: MetricCatalogItem, t: (key: TranslationKey) => string, samples: readonly SystemSample[] = []): string {
  if (!item.device && item.descriptor.dimension === "gpu") {
    return `${t(item.descriptor.translationKey)} · ${item.providerDisplayName}`;
  }
  return metricDisplayName(item.descriptor, t, samples, item.device ? [item.device] : []);
}

function createCatalogItem({
  descriptor,
  category,
  providerId,
  device,
  entry,
  providers,
  settings,
  samples
}: {
  descriptor: MetricDescriptor;
  category: MetricCategory;
  providerId: string;
  device: MetricCatalogDevice | null;
  entry: MetricCatalogEntry | undefined;
  providers: readonly ProviderStatus[];
  settings?: CollectionSettings | null;
  samples: readonly SystemSample[];
}): MetricCatalogItem {
  const provider = providers.find((candidate) => candidate.providerId === providerId);
  return {
    id: descriptor.id,
    descriptor,
    category,
    providerId,
    providerDisplayName: provider?.displayName ?? (providerId === GPU_METRIC_PROVIDER_ID ? "NVIDIA GPU" : providerId),
    device,
    status: projectMetricStatus({ descriptor, category, providerId, entry, provider, providers, settings, samples }),
    rawSupportStatus: entry?.supportStatus ?? "unknown"
  };
}

function projectMetricStatus({
  descriptor,
  category,
  providerId,
  entry,
  provider,
  providers,
  settings,
  samples
}: {
  descriptor: MetricDescriptor;
  category: MetricCategory;
  providerId: string;
  entry: MetricCatalogEntry | undefined;
  provider: ProviderStatus | undefined;
  providers: readonly ProviderStatus[];
  settings?: CollectionSettings | null;
  samples: readonly SystemSample[];
}): MetricUiStatus {
  const raw = entry?.supportStatus;
  const capability = provider?.capabilities.find((candidate) => candidate.category === category);
  const categoryDisabled = settings != null && !settings.enabledCategories.includes(category);
  const providerDisabled = settings?.disabledProviders.some((disabledProvider) => disabledProvider.trim().toLowerCase() === providerId.trim().toLowerCase()) === true;
  if (categoryDisabled || entry?.enabled === false || capability?.state === "supportedDisabled" || providerDisabled) return "DISABLED";

  if (raw === "probe_failed" || raw === "failed" || capability?.state === "failed" || provider?.lifecycle === "failed") return "FAILED";
  if (raw === "unsupported" || raw === "permission_denied" || raw === "provider_missing" || provider?.supported === false || capability?.state === "unsupported") return "UNSUPPORTED";

  const providerHasCapability = capability?.supportStatus === "supported"
    || provider?.supported === true
    || raw === "supported"
    || providers.some((candidate) => candidate.providerId === providerId && candidate.supported);
  if ((provider?.lifecycle === "paused" || provider?.lastError != null) && providerHasCapability) return "DEGRADED";
  if (hasMetricData(descriptor.id, samples)) return "AVAILABLE";
  if (providerHasCapability) return "NO_DATA_IN_RANGE";
  return "UNKNOWN";
}

function systemMetricCategory(id: MetricId): MetricCategory {
  if (id === SYSTEM_METRIC_IDS.cpuUsage) return "cpu";
  if (id === SYSTEM_METRIC_IDS.memoryUsage || id === SYSTEM_METRIC_IDS.memoryUsed) return "memory";
  return "disk";
}

function mergeGpuDevices(
  snapshotDevices: readonly MetricCatalogDevice[],
  entries: readonly MetricCatalogEntry[],
  samples: readonly SystemSample[]
): MetricCatalogDevice[] {
  const devices = new Map<string, MetricCatalogDevice>();
  for (const device of snapshotDevices) devices.set(device.stableKey, device);
  for (const entry of entries) {
    if (entry.device) devices.set(entry.device.stableKey, entry.device);
  }
  for (const sample of samples) {
    for (const gpu of sample.gpus) {
      if (!devices.has(gpu.deviceKey)) {
        devices.set(gpu.deviceKey, {
          stableKey: gpu.deviceKey,
          vendor: gpu.vendor,
          model: gpu.model,
          capacityBytes: gpu.capacityBytes
        });
      }
    }
  }
  return [...devices.values()].sort((left, right) => left.stableKey.localeCompare(right.stableKey));
}
