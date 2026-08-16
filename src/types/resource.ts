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
}

export interface CollectionSettings {
  foregroundPollIntervalMs: number;
  systemSampleIntervalMs: number;
  idleThresholdSeconds: number;
  systemSampleRetentionDays: number;
}
