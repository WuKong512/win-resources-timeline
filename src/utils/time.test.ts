import { describe, expect, it } from "vitest";
import { clipInterval, formatBytes, formatDuration, localDayRange, timelinePercent } from "./time";

describe("time and formatting helpers", () => {
  it("creates a local half-open day range", () => {
    const range = localDayRange("2026-07-12");
    expect(new Date(range.startMs).getDate()).toBe(12);
    expect(new Date(range.endMs).getDate()).toBe(13);
    expect(range.endMs).toBeGreaterThan(range.startMs);
  });

  it("formats duration and byte rates", () => {
    expect(formatDuration(3661)).toBe("1h 1m");
    expect(formatBytes(1536)).toBe("1.5 KB");
    expect(formatBytes(null)).toBe("No sample");
  });

  it("clips intervals to a half-open range", () => {
    expect(clipInterval(500, 2_000, 1_000, 3_000)).toEqual({ startMs: 1_000, endMs: 2_000 });
    expect(clipInterval(0, 1_000, 1_000, 2_000)).toBeNull();
  });

  it("maps timestamps to bounded timeline percentages", () => {
    expect(timelinePercent(1_500, 1_000, 2_000)).toBe(50);
    expect(timelinePercent(500, 1_000, 2_000)).toBe(0);
    expect(timelinePercent(2_500, 1_000, 2_000)).toBe(100);
  });
});
