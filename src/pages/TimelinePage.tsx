import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Activity, AlertTriangle, CircleHelp, Cpu, Database, HardDrive, MemoryStick, MonitorCog } from "lucide-react";
import {
  getAppResourceSamples,
  getCollectionSettings,
  getCollectorStatus,
  getDashboardConfig,
  getResourceAvailableDates,
  getSystemTimeline,
  setDashboardConfig
} from "../api/tauriApi";
import { DashboardChartCard } from "../components/DashboardChartCard";
import { DashboardEditor } from "../components/DashboardEditor";
import { DateRangePicker } from "../components/DateRangePicker";
import { ResourceTimelineChart } from "../components/ResourceTimelineChart";
import { Badge } from "../components/ui/Badge";
import { Button } from "../components/ui/Button";
import { Card, CardContent, CardHeader, CardTitle } from "../components/ui/Card";
import { useI18n } from "../i18n";
import { useUiStore } from "../stores/uiStore";
import { createDefaultDashboardConfig, validateDashboardConfig, type DashboardConfig } from "../dashboard/config";
import { canPersistDashboard, classifyDashboardLoad, isDashboardEditable, type DashboardLoadState } from "../dashboard/loadState";
import type { AppResourceSample, CapabilityState, CollectionSettings, CollectorStatus, GpuSample, ProviderStatus, SystemSample, TimelineQueryResult } from "../types/resource";
import { effectiveTimelineDate, formatBytes, formatClock, localDateString, millisecondsUntilLocalMidnight, timelineWindowRange } from "../utils/time";
import { aggregateCategoryCapability, gpuDevices, metricDataState, timelineCoverageState, timelineRefreshIntervalMs } from "../utils/uiSemantics";

type WindowPreset = 1 | 7 | 30;

