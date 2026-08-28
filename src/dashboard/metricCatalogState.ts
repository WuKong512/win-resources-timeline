import type { MetricCatalogSnapshot } from "../types/resource";

export type MetricCatalogLoadPhase = "loading" | "loaded" | "failed";

export interface MetricCatalogLoadState {
  phase: MetricCatalogLoadPhase;
  snapshot: MetricCatalogSnapshot | null;
}

export const initialMetricCatalogLoadState: MetricCatalogLoadState = {
  phase: "loading",
  snapshot: null
};

export function startMetricCatalogLoad(): MetricCatalogLoadState {
  return { phase: "loading", snapshot: null };
}

export function completeMetricCatalogLoad(snapshot: MetricCatalogSnapshot): MetricCatalogLoadState {
  return { phase: "loaded", snapshot };
}

export function failMetricCatalogLoad(): MetricCatalogLoadState {
  return { phase: "failed", snapshot: null };
}

export function hasAuthoritativeMetricCatalog(state: MetricCatalogLoadState): boolean {
  return state.phase === "loaded" && state.snapshot != null;
}

export function canRetryMetricCatalog(state: MetricCatalogLoadState): boolean {
  return state.phase === "failed";
}
