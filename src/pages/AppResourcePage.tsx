import { useEffect, useState } from "react";
import { Cpu, Database, HardDrive, MemoryStick } from "lucide-react";
import {
  getAppResourceAvailableDates,
  getAppResourceHistory,
  getResourceApps
} from "../api/tauriApi";
import { AppResourceHistoryChart } from "../components/AppResourceHistoryChart";
import { AppResourcePicker } from "../components/AppResourcePicker";
import { DateRangePicker } from "../components/DateRangePicker";
import { Card, CardContent, CardHeader, CardTitle } from "../components/ui/Card";
import { useI18n } from "../i18n";
import type { AppResourceHistoryPoint, ResourceApp } from "../types/resource";
import { formatBytes, localDateString, localDayRange } from "../utils/time";

type PresetDays = 1 | 7 | 30;

export function AppResourcePage() {
  const { language, t } = useI18n();
  const [apps, setApps] = useState<ResourceApp[]>([]);
  const [appsLoading, setAppsLoading] = useState(true);
  const [appsError, setAppsError] = useState("");
  const [selectedAppKey, setSelectedAppKey] = useState("");
  const [availableDates, setAvailableDates] = useState<string[]>([]);
  const [datesLoading, setDatesLoading] = useState(false);
  const [startDate, setStartDate] = useState(localDateString);
  const [endDate, setEndDate] = useState(localDateString);
  const [preset, setPreset] = useState<PresetDays | null>(1);
  const [history, setHistory] = useState<AppResourceHistoryPoint[]>([]);
  const [historyLoading, setHistoryLoading] = useState(false);
  const [historyError, setHistoryError] = useState("");

  useEffect(() => {
    let cancelled = false;
    setAppsLoading(true);
    getResourceApps()
      .then((data) => {
        if (cancelled) return;
        setApps(data);
        const newest = [...data].sort((a, b) => b.lastSampleAtMs - a.lastSampleAtMs)[0];
        if (newest) setSelectedAppKey(newest.appKey);
      })
      .catch((error) => {
        if (!cancelled) setAppsError(String(error));
      })
      .finally(() => {
        if (!cancelled) setAppsLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (!selectedAppKey) {
      setAvailableDates([]);
      setHistory([]);
      return;
    }
    let cancelled = false;
    setDatesLoading(true);
    setHistoryError("");
    setAvailableDates([]);
    getAppResourceAvailableDates(selectedAppKey)
      .then((dates) => {
        if (cancelled) return;
        setAvailableDates(dates);
        const latest = dates[dates.length - 1];
        if (latest) {
          setStartDate(latest);
          setEndDate(latest);
          setPreset(1);
        }
      })
      .catch((error) => {
        if (!cancelled) setHistoryError(String(error));
      })
      .finally(() => {
        if (!cancelled) setDatesLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [selectedAppKey]);

  useEffect(() => {
    if (
      !selectedAppKey ||
      !availableDates.includes(startDate) ||
      !availableDates.includes(endDate) ||
      startDate > endDate
    ) {
      setHistory([]);
      return;
    }
    let cancelled = false;
    const startMs = localDayRange(startDate).startMs;
    const endMs = localDayRange(endDate).endMs;
    setHistoryLoading(true);
    setHistoryError("");
    getAppResourceHistory(selectedAppKey, startMs, endMs, 5_000)
      .then((data) => {
        if (!cancelled) setHistory(data);
      })
      .catch((error) => {
        if (!cancelled) setHistoryError(String(error));
      })
      .finally(() => {
        if (!cancelled) setHistoryLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [availableDates, endDate, selectedAppKey, startDate]);

  const selectedApp = apps.find((app) => app.appKey === selectedAppKey);
  const captured = history.filter((point) =>
    point.cpuPercent != null ||
    point.memoryUsedBytes != null ||
    point.ioReadBytesPerSec != null ||
    point.ioWriteBytesPerSec != null
  );
  const summary = summarize(captured);

  function applyPreset(days: PresetDays) {
    if (!availableDates.length) return;
    const rangeEnd = availableDates.includes(endDate)
      ? endDate
      : availableDates[availableDates.length - 1];
    const endStartMs = localDayRange(rangeEnd).startMs;
    const cutoff = localDateString(new Date(endStartMs - (days - 1) * 86_400_000));
    const candidates = availableDates.filter((date) => date >= cutoff && date <= rangeEnd);
    setStartDate(candidates[0] ?? availableDates[0]);
    setEndDate(rangeEnd);
    setPreset(days);
  }

  function changeStart(date: string) {
    setStartDate(date);
    if (date > endDate) setEndDate(date);
    setPreset(null);
  }

  function changeEnd(date: string) {
    setEndDate(date);
    if (date < startDate) setStartDate(date);
    setPreset(null);
  }

  return <div className="space-y-5">
    <div>
      <h1 className="page-title text-[26px] font-semibold">{t("appResourcesTitle")}</h1>
      <p className="mt-1 text-sm text-muted-foreground">{t("appResourcesSubtitle")}</p>
    </div>

    <Card>
      <CardHeader className="border-b border-border/70">
        <CardTitle>{t("sampledApps")}</CardTitle>
      </CardHeader>
      <CardContent className="pt-4">
        {appsError ? <StateMessage tone="error">{appsError}</StateMessage> :
          appsLoading ? <StateMessage>{t("loadingResourceApps")}</StateMessage> :
            apps.length ? <AppResourcePicker apps={apps} value={selectedAppKey} onChange={setSelectedAppKey} /> :
              <StateMessage>{t("noResourceApps")}</StateMessage>}

        {selectedApp && <>
          <div className="mt-4 flex flex-wrap items-start justify-between gap-3 rounded-lg bg-muted/45 px-4 py-3">
            <div className="min-w-0">
              <div className="text-sm font-semibold">{selectedApp.displayName}</div>
              {selectedApp.exePath && <div className="mt-1 max-w-4xl truncate text-[11px] text-muted-foreground" title={selectedApp.exePath}>{selectedApp.exePath}</div>}
            </div>
            <div className="rounded-full bg-card px-3 py-1 text-[11px] text-muted-foreground ring-1 ring-inset ring-border">{t("mergedAppVersions")}</div>
          </div>

          <div className="mt-5 flex flex-wrap items-end gap-3">
            <div className="shrink-0">
              <div className="mb-1.5 text-[11px] font-medium text-muted-foreground">{t("startDate")}</div>
              <DateRangePicker value={startDate} onChange={changeStart} availableDates={availableDates} loading={datesLoading} />
            </div>
            <div className="shrink-0">
              <div className="mb-1.5 text-[11px] font-medium text-muted-foreground">{t("endDate")}</div>
              <DateRangePicker value={endDate} onChange={changeEnd} availableDates={availableDates} loading={datesLoading} />
            </div>
            <div className="flex h-9 shrink-0 items-center rounded-lg border border-input bg-card p-1">
              {([1, 7, 30] as PresetDays[]).map((days) => <button
                key={days}
                type="button"
                className={`h-7 rounded-md px-3 text-xs font-medium transition-colors ${preset === days ? "bg-primary text-primary-foreground" : "text-muted-foreground hover:bg-muted"}`}
                onClick={() => applyPreset(days)}
              >{t(days === 1 ? "singleDay" : days === 7 ? "last7Days" : "last30Days")}</button>)}
            </div>
          </div>
        </>}
      </CardContent>
    </Card>

    {!selectedApp ? <Card><StateMessage spacious>{t("chooseApp")}</StateMessage></Card> : <>
      <div className="grid grid-cols-2 gap-3 2xl:grid-cols-5">
        <Metric label={t("sampledPoints")} value={String(captured.length)} icon={<Database size={15} />} tone="teal" />
        <Metric label={t("cpuAverage")} value={summary.cpuAverage == null ? t("noSample") : `${summary.cpuAverage.toFixed(1)}%`} icon={<Cpu size={15} />} tone="blue" />
        <Metric label={t("cpuPeak")} value={summary.cpuPeak == null ? t("noSample") : `${summary.cpuPeak.toFixed(1)}%`} icon={<Cpu size={15} />} tone="violet" />
        <Metric label={t("memoryPeak")} value={formatBytes(summary.memoryPeak, language)} icon={<MemoryStick size={15} />} tone="amber" />
        <Metric label={t("ioPeak")} value={`${formatBytes(summary.ioPeak, language)}/s`} icon={<HardDrive size={15} />} tone="teal" />
      </div>

      <Card>
        <CardHeader className="border-b border-border/70">
          <CardTitle>{t("appHistoryTitle")}</CardTitle>
          <p className="text-xs font-normal text-muted-foreground">{formatRange(startDate, endDate, language)}</p>
        </CardHeader>
        <CardContent className="pt-4">
          {historyError ? <StateMessage tone="error">{historyError}</StateMessage> :
            datesLoading || historyLoading ? <StateMessage>{t("loadingAppHistory")}</StateMessage> :
              captured.length ? <>
                <AppResourceHistoryChart points={history} />
                <p className="mt-2 text-xs text-muted-foreground">{t("appHistoryCoverage")}</p>
              </> : <StateMessage spacious>{t("noAppHistoryForRange")}</StateMessage>}
        </CardContent>
      </Card>
    </>}
  </div>;
}

const metricTones = {
  teal: "bg-muted text-[hsl(var(--signal-cyan))]",
  blue: "bg-muted text-[hsl(var(--signal-blue))]",
  violet: "bg-muted text-[hsl(var(--signal-violet))]",
  amber: "bg-muted text-[hsl(var(--signal-amber))]"
};

function Metric({ label, value, icon, tone }: {
  label: string;
  value: string;
  icon: React.ReactNode;
  tone: keyof typeof metricTones;
}) {
  return <Card>
    <CardContent className="pt-4">
      <div className="flex items-center justify-between gap-2 text-xs text-muted-foreground">
        <span>{label}</span>
        <span className={`flex h-7 w-7 items-center justify-center rounded-lg ${metricTones[tone]}`}>{icon}</span>
      </div>
      <div className="metric-value mt-3 truncate text-xl font-semibold">{value}</div>
    </CardContent>
  </Card>;
}

function StateMessage({ children, tone = "default", spacious = false }: {
  children: React.ReactNode;
  tone?: "default" | "error";
  spacious?: boolean;
}) {
  return <div className={`${spacious ? "py-14" : "py-8"} text-center text-sm ${tone === "error" ? "text-[hsl(var(--danger))]" : "text-muted-foreground"}`}>{children}</div>;
}

function summarize(points: AppResourceHistoryPoint[]) {
  const cpu = points.flatMap((point) => point.cpuPercent == null ? [] : [point.cpuPercent]);
  const memory = points.flatMap((point) => point.memoryUsedBytes == null ? [] : [point.memoryUsedBytes]);
  const io = points.flatMap((point) => {
    if (point.ioReadBytesPerSec == null && point.ioWriteBytesPerSec == null) return [];
    return [(point.ioReadBytesPerSec ?? 0) + (point.ioWriteBytesPerSec ?? 0)];
  });
  return {
    cpuAverage: cpu.length ? cpu.reduce((sum, value) => sum + value, 0) / cpu.length : null,
    cpuPeak: cpu.length ? Math.max(...cpu) : null,
    memoryPeak: memory.length ? Math.max(...memory) : null,
    ioPeak: io.length ? Math.max(...io) : null
  };
}

function formatRange(startDate: string, endDate: string, language: string) {
  const locale = language === "zh-CN" ? "zh-CN" : "en";
  const start = new Date(`${startDate}T12:00:00`).toLocaleDateString(locale);
  if (startDate === endDate) return start;
  const end = new Date(`${endDate}T12:00:00`).toLocaleDateString(locale);
  return `${start} – ${end}`;
}
