import { describe, expect, it } from "vitest";
import { clipInterval, effectiveTimelineDate, formatBytes, formatDuration, localDayRange, millisecondsUntilLocalMidnight, shiftLocalDate, timelinePercent, timelineWindowRange } from "./time";

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

  it("shifts dates by local calendar days", () => {
    expect(shiftLocalDate("2026-08-21", -1)).toBe("2026-08-20");
    expect(shiftLocalDate("2026-08-21", 9)).toBe("2026-08-30");
  });

  it("maps timestamps to bounded timeline percentages", () => {
    expect(timelinePercent(1_500, 1_000, 2_000)).toBe(50);
    expect(timelinePercent(500, 1_000, 2_000)).toBe(0);
    expect(timelinePercent(2_500, 1_000, 2_000)).toBe(100);
  });

  it("ends a current one-day window at the supplied current time", () => {
    const selectedDate = "2026-08-21";
    const calendar = localDayRange(selectedDate);
    const nowMs = calendar.startMs + 12 * 60 * 60 * 1_000;
    expect(timelineWindowRange(selectedDate, 1, nowMs)).toEqual({
      startMs: calendar.startMs,
      endMs: nowMs
    });
  });

  it("ends a current seven-day window at now without shortening its start", () => {
    const selectedDate = "2026-08-21";
    const nowMs = localDayRange(selectedDate).startMs + 12 * 60 * 60 * 1_000;
    expect(timelineWindowRange(selectedDate, 7, nowMs)).toEqual({
      startMs: localDayRange(shiftLocalDate(selectedDate, -6)).startMs,
      endMs: nowMs
    });
  });

  it("ends a current thirty-day window at now", () => {
    const selectedDate = "2026-08-21";
    const nowMs = localDayRange(selectedDate).startMs + 12 * 60 * 60 * 1_000;
    expect(timelineWindowRange(selectedDate, 30, nowMs).endMs).toBe(nowMs);
    expect(timelineWindowRange(selectedDate, 30, nowMs).startMs)
      .toBe(localDayRange(shiftLocalDate(selectedDate, -29)).startMs);
  });

  it("keeps a past day at its complete historical calendar boundary", () => {
    const selectedDate = "2026-08-20";
    const nowMs = localDayRange("2026-08-21").startMs + 12 * 60 * 60 * 1_000;
    expect(timelineWindowRange(selectedDate, 1, nowMs)).toEqual(localDayRange(selectedDate));
  });

  it("never includes future time when a calendar window extends beyond now", () => {
    const selectedDate = "2026-08-21";
    const nowMs = localDayRange(selectedDate).startMs + 12 * 60 * 60 * 1_000;
    const range = timelineWindowRange(selectedDate, 1, nowMs);
    expect(range.endMs).toBeLessThanOrEqual(nowMs);
  });

  it("rolls a live timeline date across local midnight", () => {
    const before = new Date(2026, 7, 21, 23, 59, 59).getTime();
    const after = new Date(2026, 7, 22, 0, 0, 1).getTime();
    expect(effectiveTimelineDate("2026-08-21", true, before)).toBe("2026-08-21");
    expect(effectiveTimelineDate("2026-08-21", true, after)).toBe("2026-08-22");
    expect(timelineWindowRange(effectiveTimelineDate("2026-08-21", true, after), 1, after)).toEqual({
      startMs: localDayRange("2026-08-22").startMs,
      endMs: after
    });
  });

  it("advances current seven-day and thirty-day windows by local calendar days", () => {
    const after = new Date(2026, 7, 22, 0, 0, 1).getTime();
    const currentDate = effectiveTimelineDate("2026-08-21", true, after);
    expect(timelineWindowRange(currentDate, 7, after).startMs)
      .toBe(localDayRange(shiftLocalDate("2026-08-22", -6)).startMs);
    expect(timelineWindowRange(currentDate, 30, after).startMs)
      .toBe(localDayRange(shiftLocalDate("2026-08-22", -29)).startMs);
  });

  it("keeps a deliberate historical selection fixed across current midnight", () => {
    const after = new Date(2026, 7, 22, 0, 0, 1).getTime();
    expect(effectiveTimelineDate("2026-08-20", false, after)).toBe("2026-08-20");
    expect(timelineWindowRange("2026-08-20", 1, after)).toEqual(localDayRange("2026-08-20"));
  });

  it("uses local calendar boundaries for the rollover timer", () => {
    const before = new Date(2026, 7, 21, 23, 59, 59).getTime();
    expect(millisecondsUntilLocalMidnight(before)).toBe(localDayRange("2026-08-21").endMs - before);
    expect(shiftLocalDate("2024-02-29", 1)).toBe("2024-03-01");
  });
});