export function TimelinePage() {
  const { language, t } = useI18n();
  const selectedDate = useUiStore((state) => state.selectedDate);
  const setSelectedDate = useUiStore((state) => state.setSelectedDate);
  const [preset, setPreset] = useState<WindowPreset>(1);
  const [availableDates, setAvailableDates] = useState<string[]>([]);
  const [datesLoading, setDatesLoading] = useState(true);
  const [timeline, setTimeline] = useState<TimelineQueryResult | null>(null);
  const [status, setStatus] = useState<CollectorStatus | null>(null);
  const [settings, setSettings] = useState<CollectionSettings | null>(null);
  const [dashboardConfig, setDashboardConfigState] = useState<DashboardConfig | null>(null);
  const [dashboardLoadState, setDashboardLoadState] = useState<DashboardLoadState>("loading");
  const [dashboardCustomizing, setDashboardCustomizing] = useState(false);
  const [dashboardDirty, setDashboardDirty] = useState(false);
  const [dashboardSaving, setDashboardSaving] = useState(false);
  const [dashboardSaveError, setDashboardSaveError] = useState("");
  const [selected, setSelected] = useState<SystemSample | null>(null);
  const [processEvidence, setProcessEvidence] = useState<AppResourceSample[]>([]);
  const [processLoading, setProcessLoading] = useState(false);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [error, setError] = useState("");
  const [refreshError, setRefreshError] = useState("");
  const timelineRef = useRef<TimelineQueryResult | null>(null);
  const requestIdRef = useRef(0);
  const dashboardLoadRequestRef = useRef(0);
  const dashboardSaveRevisionRef = useRef(0);
  const mountedRef = useRef(false);
  // Deliberate historical selections stay fixed; only a view selected as today follows local midnight.
  const [followsCurrentDate, setFollowsCurrentDate] = useState(() => selectedDate === localDateString());

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      requestIdRef.current += 1;
    };
  }, []);

  const handleDateChange = useCallback((date: string) => {
    setFollowsCurrentDate(date === localDateString());
    setSelectedDate(date);
  }, [setSelectedDate]);

  const [range, setRange] = useState(() => timelineWindowRange(
    effectiveTimelineDate(selectedDate, selectedDate === localDateString()),
    preset
  ));
  const loadTimeline = useCallback((background = false) => {
    let cancelled = false;
    const requestId = requestIdRef.current + 1;
    requestIdRef.current = requestId;
    const nowMs = Date.now();
    const nextDate = effectiveTimelineDate(selectedDate, followsCurrentDate, nowMs);
    const nextRange = timelineWindowRange(nextDate, preset, nowMs);
    if (nextDate !== selectedDate) setSelectedDate(nextDate);
    if (background) {
      setRefreshing(true);
      setRefreshError("");
    } else {
      timelineRef.current = null;
      setTimeline(null);
      setLoading(true);
      setError("");
      setRefreshError("");
    }
    Promise.all([
      getSystemTimeline(nextRange.startMs, nextRange.endMs, 2_500),
      getCollectorStatus(),
      getCollectionSettings()
    ])
      .then(([nextTimeline, nextStatus, nextSettings]) => {
        if (!mountedRef.current || cancelled || requestId !== requestIdRef.current) return;
        setRange(nextRange);
        timelineRef.current = nextTimeline;
        setTimeline(nextTimeline);
        setStatus(nextStatus);
        setSettings(nextSettings);
        setSelected((current) => current && nextTimeline.samples.some((sample) => sample.timestampMs === current.timestampMs) ? current : null);
      })
      .catch(() => {
        if (!mountedRef.current || cancelled || requestId !== requestIdRef.current) return;
        if (background && timelineRef.current) setRefreshError(t("timelineRefreshFailed"));
        else setError(t("timelineErrorMessage"));
      })
      .finally(() => {
        if (!mountedRef.current || cancelled || requestId !== requestIdRef.current) return;
        if (background) setRefreshing(false);
        else setLoading(false);
      });
    return () => { cancelled = true; };
  }, [followsCurrentDate, preset, selectedDate, setSelectedDate, t]);

  useEffect(() => {
    let cancelled = false;
    setDatesLoading(true);
    getResourceAvailableDates()
      .then((dates) => {
        if (cancelled) return;
        setAvailableDates(dates);
        if (dates.length && !dates.includes(selectedDate) && !followsCurrentDate) setSelectedDate(dates[dates.length - 1]);
      })
      .catch(() => {
        if (!cancelled) setError(t("timelineErrorMessage"));
      })
      .finally(() => {
        if (!cancelled) setDatesLoading(false);
      });
    return () => { cancelled = true; };
  }, [followsCurrentDate, selectedDate, setSelectedDate, t]);

  const loadDashboardConfig = useCallback(() => {
    let cancelled = false;
    const requestId = dashboardLoadRequestRef.current + 1;
    dashboardLoadRequestRef.current = requestId;
    dashboardSaveRevisionRef.current += 1;
    setDashboardLoadState("loading");
    setDashboardCustomizing(false);
    setDashboardDirty(false);
    setDashboardSaveError("");
    getDashboardConfig()
      .then((config) => {
        if (cancelled || requestId !== dashboardLoadRequestRef.current) return;
        const validation = config ? validateDashboardConfig(config) : null;
        setDashboardConfigState(validation?.ok ? validation.config : null);
        setDashboardLoadState(classifyDashboardLoad(config, validation?.ok === true));
      })
      .catch(() => {
        if (cancelled || requestId !== dashboardLoadRequestRef.current) return;
        setDashboardLoadState("failed");
      });
    return () => { cancelled = true; };
  }, []);

  useEffect(() => loadDashboardConfig(), [loadDashboardConfig]);

  useEffect(() => {
    if (!canPersistDashboard(dashboardLoadState, dashboardDirty, dashboardConfig)) return;
    const timer = window.setTimeout(() => {
      const revision = dashboardSaveRevisionRef.current + 1;
      dashboardSaveRevisionRef.current = revision;
      setDashboardSaving(true);
      setDashboardSaveError("");
      setDashboardConfig(dashboardConfig)
        .then(() => {
          if (dashboardSaveRevisionRef.current === revision) setDashboardDirty(false);
        })
        .catch(() => {
          if (dashboardSaveRevisionRef.current === revision) setDashboardSaveError(t("dashboardSaveError"));
        })
        .finally(() => {
          if (dashboardSaveRevisionRef.current === revision) setDashboardSaving(false);
        });
    }, 250);
    return () => {
      window.clearTimeout(timer);
      dashboardSaveRevisionRef.current += 1;
    };
  }, [dashboardConfig, dashboardDirty, dashboardLoadState, t]);

  useEffect(() => loadTimeline(), [loadTimeline]);

  useEffect(() => {
    if (!followsCurrentDate) return;
    const currentDate = localDateString();
    if (selectedDate !== currentDate) {
      setSelectedDate(currentDate);
      return;
    }
    const rolloverTimer = window.setTimeout(() => {
      const nextDate = localDateString();
      if (nextDate !== selectedDate) setSelectedDate(nextDate);
    }, millisecondsUntilLocalMidnight());
    return () => window.clearTimeout(rolloverTimer);
  }, [followsCurrentDate, selectedDate, setSelectedDate]);

  useEffect(() => {
    const refreshIntervalMs = timelineRefreshIntervalMs(preset, followsCurrentDate);
    const timer = refreshIntervalMs != null
      ? window.setInterval(() => {
        const nextDate = effectiveTimelineDate(selectedDate, followsCurrentDate);
        if (nextDate !== selectedDate) {
          setSelectedDate(nextDate);
          return;
        }
        loadTimeline(true);
      }, refreshIntervalMs)
      : undefined;
    return () => {
      if (timer != null) window.clearInterval(timer);
      requestIdRef.current += 1;
    };
  }, [followsCurrentDate, loadTimeline, preset, selectedDate, setSelectedDate]);

  useEffect(() => {
    if (!selected?.hasAppSnapshot) {
      setProcessEvidence([]);
      setProcessLoading(false);
      return;
    }
    let cancelled = false;
    setProcessLoading(true);
    getAppResourceSamples(selected.timestampMs)
      .then((items) => { if (!cancelled) setProcessEvidence(items); })
      .catch(() => { if (!cancelled) setProcessEvidence([]); })
      .finally(() => { if (!cancelled) setProcessLoading(false); });
    return () => { cancelled = true; };
  }, [selected]);

  const samples = timeline?.samples ?? [];
  const latest = selected ?? samples[samples.length - 1] ?? null;
  const devices = useMemo(() => gpuDevices(samples), [samples]);
  const coverage = timeline?.coverage ?? 0;
  const selectedTime = selected ? formatClock(selected.timestampMs, language) : null;
  const effectiveDashboardConfig = dashboardConfig ?? createDefaultDashboardConfig(samples);
  const dashboardEditable = isDashboardEditable(dashboardLoadState);

  const updateDashboardConfig = useCallback((next: DashboardConfig) => {
    if (!isDashboardEditable(dashboardLoadState)) return;
    const validation = validateDashboardConfig(next);
    if (!validation.ok) return;
    setDashboardConfigState(validation.config);
    setDashboardDirty(true);
    setDashboardSaveError("");
  }, [dashboardLoadState]);

  const restoreDashboardDefaults = useCallback(() => {
    if (window.confirm(t("dashboardRestoreConfirm"))) updateDashboardConfig(createDefaultDashboardConfig(samples));
  }, [samples, t, updateDashboardConfig]);

  return <div className="space-y-5">
    <header className="flex flex-wrap items-end justify-between gap-4">
      <div>
        <div className="eyebrow">{t("timelineWindow")}</div>
        <h1 className="page-title mt-1 text-[28px] font-semibold tracking-[-0.02em]">{t("timelinePageTitle")}</h1>
        <p className="mt-1 max-w-2xl text-sm text-muted-foreground">{t("timelinePageSubtitle")}</p>
      </div>
      <div className="flex flex-wrap items-center gap-2">
        <div className="segmented-control" aria-label={t("timelineWindow")}>
          {([1, 7, 30] as WindowPreset[]).map((days) => <button key={days} type="button" className={preset === days ? "segmented-control-active" : "segmented-control-item"} onClick={() => setPreset(days)}>{t(days === 1 ? "rangeDay" : days === 7 ? "range7Days" : "range30Days")}</button>)}
        </div>
        <DateRangePicker value={selectedDate} onChange={handleDateChange} availableDates={availableDates} loading={datesLoading} />
        <Button type="button" variant={dashboardCustomizing ? "default" : "outline"} disabled={!dashboardEditable} onClick={() => setDashboardCustomizing((value) => !value)}>{dashboardCustomizing ? t("dashboardDone") : t("dashboardCustomize")}</Button>
      </div>
    </header>

    {error && !timeline ? <InlineError message={error} onRetry={loadTimeline} title={t("timelineErrorTitle")} /> : loading ? <TimelineLoading /> : <>
      {(refreshing || refreshError) && <div role={refreshError ? "alert" : "status"} className={`${refreshError ? "error-surface" : "border-border bg-muted/40 text-muted-foreground"} flex flex-wrap items-center justify-between gap-3 rounded-lg border px-4 py-2.5 text-xs`}><span>{refreshError || t("timelineRefreshing")}</span>{refreshError && <Button variant="outline" className="h-8 px-2.5 text-xs" onClick={() => loadTimeline(true)}>{t("retry")}</Button>}</div>}
      {dashboardCustomizing && dashboardEditable && <DashboardEditor config={effectiveDashboardConfig} samples={samples} onChange={updateDashboardConfig} onRestoreDefaults={restoreDashboardDefaults} saving={dashboardSaving} saveError={dashboardSaveError} />}
      <section aria-labelledby="dashboard-title" className="space-y-3">
        <div className="flex flex-wrap items-end justify-between gap-3">
          <div>
            <div className="eyebrow">{t("dashboardTitle")}</div>
            <h2 id="dashboard-title" className="mt-1 text-lg font-semibold">{t("dashboardTitle")}</h2>
          </div>
          {dashboardLoadState === "loading" && <span className="text-xs text-muted-foreground">{t("loadingLocalData")}</span>}
        </div>
        {dashboardLoadState === "failed" && <InlineError title={t("dashboardLoadErrorTitle")} message={t("dashboardLoadErrorMessage")} onRetry={loadDashboardConfig} />}
        {effectiveDashboardConfig.cards.some((card) => card.visible) ? <div className="grid gap-3 xl:grid-cols-2">{effectiveDashboardConfig.cards.filter((card) => card.visible).sort((left, right) => left.order - right.order).map((card) => <DashboardChartCard key={card.id} card={card} samples={samples} gaps={timeline?.gaps ?? []} startMs={range.startMs} endMs={range.endMs} selectedTimestampMs={selected?.timestampMs ?? null} onSampleSelect={setSelected} />)}</div> : <Card><CardContent className="py-10 text-center text-sm text-muted-foreground">{t("dashboardNoCards")}</CardContent></Card>}
      </section>
      <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
        <SignalCard icon={<Cpu size={16} />} label={t("metricCpu")} value={latest?.cpuPercent} capability={aggregateCategoryCapability(status?.providerStatus ?? [], settings, "cpu")} unit="percent" />
        <SignalCard icon={<MemoryStick size={16} />} label={t("metricMemory")} value={latest?.memoryPercent} capability={aggregateCategoryCapability(status?.providerStatus ?? [], settings, "memory")} unit="percent" />
        <SignalCard icon={<HardDrive size={16} />} label={t("metricDiskRead")} value={latest?.diskReadBytesPerSec} capability={aggregateCategoryCapability(status?.providerStatus ?? [], settings, "disk")} unit="rate" />
        <SignalCard icon={<Database size={16} />} label={t("timelineCoverage")} value={coverage} capability={timelineCoverageState(coverage) === "incomplete" ? "incomplete" : "supportedEnabled"} unit="coverage" />
      </div>

      <Card className="overflow-hidden">
        <CardHeader className="border-b border-border/70 bg-card/90">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <div>
              <CardTitle>{t("systemSample")}</CardTitle>
              <p className="mt-1 text-xs font-normal text-muted-foreground">{selectedTime ? t("selectedTimestamp", { time: selectedTime }) : t("clickTimelineHint")}</p>
            </div>
            <div className="flex items-center gap-2 text-xs text-muted-foreground"><Activity size={14} className="text-[hsl(var(--signal-cyan))]" />{t("observedCoverage", { percent: Math.round(coverage * 100) })}</div>
          </div>
        </CardHeader>
        <CardContent className="pt-4">
          {samples.length ? <ResourceTimelineChart samples={samples} gaps={timeline?.gaps ?? []} startMs={range.startMs} endMs={range.endMs} selectedTimestampMs={selected?.timestampMs ?? null} onSampleSelect={setSelected} ariaLabel={t("timelinePageTitle")} /> : <EmptyState title={t("timelineEmpty")} hint={t("timelineEmptyHint")} />}
          <div className="mt-2 flex flex-wrap items-center gap-x-5 gap-y-2 border-t border-border/70 pt-3 text-[11px] text-muted-foreground">
            <span className="font-medium text-foreground">{t("timelineLegend")}</span><span>— {t("legendZero")}</span><span>╱ {t("legendMissing")}</span><span>□ {t("legendDisabled")}</span><span>! {t("legendFailed")}</span>
          </div>
        </CardContent>
      </Card>

      <div className="grid gap-3 xl:grid-cols-[minmax(0,1.25fr)_minmax(320px,0.75fr)]">
        <GpuPanel devices={devices} samples={samples} sample={latest} status={status} settings={settings} language={language} />
        <CollectionStatusPanel providers={status?.providerStatus ?? []} />
      </div>

      <ProcessEvidencePanel sample={selected} items={processEvidence} loading={processLoading} language={language} />
    </>}
  </div>;
}

