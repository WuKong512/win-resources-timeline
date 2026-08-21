import { useEffect, useState } from "react";
import { ArrowRight, BarChart3, CalendarDays, Cpu, HardDrive, MemoryStick, PauseCircle } from "lucide-react";
import { getCollectorStatus, getOverviewAvailableDates, getTodayOverview } from "../api/tauriApi";
import { AppUsageTable } from "../components/AppUsageTable";
import { DateRangePicker } from "../components/DateRangePicker";
import { Card, CardContent, CardHeader, CardTitle } from "../components/ui/Card";
import { useI18n } from "../i18n";
import { useUiStore } from "../stores/uiStore";
import type { CollectorStatus, TodayOverview } from "../types/resource";
import { formatBytes, formatDuration, localDateString, localDayRange } from "../utils/time";

export function TodayPage() {
  const { language, t } = useI18n();
  const selectedDate = useUiStore((s) => s.selectedDate);
  const setSelectedDate = useUiStore((s) => s.setSelectedDate);
  const [availableDates, setAvailableDates] = useState<string[]>([]);
  const [datesLoading, setDatesLoading] = useState(true);
  const setPage = useUiStore((s) => s.setPage);
  const [overview, setOverview] = useState<TodayOverview | null>(null);
  const [status, setStatus] = useState<CollectorStatus | null>(null);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    setDatesLoading(true);
    getOverviewAvailableDates()
      .then((dates) => {
        if (cancelled) return;
        setAvailableDates(dates);
        if (dates.length && !dates.includes(selectedDate)) setSelectedDate(dates[dates.length - 1]);
      })
      .catch((e) => {
        if (!cancelled) setError(String(e));
      })
      .finally(() => {
        if (!cancelled) setDatesLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    const range = localDayRange(selectedDate);
    setLoading(true);
    setError("");
    const load = () => Promise.all([getTodayOverview(range.startMs, range.endMs), getCollectorStatus()])
      .then(([data, health]) => {
        if (!cancelled) {
          setOverview(data);
          setStatus(health);
        }
      })
      .catch((e) => {
        if (!cancelled) setError(String(e));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    void load();
    const timer = selectedDate === localDateString() ? window.setInterval(load, 5_000) : undefined;
    return () => {
      cancelled = true;
      if (timer != null) window.clearInterval(timer);
    };
  }, [selectedDate]);

  const statusText = status?.paused ? t("paused") : status?.running ? t("running") : t("stopped");

  return <div className="space-y-5">
    <PageHeader
      title={t("todayTitle")}
      subtitle={t("todaySubtitle")}
      right={<DateRangePicker value={selectedDate} onChange={setSelectedDate} availableDates={availableDates} loading={datesLoading} />}
      status={status}
      statusText={statusText}
    />
    {error && <Notice tone="error">{error}</Notice>}
    {loading ? <Notice>{t("loadingLocalData")}</Notice> : <>
      <div className="grid grid-cols-2 gap-3 xl:grid-cols-4">
        <Metric title={t("activeForeground")} value={formatDuration(overview?.totalActiveForegroundSeconds ?? 0, language)} icon={<CalendarDays size={16} />} tone="teal" />
        <Metric title={t("cpuPeak")} value={peak(overview?.cpuSampledPeak, t("noSample"))} icon={<Cpu size={16} />} tone="blue" />
        <Metric title={t("memoryPeak")} value={peak(overview?.memorySampledPeak, t("noSample"))} icon={<MemoryStick size={16} />} tone="violet" />
        <Metric title={t("diskPeak")} value={`${formatBytes(Math.max(overview?.diskReadSampledPeak ?? 0, overview?.diskWriteSampledPeak ?? 0), language)}/s`} icon={<HardDrive size={16} />} tone="amber" />
      </div>
      <div className="grid gap-3 xl:grid-cols-[minmax(0,1fr)_290px]">
        <Card className="overflow-hidden">
          <CardHeader className="border-b border-border/70"><CardTitle>{t("topApps")}</CardTitle></CardHeader>
          <CardContent className="px-0 pb-0">
            {overview?.topApps.length ? <AppUsageTable apps={overview.topApps} /> : <div className="py-12 text-center text-sm text-muted-foreground">{t("noActiveIntervals")}</div>}
            {(overview?.hiddenActiveForegroundSeconds ?? 0) > 0 && <p className="border-t border-border/70 px-5 py-3 text-xs text-muted-foreground">{t("hiddenAppsTime", { duration: formatDuration(overview!.hiddenActiveForegroundSeconds, language) })}</p>}
          </CardContent>
        </Card>
        <Card className="self-start overflow-hidden">
          <CardHeader className="border-b border-border/70"><CardTitle>{t("collector")}</CardTitle></CardHeader>
          <CardContent className="px-0 pb-0">
            <div className="space-y-3 px-5 py-4 text-sm">
              <div className="flex items-center justify-between gap-3">
                <span className="text-muted-foreground">{t("status")}</span>
                <strong className="flex items-center gap-2 text-xs"><StatusDot status={status} />{statusText}</strong>
              </div>
              <div className="flex items-center justify-between gap-3">
                <span className="text-muted-foreground">{t("lastHeartbeat")}</span>
                <span className="font-medium tabular-nums">{status?.lastHeartbeatAtMs ? new Date(status.lastHeartbeatAtMs).toLocaleTimeString(language) : t("waiting")}</span>
              </div>
              {status?.paused && <div className="warning-surface flex items-center gap-2 rounded-lg border px-3 py-2 text-xs"><PauseCircle size={14} />{t("collectionPaused")}</div>}
            </div>
            <div className="border-t border-border/70">
              <QuickLink icon={<CalendarDays size={16} />} label={t("navTimeline")} onClick={() => setPage("timeline")} />
              <QuickLink icon={<BarChart3 size={16} />} label={t("resourceCharts")} onClick={() => setPage("timeline")} />
            </div>
          </CardContent>
        </Card>
      </div>
    </>}
  </div>;
}

function peak(value: number | null | undefined, noSample: string) {
  return value == null ? noSample : `${value.toFixed(1)}%`;
}

const metricTones = {
  teal: "bg-muted text-[hsl(var(--signal-cyan))] ring-border",
  blue: "bg-muted text-[hsl(var(--signal-blue))] ring-border",
  violet: "bg-muted text-[hsl(var(--signal-violet))] ring-border",
  amber: "bg-muted text-[hsl(var(--signal-amber))] ring-border"
};

const metricBars = {
  teal: "bg-[hsl(var(--signal-cyan))]",
  blue: "bg-[hsl(var(--signal-blue))]",
  violet: "bg-[hsl(var(--signal-violet))]",
  amber: "bg-[hsl(var(--signal-amber))]"
};

function Metric({ title, value, icon, tone }: { title: string; value: string; icon: React.ReactNode; tone: keyof typeof metricTones }) {
  return <Card className="relative overflow-hidden">
    <div className={`absolute inset-x-0 top-0 h-0.5 ${metricBars[tone]}`} />
    <CardContent className="pt-4">
      <div className="flex items-center justify-between gap-3">
        <div className="text-xs font-medium text-muted-foreground">{title}</div>
        <div className={`flex h-7 w-7 items-center justify-center rounded-lg ring-1 ring-inset ${metricTones[tone]}`}>{icon}</div>
      </div>
      <div className="metric-value mt-4 truncate text-[25px] font-semibold leading-none">{value}</div>
    </CardContent>
  </Card>;
}

function PageHeader({ title, subtitle, right, status, statusText }: {
  title: string;
  subtitle: string;
  right?: React.ReactNode;
  status: CollectorStatus | null;
  statusText: string;
}) {
  return <div className="flex items-center justify-between gap-4">
    <div>
      <div className="flex flex-wrap items-center gap-2.5">
        <h1 className="page-title text-[26px] font-semibold">{title}</h1>
        <span className="inline-flex items-center gap-2 rounded-full border border-border bg-card px-2.5 py-1 text-[11px] font-semibold text-muted-foreground shadow-[0_1px_2px_rgba(15,23,42,0.04)]">
          <StatusDot status={status} title={statusText} />
          {statusText}
        </span>
      </div>
      <p className="mt-1 text-sm text-muted-foreground">{subtitle}</p>
    </div>
    {right}
  </div>;
}

function StatusDot({ status, title }: { status: CollectorStatus | null; title?: string }) {
  return <span className={`h-2 w-2 rounded-full ${status?.paused ? "bg-amber-500" : status?.running ? "bg-emerald-500" : "bg-slate-400"}`} title={title} />;
}

function QuickLink({ icon, label, onClick }: { icon: React.ReactNode; label: string; onClick: () => void }) {
  return <button type="button" onClick={onClick} className="group flex w-full items-center gap-3 border-b border-border/60 px-5 py-3 text-left text-sm font-medium transition-colors hover:bg-muted/40 last:border-b-0">
    <span className="text-muted-foreground group-hover:text-primary">{icon}</span>
    <span>{label}</span>
    <ArrowRight size={14} className="ml-auto text-muted-foreground transition-transform group-hover:translate-x-0.5" />
  </button>;
}

function Notice({ children, tone = "default" }: { children: React.ReactNode; tone?: "default" | "error" }) {
  return <div className={`surface-shadow rounded-lg border px-4 py-6 text-sm ${tone === "error" ? "error-surface" : "border-border bg-card text-muted-foreground"}`}>{children}</div>;
}
