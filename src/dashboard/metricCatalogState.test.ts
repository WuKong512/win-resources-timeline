import { describe, expect, it } from "vitest";
import {
  canRetryMetricCatalog,
  completeMetricCatalogLoad,
  failMetricCatalogLoad,
  hasAuthoritativeMetricCatalog,
  initialMetricCatalogLoadState,
  startMetricCatalogLoad
} from "./metricCatalogState";

describe("metric catalog load state", () => {
  it("keeps loading, loaded, and failed states distinct", () => {
    const snapshot = { metrics: [], devices: [] };
    const loaded = completeMetricCatalogLoad(snapshot);
    const failed = failMetricCatalogLoad();

    expect(initialMetricCatalogLoadState.phase).toBe("loading");
    expect(hasAuthoritativeMetricCatalog(initialMetricCatalogLoadState)).toBe(false);
    expect(startMetricCatalogLoad()).toEqual({ phase: "loading", snapshot: null });
    expect(loaded.phase).toBe("loaded");
    expect(hasAuthoritativeMetricCatalog(loaded)).toBe(true);
    expect(canRetryMetricCatalog(loaded)).toBe(false);
    expect(failed).toEqual({ phase: "failed", snapshot: null });
    expect(hasAuthoritativeMetricCatalog(failed)).toBe(false);
    expect(canRetryMetricCatalog(failed)).toBe(true);
  });
});