function SignalCard({ icon, label, value, capability, unit }: { icon: React.ReactNode; label: string; value: number | null | undefined; capability: CapabilityState | "incomplete" | undefined; unit: "percent" | "rate" | "coverage" }) {
  const { language, t } = useI18n();
  const state = unit === "coverage" ? capability === "incomplete" ? "incomplete" : "value" : metricDataState(value, capability as CapabilityState | undefined);
  const formatted = state === "disabled" ? t("disabledByUser")
    : state === "unsupported" ? t("stateUnsupported")
      : state === "failed" ? t("stateFailed")
        : state === "incomplete" ? t("observedCoverage", { percent: Math.round((value ?? 0) * 100) })
        : state === "missing" ? t("missingData")
          : value == null ? t("missingData")
            : unit === "coverage" ? `${Math.round(value * 100)}%`
              : unit === "percent" ? `${value.toFixed(1)}%${state === "zero" ? ` · ${t("realZero")}` : ""}`
                : `${formatBytes(value, language)}${state === "zero" ? ` · ${t("measuredZero")}` : ""}/s`;
  return <Card className="relative overflow-hidden"><div className={`absolute inset-x-0 top-0 h-0.5 ${state === "failed" ? "bg-[hsl(var(--danger))]" : state === "incomplete" ? "bg-[hsl(var(--warning))]" : state === "disabled" || state === "unsupported" ? "bg-[hsl(var(--muted-foreground)/0.35)]" : "bg-[hsl(var(--signal-cyan))]"}`} /><CardContent className="pt-4"><div className="flex items-center justify-between gap-3"><span className="text-xs font-medium text-muted-foreground">{label}</span><span className="flex h-7 w-7 items-center justify-center rounded-lg bg-muted text-[hsl(var(--signal-cyan))]">{icon}</span></div><div className="metric-value mt-3 truncate text-[21px] font-semibold">{formatted}</div></CardContent></Card>;
}

