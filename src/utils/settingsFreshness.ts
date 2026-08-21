export type SettingsFreshnessState = {
  fullGeneration: number;
  statusStorageGeneration: number;
};

export type SettingsFullRefreshRequest = {
  kind: "full";
  fullGeneration: number;
  statusStorageGeneration: number;
};

export type SettingsStatusPollRequest = {
  kind: "statusPoll";
  statusStorageGeneration: number;
};

export type SettingsRefreshRequest = SettingsFullRefreshRequest | SettingsStatusPollRequest;

export function beginSettingsFullRefresh(current: SettingsFreshnessState): { state: SettingsFreshnessState; request: SettingsFullRefreshRequest } {
  const state = {
    fullGeneration: current.fullGeneration + 1,
    statusStorageGeneration: current.statusStorageGeneration + 1
  };
  return { state, request: { kind: "full", ...state } };
}

export function beginSettingsStatusPoll(current: SettingsFreshnessState): { state: SettingsFreshnessState; request: SettingsStatusPollRequest } {
  const state = {
    ...current,
    statusStorageGeneration: current.statusStorageGeneration + 1
  };
  return { state, request: { kind: "statusPoll", statusStorageGeneration: state.statusStorageGeneration } };
}

export function invalidateSettingsRefreshes(current: SettingsFreshnessState): SettingsFreshnessState {
  return {
    fullGeneration: current.fullGeneration + 1,
    statusStorageGeneration: current.statusStorageGeneration + 1
  };
}

export function canCommitSettingsFullOnly(
  current: SettingsFreshnessState,
  request: SettingsRefreshRequest,
  mounted: boolean
): boolean {
  return mounted && request.kind === "full" && request.fullGeneration === current.fullGeneration;
}

export function canCommitSettingsStatusStorage(
  current: SettingsFreshnessState,
  request: SettingsRefreshRequest,
  mounted: boolean
): boolean {
  return mounted && request.statusStorageGeneration === current.statusStorageGeneration;
}
