import type { SystemSample } from "../types/resource";
import {
  getAvailableMetricDescriptors,
  getMetricDescriptor,
  hasMetricData,
  isMetricId,
  type MetricId,
  type UnitFamily,
  SYSTEM_METRIC_IDS,
  gpuMetricId
} from "./metrics";

export const DASHBOARD_CONFIG_VERSION = 1 as const;
export const MAX_DASHBOARD_CARDS = 12;
export const MAX_METRICS_PER_CARD = 8;
export const MAX_DASHBOARD_JSON_BYTES = 32_768;

export interface DashboardCardConfig {
  id: string;
  metricIds: MetricId[];
  hiddenMetricIds: MetricId[];
  order: number;
  visible: boolean;
}

export interface DashboardConfig {
  version: typeof DASHBOARD_CONFIG_VERSION;
  cards: DashboardCardConfig[];
}

export const DEFAULT_DASHBOARD_V1: DashboardConfig = {
  version: DASHBOARD_CONFIG_VERSION,
  cards: [
    {
      id: "compute-usage",
      metricIds: [SYSTEM_METRIC_IDS.cpuUsage],
      hiddenMetricIds: [],
      order: 0,
      visible: true
    },
    {
      id: "memory",
      metricIds: [SYSTEM_METRIC_IDS.memoryUsage],
      hiddenMetricIds: [],
      order: 1,
      visible: true
    },
    {
      id: "disk-io",
      metricIds: [SYSTEM_METRIC_IDS.diskRead, SYSTEM_METRIC_IDS.diskWrite],
      hiddenMetricIds: [],
      order: 2,
      visible: true
    }
  ]
};

export type DashboardValidation =
  | { ok: true; config: DashboardConfig }
  | { ok: false; reason: "invalid" | "unknown-metric" | "incompatible" | "too-large" };

export type MetricSelectionValidation =
  | { ok: true }
  | { ok: false; reason: "empty" | "unknown-metric" | "incompatible"; metricId?: string };

export function validateMetricSelection(metricIds: readonly string[]): MetricSelectionValidation {
  if (!metricIds.length) return { ok: false, reason: "empty" };
  const descriptors = metricIds.map((id) => getMetricDescriptor(id));
  const unknownIndex = descriptors.findIndex((descriptor) => descriptor == null);
  if (unknownIndex >= 0) return { ok: false, reason: "unknown-metric", metricId: metricIds[unknownIndex] };
  const family = descriptors[0]?.unitFamily;
  if (descriptors.some((descriptor) => descriptor?.unitFamily !== family)) {
    return { ok: false, reason: "incompatible", metricId: metricIds.find((id) => getMetricDescriptor(id)?.unitFamily !== family) };
  }
  return { ok: true };
}

export function validateDashboardConfig(input: unknown): DashboardValidation {
  if (!input || typeof input !== "object") return { ok: false, reason: "invalid" };
  const raw = input as { version?: unknown; cards?: unknown };
  if (raw.version !== DASHBOARD_CONFIG_VERSION || !Array.isArray(raw.cards) || raw.cards.length > MAX_DASHBOARD_CARDS) {
    return { ok: false, reason: "invalid" };
  }
  const cardIds = new Set<string>();
  const orders = new Set<number>();
  const cards: DashboardCardConfig[] = [];
  for (const rawCard of raw.cards) {
    if (!rawCard || typeof rawCard !== "object") return { ok: false, reason: "invalid" };
    const card = rawCard as { id?: unknown; metricIds?: unknown; hiddenMetricIds?: unknown; order?: unknown; visible?: unknown };
    if (
      typeof card.id !== "string" ||
      card.id.trim().length === 0 ||
      card.id.length > 64 ||
      cardIds.has(card.id) ||
      !Array.isArray(card.metricIds) ||
      card.metricIds.length === 0 ||
      card.metricIds.length > MAX_METRICS_PER_CARD ||
      (card.hiddenMetricIds != null && !Array.isArray(card.hiddenMetricIds)) ||
      typeof card.order !== "number" ||
      !Number.isInteger(card.order) ||
      card.order < 0 ||
      card.order >= MAX_DASHBOARD_CARDS ||
      orders.has(card.order) ||
      typeof card.visible !== "boolean"
    ) {
      return { ok: false, reason: "invalid" };
    }
    const metricIds = card.metricIds.filter((id): id is string => typeof id === "string");
    if (metricIds.length !== card.metricIds.length || new Set(metricIds).size !== metricIds.length) {
      return { ok: false, reason: "invalid" };
    }
    const selection = validateMetricSelection(metricIds);
    if (!selection.ok) return { ok: false, reason: selection.reason === "unknown-metric" ? "unknown-metric" : selection.reason === "incompatible" ? "incompatible" : "invalid" };
    const hiddenMetricIds = (Array.isArray(card.hiddenMetricIds) ? card.hiddenMetricIds : []).filter((id): id is string => typeof id === "string");
    if (hiddenMetricIds.length !== (Array.isArray(card.hiddenMetricIds) ? card.hiddenMetricIds.length : 0) || hiddenMetricIds.length > metricIds.length) {
      return { ok: false, reason: "invalid" };
    }
    if (new Set(hiddenMetricIds).size !== hiddenMetricIds.length || hiddenMetricIds.some((id) => !metricIds.includes(id))) {
      return { ok: false, reason: "invalid" };
    }
    if (metricIds.some((id) => !isMetricId(id))) return { ok: false, reason: "unknown-metric" };
    cardIds.add(card.id);
    orders.add(card.order);
    cards.push({
      id: card.id,
      metricIds: metricIds as MetricId[],
      hiddenMetricIds: hiddenMetricIds as MetricId[],
      order: card.order,
      visible: card.visible
    });
  }
  const config: DashboardConfig = { version: DASHBOARD_CONFIG_VERSION, cards };
  if (serializedDashboardSize(config) > MAX_DASHBOARD_JSON_BYTES) return { ok: false, reason: "too-large" };
  return { ok: true, config };
}