function GpuPanel({ devices, samples, sample, status, settings, language }: { devices: GpuSample[]; samples: SystemSample[]; sample: SystemSample | null; status: CollectorStatus | null; settings: CollectionSettings | null; language: "en" | "zh-CN" }) {
  const { t } = useI18n();
  const capability = aggregateCategoryCapability(status?.providerStatus ?? [], settings, "gpu");
  return <Card className="overflow-hidden"><CardHeader className="border-b border-border/70"><div className="flex items-center justify-between gap-3"><CardTitle>{t("gpuDevices")}</CardTitle><CapabilityBadge state={capability} /></div></CardHeader><CardContent className="pt-4">{!devices.length ? <div className="flex items-start gap-3 rounded-lg border border-dashed border-border px-4 py-5 text-sm text-muted-foreground"><CircleHelp size={17} className="mt-0.5 shrink-0" /><span>{capability === "supportedDisabled" ? t("disabledByUser") : capability === "unsupported" ? t("stateUnsupported") : capability === "failed" ? t("stateFailed") : t("noGpuDevices")}</span></div> : <div className="grid gap-3 md:grid-cols-2">{devices.map((device) => {
    const current = sample?.gpus.find((item) => item.deviceKey === device.deviceKey);
    const deviceSamples = samples.flatMap((item) => item.gpus.filter((gpu) => gpu.deviceKey === device.deviceKey));
    const hasMetric = (read: (gpu: GpuSample) => number | null) => deviceSamples.some((gpu) => read(gpu) != null);
    return <div key={device.deviceKey} className="rounded-lg border border-border/80 bg-muted/20 p-3"><div className="flex items-start justify-between gap-3"><div className="min-w-0"><div className="truncate text-sm font-semibold" title={device.deviceKey}>{[device.vendor, device.model].filter(Boolean).join(" ") || device.deviceKey}</div><div className="mt-1 truncate font-mono text-[10px] text-muted-foreground">{device.deviceKey}</div></div><Badge className="shrink-0 border-border bg-card text-muted-foreground">{current ? formatClock(sample?.timestampMs ?? 0, language) : t("gpuNoSamples")}</Badge></div><div className="mt-3 grid grid-cols-2 gap-x-4 gap-y-3 text-xs">{hasMetric((gpu) => gpu.utilizationPercent) && <GpuMetric label={t("metricGpuUsage")} value={current?.utilizationPercent ?? null} unit="percent" />}{hasMetric((gpu) => gpu.temperatureCelsius) && <GpuMetric label={t("metricGpuTemp")} value={current?.temperatureCelsius ?? null} unit="temperature" />}{hasMetric((gpu) => gpu.powerWatts) && <GpuMetric label={t("metricGpuPower")} value={current?.powerWatts ?? null} unit="power" />}{hasMetric((gpu) => gpu.vramUsedBytes) && <GpuMetric label={t("metricGpuVram")} value={current?.vramUsedBytes ?? null} unit="bytes" language={language} />}</div></div>;
  })}</div>}</CardContent></Card>;
}

