import { describe, expect, it } from "vitest";
import { DEFAULT_DASHBOARD_V1 } from "./config";
import { canPersistDashboard, classifyDashboardLoad, isDashboardEditable } from "./loadState";

describe("dashboard configuration load state", () => {
  it("distinguishes a valid config from a successful empty result", () => {
    expect(classifyDashboardLoad(DEFAULT_DASHBOARD_V1, true)).toBe("loaded-config");
    expect(classifyDashboardLoad(null, false)).toBe("loaded-empty");
  });

  it("blocks fallback persistence while a load is rejected and allows retry success", () => {
    expect(isDashboardEditable("failed")).toBe(false);
    expect(canPersistDashboard("failed", true, DEFAULT_DASHBOARD_V1)).toBe(false);
    expect(isDashboardEditable("loaded-empty")).toBe(true);
    expect(canPersistDashboard("loaded-empty", true, DEFAULT_DASHBOARD_V1)).toBe(true);
    expect(isDashboardEditable("loaded-config")).toBe(true);
    expect(canPersistDashboard("loaded-config", true, DEFAULT_DASHBOARD_V1)).toBe(true);
  });
});
