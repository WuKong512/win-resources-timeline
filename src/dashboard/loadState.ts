import type { DashboardConfig } from "./config";

export type DashboardLoadState = "loading" | "loaded-config" | "loaded-empty" | "failed";

export function classifyDashboardLoad(config: DashboardConfig | null, valid: boolean): DashboardLoadState {
  return config && valid ? "loaded-config" : "loaded-empty";
}

export function isDashboardEditable(state: DashboardLoadState): boolean {
  return state === "loaded-config" || state === "loaded-empty";
}

export function canPersistDashboard(
  state: DashboardLoadState,
  dirty: boolean,
  config: DashboardConfig | null
): config is DashboardConfig {
  return isDashboardEditable(state) && dirty && config != null;
}