function GpuMetric({ label, value, unit, language }: { label: string; value: number | null; unit: "percent" | "temperature" | "power" | "bytes"; language?: "en" | "zh-CN" }) {
  const { t } = useI18n();
  const state = metricDataState(value, undefined);
  const formatted = value == null ? t("missingData") : unit === "percent" ? `${value.toFixed(1)}%${state === "zero" ? ` · ${t("realZero")}` : ""}` : unit === "temperature" ? `${value.toFixed(1)} °C` : unit === "power" ? `${value.toFixed(1)} W` : formatBytes(value, language);
  return <div><div className="text-[11px] text-muted-foreground">{label}</div><div className={`mt-1 font-mono text-xs font-medium ${state === "missing" ? "text-muted-foreground" : "text-foreground"}`}>{formatted}</div></div>;
}

function CollectionStatusPanel({ providers }: { providers: ProviderStatus[] }) {
  const { t, language } = useI18n();
  return <Card className="overflow-hidden"><CardHeader className="border-b border-border/70"><CardTitle>{t("resourceStatus")}</CardTitle><p className="text-xs font-normal text-muted-foreground">{t("resourceStatusDescription")}</p></CardHeader><CardContent className="pt-4">{providers.length ? <div className="space-y-3">{providers.map((provider) => <div key={provider.providerId} className="rounded-lg border border-border/80 bg-muted/15 p-3"><div className="flex items-start justify-between gap-3"><div><div className="text-sm font-semibold">{provider.displayName}</div><div className="mt-1 font-mono text-[10px] text-muted-foreground">{provider.providerId}</div></div><CapabilityBadge lifecycle={provider.lifecycle} supported={provider.supported} /></div><div className="mt-3 flex flex-wrap gap-1.5">{provider.capabilities.map((capability) => <span key={`${provider.providerId}-${capability.category}`} className="rounded-full border border-border bg-card px-2 py-1 text-[10px] text-muted-foreground">{capability.category} · {capabilityLabel(capability.state, t)}</span>)}</div>{provider.lastError && <div className="mt-3 flex items-start gap-2 text-xs text-[hsl(var(--danger))]"><AlertTriangle size={13} className="mt-0.5 shrink-0" /><span>{provider.lastError.message ?? provider.lastError.code}{provider.failureCount ? ` · ${t("failureCount", { count: provider.failureCount })}` : ""}</span></div>}{provider.lastSuccessAtMs && <div className="mt-3 text-[10px] text-muted-foreground">{t("lastSuccess")}: {new Date(provider.lastSuccessAtMs).toLocaleString(language)}</div>}</div>)}</div> : <div className="rounded-lg border border-dashed border-border px-4 py-5 text-sm text-muted-foreground">{t("notReported")}</div>}</CardContent></Card>;
}

