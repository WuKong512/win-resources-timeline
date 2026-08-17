export interface AppIdentity {
  id: number;
  processName: string;
  exePath: string | null;
  displayName: string;
  publisher: string | null;
  isHidden: boolean;
  firstSeenAtMs: number;
  lastSeenAtMs: number;
}

export type ActivityState = "active" | "idle";

export interface ForegroundInterval {
  id: number;
  appId: number;
  appName: string;
  displayName: string;
  startTimeMs: number;
  endTimeMs: number;
  durationMs: number;
  activityState: ActivityState;
  isHidden: boolean;
}

export interface AppUsageSummary {
  appId: number;
  appName: string;
  displayName: string;
  foregroundTotalMs: number;
  activeUsageMs: number;
  idleForegroundMs: number;
  activeSeconds: number;
  idleSeconds: number;
  percentage: number;
  isHidden: boolean;
}

export interface DailyUsageSummary {
  localDate: string;
  appId: number;
  appName: string;
  displayName: string;
  foregroundTotalMs: number;
  activeUsageMs: number;
  idleForegroundMs: number;
  launchCount: number;
  processingVersion: string;
  isHidden: boolean;
}

export interface SystemSample {
  timestampMs: number;
  sampleDurationMs: number;
  cpuPercent: number | null;
  memoryPercent: number | null;
  memoryUsedBytes: number | null;
  memoryTotalBytes: number | null;
  diskReadBytesPerSec: number | null;
  diskWriteBytesPerSec: number | null;
  hasAppSnapshot: boolean;
}

export interface AppResourceSample {
  appKey: string;
  processName: string;
  exePath: string | null;
  processCount: number;
  cpuPercent: number;
  memoryUsedBytes: number;
  ioReadBytesPerSec: number;
  ioWriteBytesPerSec: number;
}

export interface ResourceApp {
  appKey: string;
  processName: string;
  displayName: string;
  exePath: string | null;
  lastSampleAtMs: number;
}

export interface AppResourceHistoryPoint {
  timestampMs: number;
  sampleDurationMs: number;
  cpuPercent: number | null;
  memoryUsedBytes: number | null;
  ioReadBytesPerSec: number | null;
  ioWriteBytesPerSec: number | null;
}

export interface TodayOverview {
  startMs: number;
  endMs: number;
  totalActiveForegroundSeconds: number;
  totalIdleForegroundSeconds: number;
  computerActiveSeconds: number;
  hiddenActiveForegroundSeconds: number;
  topApps: AppUsageSummary[];
  cpuSampledPeak: number | null;
  memorySampledPeak: number | null;
  diskReadSampledPeak: number | null;
  diskWriteSampledPeak: number | null;
}

export type MetricCategory = "cpu" | "gpu" | "memory" | "disk" | "network" | "power" | "battery" | "process";
export type CapabilitySupportStatus = "supported" | "unsupported";
export type CapabilityState = "supportedEnabled" | "supportedDisabled" | "unsupported" | "failed";
export type ProviderLifecycleState = "stopped" | "running" | "paused" | "failed";
export type ProviderErrorCode =
  | "providerMissing"
  | "permissionDenied"
  | "startupFailed"
  | "reconfigureFailed"
  | "sampleFailed"
  | "stopFailed"
  | "timeout"
  | "unsupported"
  | "userDisabled"
  | "categoryDisabled"
  | "paused";

export interface ProviderErrorSummary {
  code: ProviderErrorCode;
  message: string | null;
}

export interface MetricCapabilityStatus {
  providerId: string;
  category: MetricCategory;
  supportStatus: CapabilitySupportStatus;
  enabled: boolean;
  canToggle: boolean;
  state: CapabilityState;
  reasonCode: ProviderErrorCode | null;
}

export interface ProviderStatus {
  providerId: string;
  displayName: string;
  supported: boolean;
  enabled: boolean;
  lifecycle: ProviderLifecycleState;
  capabilities: MetricCapabilityStatus[];
  lastSuccessAtMs: number | null;
  failureCount: number;
  lastError: ProviderErrorSummary | null;
}

export interface CollectorStatus {
  running: boolean;
  paused: boolean;
  startedAtMs: number | null;
  lastHeartbeatAtMs: number | null;
  lastForegroundSampleAtMs: number | null;
  lastSystemSampleAtMs: number | null;
  droppedSystemSamples: number;
  databaseSizeBytes: number;
  databasePath: string;
  providerStatus: ProviderStatus[];
}

export interface CollectionSettings {
  foregroundPollIntervalMs: number;
  systemSampleIntervalMs: number;
  idleThresholdSeconds: number;
  systemSampleRetentionDays: number;
  enabledCategories: MetricCategory[];
  disabledProviders: string[];
}
