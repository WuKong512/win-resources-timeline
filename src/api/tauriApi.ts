import { invoke } from "@tauri-apps/api/core";
import type { DashboardConfig } from "../dashboard/config";
import type { AppIdentity, AppResourceHistoryPoint, AppResourceSample, CollectionSettings, CollectorStatus, CrashCaseSummary, CrashDetectorStatus, CrashEvidenceDetail, DailyUsageSummary, ForegroundInterval, MetricCatalogSnapshot, ResourceApp, StorageUsage, SystemSample, TimelineQueryResult, TodayOverview, UsageSummary } from "../types/resource";

export const getTodayOverview = (startMs: number, endMs: number) =>
  invoke<TodayOverview>("get_today_overview", { startMs, endMs });

export const getOverviewAvailableDates = () => invoke<string[]>("get_overview_available_dates");

export const getDailyUsageSummary = (localDate: string, includeHidden: boolean) =>
  invoke<DailyUsageSummary[]>("get_daily_usage_summary", { localDate, includeHidden });

export const getUsageSummary = (startMs: number, endMs: number, includeHidden: boolean) =>
  invoke<UsageSummary>("get_usage_summary", { startMs, endMs, includeHidden });

export const getAppUsageTimeline = (startMs: number, endMs: number, includeHidden: boolean, includeIdle: boolean) =>
  invoke<ForegroundInterval[]>("get_app_usage_timeline", { startMs, endMs, includeHidden, includeIdle });

export const getTimelineAvailableDates = () => invoke<string[]>("get_timeline_available_dates");

export const getSystemSamples = (startMs: number, endMs: number, maxPoints = 2500) =>
  invoke<SystemSample[]>("get_system_samples", { startMs, endMs, maxPoints });

export const getSystemTimeline = (startMs: number, endMs: number, maxPoints = 2500) =>
  invoke<TimelineQueryResult>("get_system_timeline", { startMs, endMs, maxPoints });

export const getMetricCatalog = () => invoke<MetricCatalogSnapshot>("get_metric_catalog");

export const getResourceAvailableDates = () => invoke<string[]>("get_resource_available_dates");

export const getAppResourceSamples = (timestampMs: number) =>
  invoke<AppResourceSample[]>("get_app_resource_samples", { timestampMs });

export const getResourceApps = () => invoke<ResourceApp[]>("get_resource_apps");

export const getAppResourceAvailableDates = (appKey: string) =>
  invoke<string[]>("get_app_resource_available_dates", { appKey });

export const getAppResourceHistory = (appKey: string, startMs: number, endMs: number, maxPoints = 2500) =>
  invoke<AppResourceHistoryPoint[]>("get_app_resource_history", { appKey, startMs, endMs, maxPoints });

export const listApps = () => invoke<AppIdentity[]>("list_apps");
export const setAppHidden = (appId: number, hidden: boolean) => invoke<void>("set_app_hidden", { appId, hidden });
export const getCollectorStatus = () => invoke<CollectorStatus>("get_collector_status");
export const getStorageUsage = () => invoke<StorageUsage>("get_storage_usage");
export const setCollectionPaused = (paused: boolean) => invoke<void>("set_collection_paused", { paused });
export const getCollectionSettings = () => invoke<CollectionSettings>("get_collection_settings");
export const setCollectionSettings = (settings: CollectionSettings) => invoke<void>("set_collection_settings", { settings });
export const getDashboardConfig = () => invoke<DashboardConfig | null>("get_dashboard_config");
export const setDashboardConfig = (config: DashboardConfig) => invoke<void>("set_dashboard_config", { config });
export const getAutostartEnabled = () => invoke<boolean>("get_autostart_enabled");
export const setAutostartEnabled = (enabled: boolean) => invoke<void>("set_autostart_enabled", { enabled });
export const clearCollectedData = () => invoke<void>("clear_collected_data");

export const getCrashDetectorStatus = () => invoke<CrashDetectorStatus>("get_crash_detector_status");
export const listCrashCases = () => invoke<CrashCaseSummary[]>("list_crash_cases");
export const getCrashCaseDetail = (caseId: number) => invoke<CrashEvidenceDetail>("get_crash_case_detail", { caseId });
