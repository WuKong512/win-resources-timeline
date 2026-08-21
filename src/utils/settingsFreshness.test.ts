import { describe, expect, it } from "vitest";
import {
  beginSettingsFullRefresh,
  beginSettingsStatusPoll,
  canCommitSettingsFullOnly,
  canCommitSettingsStatusStorage,
  invalidateSettingsRefreshes,
  type SettingsFreshnessState
} from "./settingsFreshness";

const initialState: SettingsFreshnessState = { fullGeneration: 0, statusStorageGeneration: 0 };

describe("settings refresh freshness", () => {
  it("keeps full-only data when a newer poll wins status and storage", () => {
    const full = beginSettingsFullRefresh(initialState);
    const poll = beginSettingsStatusPoll(full.state);

    expect(canCommitSettingsStatusStorage(poll.state, poll.request, true)).toBe(true);
    expect(canCommitSettingsFullOnly(poll.state, full.request, true)).toBe(true);
    expect(canCommitSettingsStatusStorage(poll.state, full.request, true)).toBe(false);
  });

  it("rejects an older poll after a newer poll resolves", () => {
    const pollA = beginSettingsStatusPoll(initialState);
    const pollB = beginSettingsStatusPoll(pollA.state);

    expect(canCommitSettingsStatusStorage(pollB.state, pollB.request, true)).toBe(true);
    expect(canCommitSettingsStatusStorage(pollB.state, pollA.request, true)).toBe(false);
  });

  it("rejects all fields from an older full refresh", () => {
    const fullA = beginSettingsFullRefresh(initialState);
    const fullB = beginSettingsFullRefresh(fullA.state);

    expect(canCommitSettingsFullOnly(fullB.state, fullB.request, true)).toBe(true);
    expect(canCommitSettingsStatusStorage(fullB.state, fullB.request, true)).toBe(true);
    expect(canCommitSettingsFullOnly(fullB.state, fullA.request, true)).toBe(false);
    expect(canCommitSettingsStatusStorage(fullB.state, fullA.request, true)).toBe(false);
  });

  it("rejects full and poll completions after unmount", () => {
    const full = beginSettingsFullRefresh(initialState);
    const poll = beginSettingsStatusPoll(full.state);
    const invalidated = invalidateSettingsRefreshes(poll.state);

    expect(canCommitSettingsFullOnly(poll.state, full.request, false)).toBe(false);
    expect(canCommitSettingsStatusStorage(poll.state, poll.request, false)).toBe(false);
    expect(canCommitSettingsFullOnly(invalidated, full.request, true)).toBe(false);
    expect(canCommitSettingsStatusStorage(invalidated, poll.request, true)).toBe(false);
  });
});
