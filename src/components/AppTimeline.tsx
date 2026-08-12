import { useState } from "react";
import type { ForegroundInterval } from "../types/resource";
import { useI18n } from "../i18n";
import { formatClock, formatDuration, localDayRange, timelinePercent } from "../utils/time";

const palette = ["#2aa99a", "#4f83d1", "#8d79dc", "#e07b67", "#7f9b54", "#48a3b7", "#d49a4f"];
type Row = { appId: number; label: string; activeMs: number; intervals: ForegroundInterval[] };

export function AppTimeline({ intervals, date }: { intervals: ForegroundInterval[]; date: string }) {
  const { language, t } = useI18n();
  const [focusedInterval, setFocusedInterval] = useState<string | null>(null);
  const [focusedByApp, setFocusedByApp] = useState<Record<number, number>>({});
  const { startMs, endMs } = localDayRange(date);
  const rows = Array.from(intervals.reduce((map, item) => {
    const row = map.get(item.appId) ?? { appId: item.appId, label: item.displayName || item.appName, activeMs: 0, intervals: [] };
    row.intervals.push(item);
    if (item.activityState === "active") row.activeMs += item.durationMs;
    map.set(item.appId, row);
    return map;
  }, new Map<number, Row>()).values()).sort((a, b) => b.activeMs - a.activeMs);

  if (!rows.length) return <div className="rounded-lg border border-border bg-card px-6 py-16 text-center text-sm text-muted-foreground">{t("noTimelineData")}</div>;
  return <div className="surface-shadow overflow-hidden rounded-lg border border-border/80 bg-card">
    <div className="flex min-h-9 items-center justify-between gap-3 border-b border-border bg-muted/25 px-4 py-2 text-[11px] text-muted-foreground">
      <span>{t("timelineAppCount", { count: rows.length })}</span>
      <span className="min-w-0 truncate text-right tabular-nums">{focusedInterval ?? t("timelineSegmentHint")}</span>
    </div>
    <div className="grid grid-cols-[210px_1fr] border-b border-border bg-muted/30 text-[11px] font-medium text-muted-foreground">
      <div className="px-4 py-3">{t("app")}</div>
      <div className="grid grid-cols-8 px-3 py-3 tabular-nums">{["00", "03", "06", "09", "12", "15", "18", "21"].map((h) => <span key={h}>{h}:00</span>)}</div>
    </div>
    <div className="max-h-[640px] overflow-auto">{rows.map((row, rowIndex) => {
      const focusedIndex = Math.min(focusedByApp[row.appId] ?? 0, Math.max(0, row.intervals.length - 1));
      return <div key={row.appId} className="grid min-h-[62px] grid-cols-[210px_1fr] border-b border-border/70 transition-colors last:border-b-0 hover:bg-muted/20">
      <div className="min-w-0 px-4 py-3"><div className="truncate text-sm font-medium" title={row.label}>{row.label}</div><div className="mt-1 text-[11px] tabular-nums text-muted-foreground">{formatDuration(row.activeMs / 1000, language)}</div></div>
      <div className="timeline-grid relative mx-3 my-3.5 rounded-md bg-stone-50/90 ring-1 ring-inset ring-border/30">{row.intervals.map((item, itemIndex) => {
        const left = timelinePercent(item.startTimeMs, startMs, endMs);
        const right = timelinePercent(item.endTimeMs, startMs, endMs);
        const width = Math.min(100 - left, right - left);
        const title = `${row.label} | ${formatClock(item.startTimeMs, language)} - ${formatClock(item.endTimeMs, language)} | ${formatDuration(item.durationMs / 1000, language)} | ${item.activityState === "idle" ? t("activityIdle") : t("activityActive")}`;
        return <div key={item.id} title={title} role="img" tabIndex={itemIndex === focusedIndex ? 0 : -1} aria-label={title}
          onMouseEnter={() => setFocusedInterval(title)} onMouseLeave={() => setFocusedInterval(null)}
          onFocus={() => { setFocusedByApp((current) => ({ ...current, [row.appId]: itemIndex })); setFocusedInterval(title); }} onBlur={() => setFocusedInterval(null)}
          onKeyDown={(event) => {
            if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
            event.preventDefault();
            const nextIndex = Math.min(row.intervals.length - 1, Math.max(0, itemIndex + (event.key === "ArrowRight" ? 1 : -1)));
            setFocusedByApp((current) => ({ ...current, [row.appId]: nextIndex }));
            (event.currentTarget.parentElement?.children[nextIndex] as HTMLElement | undefined)?.focus();
          }}
          className={`absolute top-2 h-4 rounded-[3px] shadow-[0_2px_5px_rgba(15,23,42,0.18)] outline-none transition-[height,box-shadow,filter] duration-150 hover:h-5 hover:brightness-110 focus-visible:h-5 focus-visible:ring-2 focus-visible:ring-ring/60 ${item.activityState === "idle" ? "timeline-idle" : ""}`}
          style={{ left: `${left}%`, width: `${Math.max(width, 0.12)}%`, backgroundColor: palette[rowIndex % palette.length] }} />;
      })}</div>
    </div>})}</div>
  </div>;
}
