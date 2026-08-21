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

export interface ComputerStateInterval {
  state: "active" | "idle" | "locked" | "sleep" | "disconnected" | "unknown" | string;
  startTimeMs: number;
  endTimeMs: number;
  durationMs: number;
}

export interface UsageSummary {
  startMs: number;
  endMs: number;
  observedUntilMs: number | null;
  coverage: number;
  computerActiveSeconds: number;
  stateIntervals: ComputerStateInterval[];
  apps: AppUsageSummary[];
}

export interface GpuSample {
  deviceKey: string;
  vendor: string | null;
  model: string | null;
  capacityBytes: number | null;
  utilizationPercent: number | null;
  memoryControllerUtilizationPercent: number | null;
  temperatureCelsius: number | null;
  powerWatts: number | null;
  graphicsClockMhz: number | null;
  memoryClockMhz: number | null;
  vramUsedBytes: number | null;
  vramTotalBytes: number | null;
  powerScope: string | null;
  qualityMask: number;
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
  gpus: GpuSample[];
  hasAppSnapshot: boolean;
}

export interface TimelineGap {
  startMs: number;
  endMs: number;
  durationMs: number;
}

export interface TimelineQueryResult {
  startMs: number;
  endMs: number;
  observedMs: number;
  coverage: number;
  samples: SystemSample[];
  gaps: TimelineGap[];
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
  processIdentityKey: string | null;
  pid: number | null;
  processCreationTimeMs: number | null;
  privateBytes: number | null;
  cpuTimeDeltaUs: number | null;
  gpuPercent: number | null;
  vramBytes: number | null;
  networkBytesPerSec: number | null;
  selectionReason: number;
  qualityMask: number;
  measuredCpuPercent: number | null;
  measuredWorkingSetBytes: number | null;
  measuredReadBytesPerSec: number | null;
  measuredWriteBytesPerSec: number | null;
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
  usageWriteFailures: number;
  usageWriteRetries: number;
  lastUsageWriteError: string | null;
  databaseSizeBytes: number;
  databasePath: string;
  providerStatus: ProviderStatus[];
}

export interface StorageUsage {
  mainBytes: number;
  walBytes: number;
  shmBytes: number;
  totalBytes: number;
}

export type CrashClassification = "bsod" | "unexpected_shutdown" | "abnormal_restart" | "insufficient_evidence" | string;
export type CrashEvidenceWindow = "pre_1m" | "pre_5m" | "pre_30m" | "post_5m";

export interface CrashDetectorStatus {
  state: "idle" | "scanning" | "ready" | "permission_denied" | "failed" | string;
  lastSuccessfulScanAtMs: number | null;
  lastError: string | null;
}

export interface CrashCaseSummary {
  id: number;
  stableKey: string;
  anchorTimeMs: number;
  classification: CrashClassification;
  windowStartMs: number;
  windowEndMs: number;
  evidenceStatus: "pending" | "post_pending" | "partial" | "complete" | "failed" | string;
  processingVersion: string;
  hasActiveHold: boolean;
  summaryCount: number;
}

export interface CrashSystemEvent {
  id: number;
  channel: string;
  provider: string | null;
  eventId: string;
  recordId: string;
  eventTimeMs: number;
  kind: string;
  bugcheckCode: string | null;
  bootId: string | null;
  previousShutdownTimeMs: number | null;
  cleanShutdown: boolean | null;
  restartBoundary: boolean | null;
  dumpAvailable: boolean | null;
  dumpSizeBytes: number | null;
}

export interface CrashEvidenceMetric {
  metricKey: string;
  metric: string;
  window: CrashEvidenceWindow;
  deviceKey: string | null;
  processIdentityKey: string | null;
  windowStartMs: number;
  windowEndMs: number;
  avg: number | null;
  min: number | null;
  max: number | null;
  delta: number | null;
  peakTimeMs: number | null;
  sampleCount: number;
  coverage: number;
  evidenceRef: string | null;
}

export interface CrashEvidenceProcessEntry {
  window: CrashEvidenceWindow;
  processIdentityKey: string;
  appKey: string;
  processName: string;
  pid: number | null;
  processCreationTimeMs: number | null;
  cpuAvgPercent: number | null;
  cpuPeakPercent: number | null;
  cpuDeltaPercent: number | null;
  memoryPeakBytes: number | null;
  memoryDeltaBytes: number | null;
  readBytes: number;
  writeBytes: number;
  selectionReasonMask: number;
  coverage: number;
  sampleCount: number;
  cpuRank: number | null;
  memoryRank: number | null;
  ioRank: number | null;
}

export interface CrashEvidenceDetail {
  case: CrashCaseSummary;
  events: CrashSystemEvent[];
  metrics: CrashEvidenceMetric[];
  processes: CrashEvidenceProcessEntry[];
}

export interface CollectionSettings {
  foregroundPollIntervalMs: number;
  systemSampleIntervalMs: number;
  idleThresholdSeconds: number;
  systemSampleRetentionDays: number;
  enabledCategories: MetricCategory[];
  disabledProviders: string[];
}
