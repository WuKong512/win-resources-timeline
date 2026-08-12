import { useEffect, useState } from "react";
import { getAppUsageTimeline, getCollectionSettings, getTimelineAvailableDates } from "../api/tauriApi";
import { AppTimeline } from "../components/AppTimeline";
import { DateRangePicker } from "../components/DateRangePicker";
import { Switch } from "../components/ui/Switch";
import { useI18n } from "../i18n";
import { useUiStore } from "../stores/uiStore";
import type { ForegroundInterval } from "../types/resource";
import { formatDuration, localDateString, localDayRange } from "../utils/time";

export function TimelinePage() {
  const { language, t } = useI18n();
  const selectedDate = useUiStore((s) => s.selectedDate); const setSelectedDate = useUiStore((s) => s.setSelectedDate);
  const showHidden = useUiStore((s) => s.showHiddenApps); const setShowHidden = useUiStore((s) => s.setShowHiddenApps);
  const [showIdle, setShowIdle] = useState(true); const [intervals, setIntervals] = useState<ForegroundInterval[]>([]); const [error, setError] = useState(""); const [loading, setLoading] = useState(true);
  const [availableDates, setAvailableDates] = useState<string[]>([]); const [datesLoading, setDatesLoading] = useState(true);
  const [idleThresholdSeconds, setIdleThresholdSeconds] = useState(300);
  useEffect(() => {
    let cancelled = false; setDatesLoading(true);
    getTimelineAvailableDates().then((dates) => {
      if (cancelled) return;
      setAvailableDates(dates);
      if (dates.length && !dates.includes(selectedDate)) setSelectedDate(dates[dates.length - 1]);
    }).catch((e) => setError(String(e))).finally(() => { if (!cancelled) setDatesLoading(false); });
    return () => { cancelled = true; };
  }, [selectedDate, setSelectedDate]);
  useEffect(() => {
    let cancelled = false; const range = localDayRange(selectedDate); setLoading(true); setError("");
    const load = () => getAppUsageTimeline(range.startMs, range.endMs, showHidden, true).then((data) => { if (!cancelled) setIntervals(data); }).catch((e) => { if (!cancelled) setError(String(e)); }).finally(() => { if (!cancelled) setLoading(false); });
    void load();
    const timer = selectedDate === localDateString() ? window.setInterval(load, 5_000) : undefined;
    return () => { cancelled = true; if (timer != null) window.clearInterval(timer); };
  }, [selectedDate, showHidden]);
  useEffect(() => { getCollectionSettings().then((settings) => setIdleThresholdSeconds(settings.idleThresholdSeconds)).catch(() => undefined); }, []);
  const idleIntervals = intervals.filter((interval) => interval.activityState === "idle");
  const idleSeconds = idleIntervals.reduce((total, interval) => total + interval.durationMs, 0) / 1000;
  const visibleIntervals = showIdle ? intervals : intervals.filter((interval) => interval.activityState !== "idle");
  return <div className="space-y-5">
    <div className="flex flex-wrap items-start justify-between gap-4">
      <div><h1 className="page-title text-[26px] font-semibold">{t("timelineTitle")}</h1><p className="mt-1 text-sm text-muted-foreground">{t("timelineSubtitle")}</p></div>
      <DateRangePicker value={selectedDate} onChange={setSelectedDate} availableDates={availableDates} loading={datesLoading} />
    </div>
    <div className="surface-shadow flex flex-wrap items-center gap-5 rounded-lg border border-border/80 bg-card px-4 py-3 text-sm">
      <label className="flex items-center gap-2.5" title={idleIntervals.length ? t("idleRecorded", { duration: formatDuration(idleSeconds, language) }) : t("noIdleForDate", { threshold: formatDuration(idleThresholdSeconds, language) })}>
        <Switch checked={showIdle} onCheckedChange={setShowIdle} ariaLabel={t("showIdle")} />
        <span>{t("showIdle")}</span>
        <span className="rounded-full border border-border bg-muted/70 px-2 py-0.5 text-[11px] tabular-nums text-muted-foreground">{formatDuration(idleSeconds, language)}</span>
      </label>
      <span className="hidden h-5 w-px bg-border sm:block" />
      <label className="flex items-center gap-2.5">
        <Switch checked={showHidden} onCheckedChange={setShowHidden} ariaLabel={t("showHidden")} />
        <span>{t("showHidden")}</span>
      </label>
    </div>
    {error ? <div className="error-surface rounded-lg border p-4 text-sm">{error}</div> : loading ? <div className="surface-shadow rounded-lg border border-border bg-card px-5 py-10 text-sm text-muted-foreground">{t("loadingTimeline")}</div> : <AppTimeline intervals={visibleIntervals} date={selectedDate} />}
  </div>;
}