function ProcessEvidencePanel({ sample, items, loading, language }: { sample: SystemSample | null; items: AppResourceSample[]; loading: boolean; language: "en" | "zh-CN" }) {
  const { t } = useI18n();
  return <Card className="overflow-hidden"><CardHeader className="border-b border-border/70"><div className="flex flex-wrap items-center justify-between gap-3"><div><CardTitle>{t("processEvidence")}</CardTitle><p className="mt-1 text-xs font-normal text-muted-foreground">{t("processEvidenceScope")}</p></div><Badge className="border-border bg-card text-muted-foreground">{sample ? t("selectedTimestamp", { time: formatClock(sample.timestampMs, language) }) : t("noTimeSelected")}</Badge></div></CardHeader><CardContent className="px-0 pb-0">{!sample ? <div className="px-5 py-8 text-sm text-muted-foreground">{t("selectTime")}</div> : !sample.hasAppSnapshot ? <div className="flex items-start gap-3 px-5 py-8 text-sm text-muted-foreground"><CircleHelp size={17} className="mt-0.5 shrink-0" />{t("processCaptureUnavailable")}</div> : loading ? <div className="px-5 py-8 text-sm text-muted-foreground">{t("loadingProcessEvidence")}</div> : !items.length ? <div className="px-5 py-8 text-sm text-muted-foreground">{t("noProcessEvidence")}</div> : <div className="overflow-x-auto"><table className="w-full border-collapse text-sm"><caption className="sr-only">{t("processEvidence")}</caption><thead><tr><th scope="col" className="table-head pl-5">{t("processIdentity")}</th><th scope="col" className="table-head text-right">{t("metricCpu")}</th><th scope="col" className="table-head text-right">{t("metricMemory")}</th><th scope="col" className="table-head text-right">{t("metricDiskRead")}</th><th scope="col" className="table-head pr-5 text-right">{t("selectionReason")}</th></tr></thead><tbody>{items.map((item) => <tr key={`${item.processIdentityKey ?? item.appKey}-${item.pid ?? "none"}`} className="border-b border-border/60 last:border-b-0"><td className="table-cell pl-5"><div className="font-medium">{item.processName}</div><div className="mt-1 font-mono text-[10px] text-muted-foreground">{item.pid == null ? item.appKey : `PID ${item.pid}`}</div></td><td className="table-cell text-right font-mono">{nullableValue(item.measuredCpuPercent, "%", t)}</td><td className="table-cell text-right font-mono">{nullableBytes(item.measuredWorkingSetBytes, language, t)}</td><td className="table-cell text-right font-mono">{nullableRate(item.measuredReadBytesPerSec, language, t)}</td><td className="table-cell pr-5 text-right font-mono text-xs text-muted-foreground">0x{item.selectionReason.toString(16)}</td></tr>)}</tbody></table></div>}</CardContent></Card>;
}

