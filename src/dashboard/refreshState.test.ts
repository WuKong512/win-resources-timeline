import { describe, expect, it } from "vitest";
import { shouldClearTimelineForLoad, shouldShowTimelineRefreshNotice } from "./refreshState";

describe("timeline refresh state", () => {
  it("keeps a successful background refresh silent", () => {
    expect(shouldShowTimelineRefreshNotice({ refreshing: true, refreshError: "" })).toBe(false);
    expect(shouldShowTimelineRefreshNotice({ refreshing: false, refreshError: "" })).toBe(false);
  });

  it("shows a retryable notice when a background refresh fails", () => {
    expect(shouldShowTimelineRefreshNotice({ refreshing: true, refreshError: "The latest refresh failed." })).toBe(true);
  });

  it("retains the existing timeline during a background refresh", () => {
    expect(shouldClearTimelineForLoad(true)).toBe(false);
    expect(shouldClearTimelineForLoad(false)).toBe(true);
  });
});