export function deserializeDashboardConfig(value: string | null | undefined): DashboardConfig | null {
  if (!value || value.length > MAX_DASHBOARD_JSON_BYTES) return null;
  try {
    const result = validateDashboardConfig(JSON.parse(value));
    return result.ok ? result.config : null;
  } catch {
    return null;
  }
}

export function serializeDashboardConfig(config: DashboardConfig): string {
  const result = validateDashboardConfig(config);
  if (!result.ok) throw new Error(`Invalid dashboard config: ${result.reason}`);
  const serialized = JSON.stringify(result.config);
  if (new TextEncoder().encode(serialized).byteLength > MAX_DASHBOARD_JSON_BYTES) throw new Error("Dashboard config is too large");
  return serialized;
}

function serializedDashboardSize(config: DashboardConfig): number {
  try {
    return new TextEncoder().encode(JSON.stringify(config)).byteLength;
  } catch {
    return MAX_DASHBOARD_JSON_BYTES + 1;
  }
}

export function createDefaultDashboardConfig(samples: readonly SystemSample[]): DashboardConfig {
  const availableIds = new Set(getAvailableMetricDescriptors(samples).map((descriptor) => descriptor.id));
  const cards: DashboardCardConfig[] = [];
  const computeMetrics: MetricId[] = [];
  if (availableIds.has(SYSTEM_METRIC_IDS.cpuUsage)) computeMetrics.push(SYSTEM_METRIC_IDS.cpuUsage);
  const gpuUsageIds = [...availableIds].filter((id): id is MetricId => id.startsWith("gpu.") && id.endsWith(".utilization_pct"));
  computeMetrics.push(...gpuUsageIds);
  if (computeMetrics.length) cards.push(makeCard("compute-usage", computeMetrics, cards.length));

  if (availableIds.has(SYSTEM_METRIC_IDS.memoryUsage)) cards.push(makeCard("memory", [SYSTEM_METRIC_IDS.memoryUsage], cards.length));
  const diskMetrics = [SYSTEM_METRIC_IDS.diskRead, SYSTEM_METRIC_IDS.diskWrite].filter((id) => availableIds.has(id));
  if (diskMetrics.length) cards.push(makeCard("disk-io", diskMetrics, cards.length));

  const gpuTemperatureIds = [...availableIds].filter((id): id is MetricId => id.startsWith("gpu.") && id.endsWith(".temperature_c"));
  if (gpuTemperatureIds.length) cards.push(makeCard("gpu-temperature", gpuTemperatureIds, cards.length));

  return { version: DASHBOARD_CONFIG_VERSION, cards };
}

function makeCard(id: string, metricIds: MetricId[], order: number): DashboardCardConfig {
  return { id, metricIds, hiddenMetricIds: [], order, visible: true };
}

export function reorderDashboardCards(config: DashboardConfig, cardId: string, direction: -1 | 1): DashboardConfig {
  const cards = [...config.cards].sort((left, right) => left.order - right.order);
  const index = cards.findIndex((card) => card.id === cardId);
  const nextIndex = index + direction;
  if (index < 0 || nextIndex < 0 || nextIndex >= cards.length) return config;
  [cards[index], cards[nextIndex]] = [cards[nextIndex], cards[index]];
  return { ...config, cards: cards.map((card, order) => ({ ...card, order })) };
}

export function cardMetricFamilies(metricIds: readonly string[]): UnitFamily[] {
  return [...new Set(metricIds.flatMap((id) => {
    const family = getMetricDescriptor(id)?.unitFamily;
    return family ? [family] : [];
  }))];
}

export function canAddMetricToCard(card: DashboardCardConfig, metricId: string): boolean {
  return !card.metricIds.includes(metricId as MetricId) && validateMetricSelection([...card.metricIds, metricId]).ok;
}

export function defaultGpuMetricId(deviceKey: string, field: "utilization_pct" | "temperature_c"): MetricId {
  return gpuMetricId(deviceKey, field);
}

export function metricIsCurrentlyAvailable(metricId: MetricId, samples: readonly SystemSample[]): boolean {
  return hasMetricData(metricId, samples);
}