function nullableValue(value: number | null, suffix: string, t: ReturnType<typeof useI18n>["t"]) {
  return value == null ? t("missingData") : `${value.toFixed(1)}${suffix}`;
}

function nullableBytes(value: number | null, language: "en" | "zh-CN", t: ReturnType<typeof useI18n>["t"]) {
  return value == null ? t("missingData") : formatBytes(value, language);
}

function nullableRate(value: number | null, language: "en" | "zh-CN", t: ReturnType<typeof useI18n>["t"]) {
  return value == null ? t("missingData") : `${formatBytes(value, language)}/s`;
}

function CapabilityBadge({ state, lifecycle, supported }: { state?: CapabilityState | "degraded"; lifecycle?: ProviderStatus["lifecycle"]; supported?: boolean }) {
  const { t } = useI18n();
  const label = state ? capabilityLabel(state, t) : lifecycle ? lifecycleLabel(lifecycle, t) : supported === false ? t("stateUnsupported") : t("stateEnabled");
  const failed = state === "failed" || lifecycle === "failed";
  const muted = state === "unsupported" || state === "supportedDisabled" || supported === false || lifecycle === "stopped";
  return <Badge className={failed ? "border-[hsl(var(--danger)/0.35)] bg-[hsl(var(--danger-surface))] text-[hsl(var(--danger))]" : muted ? "border-border bg-muted text-muted-foreground" : "border-[hsl(var(--success)/0.3)] bg-[hsl(var(--success-surface))] text-[hsl(var(--success))]"}>{label}</Badge>;
}

