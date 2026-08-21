import { useCallback, useEffect, useMemo, useState } from "react";
import { Activity, CircleHelp, Clock3, LockKeyhole, Moon, WifiOff } from "lucide-react";
import { getDailyUsageSummary, getTimelineAvailableDates, getUsageSummary } from "../api/tauriApi";
import { DateRangePicker } from "../components/DateRangePicker";
import { Badge } from "../components/ui/Badge";
import { Button } from "../components/ui/Button";
import { Card, CardContent, CardHeader, CardTitle } from "../components/ui/Card";
import { Switch } from "../components/ui/Switch";
import { useI18n } from "../i18n";
import { useUiStore } from "../stores/uiStore";
import type { AppUsageSummary, ComputerStateInterval, DailyUsageSummary, UsageSummary } from "../types/resource";
import { formatDuration, localDayRange, shiftLocalDate, timelinePercent } from "../utils/time";
import { stateDurations } from "../utils/uiSemantics";

type UsagePreset = 1 | 7 | 30;

const stateOrder = ["active", "idle", "locked", "sleep", "disconnected", "unknown"] as const;

export function UsagePage() {
  const { language, t } = useI18n();
  const selectedDate = useUiStore((state) => state.selectedDate);
  const setSelectedDate = useUiStore((state) => state.setSelectedDate);
  const showHidden = useUiStore((state) => state.showHiddenApps);
  const setShowHidden = useUiStore((state) => state.setShowHiddenApps);
  const [preset, setPreset] = useState<UsagePreset>(1);
  const [availableDates, setAvailableDates] = useState<string[]>([]);
  const [datesLoading, setDatesLoading] = useState(true);
  const [summary, setSummary] = useState<UsageSummary | null>(null);
  const [dailyApps, setDailyApps] = useState<DailyUsageSummary[] | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const range = useMemo(() => usageRange(selectedDate, preset), [preset, selectedDate]);

  const loadUsage = useCallback(() => {
    let cancelled = false;
    setLoading(true);
    setError("");
    const daily = preset === 1 ? getDailyUsageSummary(selectedDate, showHidden) : Promise.resolve(null);
    Promise.all([getUsageSummary(range.startMs, range.endMs, showHidden), daily])
      .then(([nextSummary, nextDaily]) => {
        if (cancelled) return;
        setSummary(nextSummary);
        setDailyApps(nextDaily);
      })
      .catch(() => { if (!cancelled) setError(t("usageErrorMessage")); })
      .finally(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, [preset, range.endMs, range.startMs, selectedDate, showHidden, t]);

  useEffect(() => {
    let cancelled = false;
    setDatesLoading(true);
    getTimelineAvailableDates()
      .then((dates) => {
        if (cancelled) return;
        setAvailableDates(dates);
        if (dates.length && !dates.includes(selectedDate)) setSelectedDate(dates[dates.length - 1]);
      })
      .catch(() => { if (!cancelled) setError(t("usageErrorMessage")); })
      .finally(() => { if (!cancelled) setDatesLoading(false); });
    return () => { cancelled = true; };
  }, [selectedDate, setSelectedDate, t]);

  useEffect(() => loadUsage(), [loadUsage]);

  const appRows = useMemo(() => dailyApps ? dailyApps.map((app) => ({
    appId: app.appId,
    displayName: app.displayName || app.appName,
    foregroundTotalMs: app.foregroundTotalMs,
    activeUsageMs: app.activeUsageMs,
    idleForegroundMs: app.idleForegroundMs,
    percentage: 0,
    launchCount: app.launchCount,
    processingVersion: app.processingVersion
  })) : (summary?.apps ?? []).map((app) => appRow(app)), [dailyApps, summary?.apps]);
  const totalActive = appRows.reduce((total, app) => total + app.activeUsageMs, 0);
  const appRowsWithShare = appRows.map((app) => ({ ...app, percentage: totalActive ? app.activeUsageMs * 100 / totalActive : 0 }));
  const durations = stateDurations(summary?.stateIntervals ?? []);

  return <div className="space-y-5">
    <header className="flex flex-wrap items-end justify-between gap-4">
      <div><div className="eyebrow">{t("usagePeriod")}</div><h1 className="page-title mt-1 text-[28px] font-semibold tracking-[-0.02em]">{t("usageTitle")}</h1><p className="mt-1 max-w-2xl text-sm text-muted-foreground">{t("usageSubtitle")}</p></div>
      <div className="flex flex-wrap items-center gap-2"><div className="segmented-control" aria-label={t("usagePeriod")}>{([1, 7, 30] as UsagePreset[]).map((days) => <button key={days} type="button" className={preset === days ? "segmented-control-active" : "segmented-control-item"} onClick={() => setPreset(days)}>{t(days === 1 ? "usageDay" : days === 7 ? "usage7Days" : "usage30Days")}</button>)}</div><DateRangePicker value={selectedDate} onChange={setSelectedDate} availableDates={availableDates} loading={datesLoading} /></div>
    </header>

    {error ? <InlineError title={t("usageError")} message={error} onRetry={loadUsage} /> : loading ? <UsageLoading /> : !summary ? <EmptyUsage /> : <>
      <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4"><UsageMetric icon={<Activity size={16} />} label={t("computerActive")} value={formatDuration(summary.computerActiveSeconds, language)} /><UsageMetric icon={<Clock3 size={16} />} label={t("activeUsage")} value={formatDuration(Math.round(totalActive / 1000), language)} /><UsageMetric icon={<LockKeyhole size={16} />} label={t("stateLocked")} value={formatDuration(Math.round((durations.locked ?? 0) / 1000), language)} /><UsageMetric icon={<CircleHelp size={16} />} label={t("usageCoverage")} value={`${Math.round(summary.coverage * 100)}%`} /> </div>

      <Card className="overflow-hidden"><CardHeader className="border-b border-border/70"><div className="flex flex-wrap items-start justify-between gap-3"><div><CardTitle>{t("computerState")}</CardTitle><p className="mt-1 text-xs font-normal text-muted-foreground">{t("usageStateHint")}</p></div><Badge className={summary.coverage >= 0.999 ? "border-[hsl(var(--success)/0.3)] bg-[hsl(var(--success-surface))] text-[hsl(var(--success))]" : "border-[hsl(var(--warning)/0.35)] bg-[hsl(var(--warning-surface))] text-[hsl(var(--warning))]"}>{summary.coverage >= 0.999 ? t("coverageComplete") : t("coverageIncomplete")}</Badge></div></CardHeader><CardContent className="pt-4"><StateBand intervals={summary.stateIntervals} startMs={summary.startMs} endMs={summary.endMs} /><div className="mt-4 grid gap-2 sm:grid-cols-2 lg:grid-cols-3">{stateOrder.map((state) => <StateDuration key={state} state={state} durationMs={durations[state] ?? 0} present={Boolean(durations[state])} />)}</div></CardContent></Card>

      <Card className="overflow-hidden"><CardHeader className="border-b border-border/70"><div className="flex flex-wrap items-center justify-between gap-3"><div><CardTitle>{t("appUsage")}</CardTitle><p className="mt-1 text-xs font-normal text-muted-foreground">{dailyApps ? t("dailyDataSource") : t("periodDataSource")}</p></div><label className="flex items-center gap-2 text-xs text-muted-foreground"><Switch checked={showHidden} onCheckedChange={setShowHidden} ariaLabel={t("showHidden")} />{t("showHidden")}</label></div></CardHeader><CardContent className="px-0 pb-0">{appRowsWithShare.length ? <div className="overflow-x-auto"><table className="w-full min-w-[720px] border-collapse text-sm"><caption className="sr-only">{t("appUsage")}</caption><thead><tr><th scope="col" className="table-head pl-5">{t("app")}</th><th scope="col" className="table-head text-right">{t("activeUsage")}</th><th scope="col" className="table-head text-right">{t("foregroundTotal")}</th><th scope="col" className="table-head text-right">{t("idleForeground")}</th><th scope="col" className="table-head text-right">{t("launchCount")}</th><th scope="col" className="table-head pr-5 text-right">{t("share")}</th></tr></thead><tbody>{appRowsWithShare.map((app) => <tr key={app.appId} className="border-b border-border/60 last:border-b-0 hover:bg-muted/20"><td className="table-cell pl-5"><div className="font-medium">{app.displayName}</div><div className="mt-2 h-1.5 max-w-xs overflow-hidden rounded-full bg-muted"><div className="h-full rounded-full bg-[hsl(var(--signal-blue))]" style={{ width: `${Math.min(100, Math.max(app.percentage, app.activeUsageMs ? 1 : 0))}%` }} /></div></td><td className="table-cell text-right font-mono">{formatDuration(Math.round(app.activeUsageMs / 1000), language)}</td><td className="table-cell text-right font-mono text-muted-foreground">{formatDuration(Math.round(app.foregroundTotalMs / 1000), language)}</td><td className="table-cell text-right font-mono text-muted-foreground">{formatDuration(Math.round(app.idleForegroundMs / 1000), language)}</td><td className="table-cell text-right font-mono text-muted-foreground">{app.launchCount == null ? "—" : app.launchCount}</td><td className="table-cell pr-5 text-right font-mono text-muted-foreground">{app.percentage.toFixed(1)}%</td></tr>)}</tbody></table></div> : <div className="px-5 py-12 text-sm text-muted-foreground">{t("noUsageApps")}</div>}</CardContent></Card>
    </>}
  </div>;
}

function usageRange(date: string, preset: UsagePreset) {
  const endMs = localDayRange(date).endMs;
  return { startMs: localDayRange(shiftLocalDate(date, -(preset - 1))).startMs, endMs };
}

function appRow(app: AppUsageSummary) {
  return { appId: app.appId, displayName: app.displayName || app.appName, foregroundTotalMs: app.foregroundTotalMs, activeUsageMs: app.activeUsageMs, idleForegroundMs: app.idleForegroundMs, percentage: app.percentage, launchCount: null as number | null, processingVersion: null as string | null };
}

function StateBand({ intervals, startMs, endMs }: { intervals: ComputerStateInterval[]; startMs: number; endMs: number }) {
  const { language, t } = useI18n();
  return <div className="usage-state-band" role="img" aria-label={t("computerState")}><div className="usage-state-grid" />{intervals.map((interval, index) => { const left = timelinePercent(interval.startTimeMs, startMs, endMs); const right = timelinePercent(interval.endTimeMs, startMs, endMs); return <div key={`${interval.state}-${interval.startTimeMs}-${index}`} title={`${stateLabel(interval.state, t)} · ${formatDuration(interval.durationMs / 1000, language)}`} className={`usage-state-segment usage-state-${interval.state}`} style={{ left: `${left}%`, width: `${Math.max(right - left, 0.18)}%` }} />; })}</div>;
}

function StateDuration({ state, durationMs, present }: { state: string; durationMs: number; present: boolean }) {
  const { language, t } = useI18n();
  const Icon = state === "active" ? Activity : state === "idle" ? Clock3 : state === "locked" ? LockKeyhole : state === "sleep" ? Moon : state === "disconnected" ? WifiOff : CircleHelp;
  return <div className={`flex items-center justify-between gap-3 rounded-lg border px-3 py-2.5 ${present ? "border-border bg-muted/15" : "border-dashed border-border/70 bg-transparent"}`}><div className="flex items-center gap-2.5"><Icon size={15} className="text-muted-foreground" /><span className="text-xs font-medium">{stateLabel(state, t)}</span></div><span className="font-mono text-xs text-muted-foreground">{formatDuration(Math.round(durationMs / 1000), language)}</span></div>;
}

function stateLabel(state: string, t: ReturnType<typeof useI18n>["t"]) {
  if (state === "active") return t("stateActive");
  if (state === "idle") return t("stateIdle");
  if (state === "locked") return t("stateLocked");
  if (state === "sleep") return t("stateSleep");
  if (state === "disconnected") return t("stateDisconnected");
  if (state === "unknown") return t("stateUnknown");
  return t("stateUnobserved");
}

function UsageMetric({ icon, label, value }: { icon: React.ReactNode; label: string; value: string }) {
  return <Card className="relative overflow-hidden"><div className="absolute inset-x-0 top-0 h-0.5 bg-[hsl(var(--signal-blue))]" /><CardContent className="pt-4"><div className="flex items-center justify-between gap-3"><span className="text-xs font-medium text-muted-foreground">{label}</span><span className="flex h-7 w-7 items-center justify-center rounded-lg bg-muted text-[hsl(var(--signal-blue))]">{icon}</span></div><div className="metric-value mt-3 truncate text-[21px] font-semibold">{value}</div></CardContent></Card>;
}

function InlineError({ title, message, onRetry }: { title: string; message: string; onRetry: () => void }) {
  const { t } = useI18n();
  return <div role="alert" className="error-surface flex flex-wrap items-center justify-between gap-3 rounded-lg border px-4 py-3 text-sm"><div><div className="font-semibold">{title}</div><div className="mt-1 break-all text-xs opacity-80">{message}</div></div><Button variant="outline" onClick={onRetry}>{t("retry")}</Button></div>;
}

function UsageLoading() {
  const { t } = useI18n();
  return <div className="space-y-3" aria-busy="true" aria-label={t("usageLoading")}><div className="skeleton-line h-24 rounded-lg" /><div className="skeleton-line h-56 rounded-lg" /><div className="skeleton-line h-72 rounded-lg" /></div>;
}

function EmptyUsage() {
  const { t } = useI18n();
  return <div className="empty-state"><CircleHelp size={22} className="text-muted-foreground" /><div className="font-semibold text-foreground">{t("usageEmpty")}</div><div className="max-w-md text-sm text-muted-foreground">{t("usageEmptyHint")}</div></div>;
}