function capabilityLabel(state: CapabilityState | "degraded", t: ReturnType<typeof useI18n>["t"]) {
  if (state === "supportedDisabled") return t("disabledByUser");
  if (state === "unsupported") return t("stateUnsupported");
  if (state === "failed") return t("stateFailed");
  if (state === "degraded") return t("stateDegraded");
  return t("stateEnabled");
}

function lifecycleLabel(state: ProviderStatus["lifecycle"], t: ReturnType<typeof useI18n>["t"]) {
  if (state === "running") return t("providerRunning");
  if (state === "paused") return t("providerPaused");
  if (state === "failed") return t("providerFailed");
  return t("providerStopped");
}

function InlineError({ title, message, onRetry }: { title: string; message: string; onRetry: () => void }) {
  const { t } = useI18n();
  return <div role="alert" className="error-surface flex flex-wrap items-center justify-between gap-3 rounded-lg border px-4 py-3 text-sm"><div><div className="font-semibold">{title}</div><div className="mt-1 break-all text-xs opacity-80">{message}</div></div><Button variant="outline" onClick={onRetry}>{t("retry")}</Button></div>;
}

function EmptyState({ title, hint }: { title: string; hint: string }) {
  return <div className="empty-state"><MonitorCog size={22} className="text-muted-foreground" /><div className="font-semibold text-foreground">{title}</div><div className="max-w-md text-sm text-muted-foreground">{hint}</div></div>;
}

function TimelineLoading() {
  const { t } = useI18n();
  return <div className="space-y-3" aria-busy="true" aria-label={t("timelineLoading")}><div className="skeleton-line h-24 rounded-lg" /><div className="skeleton-line h-[430px] rounded-lg" /><div className="grid gap-3 md:grid-cols-2"><div className="skeleton-line h-52 rounded-lg" /><div className="skeleton-line h-52 rounded-lg" /></div></div>;
}
