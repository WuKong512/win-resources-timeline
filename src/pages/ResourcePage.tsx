import { useCallback, useEffect, useRef, useState } from "react";
import { Info, MousePointerClick } from "lucide-react";
import { getAppResourceSamples, getResourceAvailableDates, getSystemSamples } from "../api/tauriApi";
import { DateRangePicker } from "../components/DateRangePicker";
import { ResourceLineChart } from "../components/ResourceLineChart";
import { Card, CardContent, CardHeader, CardTitle } from "../components/ui/Card";
import { Table, Td, Th } from "../components/ui/Table";
import { useI18n } from "../i18n";
import { useUiStore } from "../stores/uiStore";
import type { AppResourceSample, SystemSample } from "../types/resource";
import { formatBytes, formatClock, localDateString, localDayRange } from "../utils/time";

export function ResourcePage() {
  const { language, t } = useI18n();
  const date = useUiStore((state) => state.selectedDate);
  const setDate = useUiStore((state) => state.setSelectedDate);
  const [availableDates, setAvailableDates] = useState<string[]>([]);
  const [datesLoading, setDatesLoading] = useState(true);
  const [samples, setSamples] = useState<SystemSample[]>([]);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(true);
  const [selected, setSelected] = useState<SystemSample | null>(null);
  const [appSamples, setAppSamples] = useState<AppResourceSample[]>([]);
  const [detailsLoading, setDetailsLoading] = useState(false);
  const [detailsError, setDetailsError] = useState("");
  const detailsRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    let cancelled = false;
    setDatesLoading(true);
    getResourceAvailableDates()
      .then((dates) => {
        if (cancelled) return;
        setAvailableDates(dates);
        if (dates.length && !dates.includes(date)) setDate(dates[dates.length - 1]);
      })
      .catch((e) => setError(String(e)))
      .finally(() => {
        if (!cancelled) setDatesLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    const range = localDayRange(date);
    setLoading(true);
    setError("");
    setSelected(null);
    setAppSamples([]);
    setDetailsError("");
    getSystemSamples(range.startMs, range.endMs)
      .then((data) => { if (!cancelled) setSamples(data); })
      .catch((e) => { if (!cancelled) setError(String(e)); })
      .finally(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, [date]);

  useEffect(() => {
    if (!selected) return;
    let cancelled = false;
    setDetailsLoading(true);
    setDetailsError("");
    setAppSamples([]);
    getAppResourceSamples(selected.timestampMs)
      .then((data) => {
        if (!cancelled) setAppSamples(data);
      })
      .catch((e) => {
        if (!cancelled) setDetailsError(String(e));
      })
      .finally(() => {
        if (!cancelled) setDetailsLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [selected]);

  useEffect(() => {
    if (selected) detailsRef.current?.scrollIntoView({
      behavior: window.matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth",
      block: "start"
    });
  }, [selected]);

  const selectSample = useCallback((sample: SystemSample) => setSelected(sample), []);

  return <div className="space-y-5">
    <div className="flex items-start justify-between gap-4">
      <div><h1 className="page-title text-[26px] font-semibold">{t("resourcesTitle")}</h1><p className="mt-1 text-sm text-muted-foreground">{t("resourcesSubtitle")}</p></div>
      <DateRangePicker value={date} onChange={setDate} availableDates={availableDates} loading={datesLoading} />
    </div>

    <div className="surface-shadow flex items-start gap-2.5 rounded-lg border border-border/80 bg-card px-4 py-3 text-xs leading-5 text-muted-foreground">
      <Info size={15} className="mt-0.5 shrink-0 text-primary" />
      <span>{t("resourcesWarning")}</span>
    </div>

    <Card className="overflow-hidden">
      <CardHeader className="flex-row items-center justify-between gap-3 border-b border-border/70">
        <CardTitle>{t("sampleCardTitle")}</CardTitle>
        {selected && <strong className={`rounded-full px-3 py-1 text-[11px] ${selected.hasAppSnapshot ? "bg-accent text-accent-foreground" : "warning-surface"}`}>{t(selected.hasAppSnapshot ? "selectedSampleWithDetails" : "selectedSampleWithoutDetails", { time: formatClock(selected.timestampMs, language) })}</strong>}
      </CardHeader>
      <CardContent className="pt-4">
        {error ? <StateMessage tone="error">{error}</StateMessage> : loading ? <StateMessage>{t("loadingSamples")}</StateMessage> : samples.length ? <>
          <div className="mb-2 flex items-center gap-2 text-xs text-muted-foreground"><MousePointerClick size={14} /><span>{t("clickSampleHint")}</span></div>
          <ResourceLineChart samples={samples} selectedTimestampMs={selected?.timestampMs ?? null} onSampleSelect={selectSample} />
        </> : <StateMessage spacious>{t("noSamplesForDate")}</StateMessage>}
      </CardContent>
    </Card>

    {selected && <div ref={detailsRef} className="scroll-mt-4">
      <Card className="overflow-hidden">
        <CardHeader className="border-b border-border/70"><CardTitle>{t("appSnapshotTitle", { time: formatClock(selected.timestampMs, language) })}</CardTitle></CardHeader>
        <CardContent className="px-0 pb-0">
          <div className="grid grid-cols-2 border-b border-border/70 md:grid-cols-4">
            <SampleMetric label="CPU" value={percent(selected.cpuPercent, t("noSample"))} tone="teal" />
            <SampleMetric label={t("memory")} value={percent(selected.memoryPercent, t("noSample"))} tone="blue" />
            <SampleMetric label={t("diskRead")} value={`${formatBytes(selected.diskReadBytesPerSec, language)}/s`} tone="violet" />
            <SampleMetric label={t("diskWrite")} value={`${formatBytes(selected.diskWriteBytesPerSec, language)}/s`} tone="amber" />
          </div>
          {detailsError ? <StateMessage tone="error">{detailsError}</StateMessage> : detailsLoading ? <StateMessage>{t("loadingAppSnapshot")}</StateMessage> : appSamples.length ? <>
            <div className="overflow-x-auto"><Table><thead><tr><Th className="pl-5">{t("app")}</Th><Th className="text-right">{t("processes")}</Th><Th className="text-right">CPU</Th><Th className="text-right">{t("memory")}</Th><Th className="text-right">{t("ioRead")}</Th><Th className="pr-5 text-right">{t("ioWrite")}</Th></tr></thead><tbody>{appSamples.map((app) => <tr key={app.appKey}><Td className="pl-5"><div className="font-medium">{app.processName}</div>{app.exePath && <div className="max-w-[420px] truncate text-xs text-muted-foreground" title={app.exePath}>{app.exePath}</div>}</Td><Td className="text-right">{app.processCount}</Td><Td className="text-right">{app.cpuPercent.toFixed(1)}%</Td><Td className="text-right">{formatBytes(app.memoryUsedBytes, language)}</Td><Td className="text-right">{formatBytes(app.ioReadBytesPerSec, language)}/s</Td><Td className="pr-5 text-right">{formatBytes(app.ioWriteBytesPerSec, language)}/s</Td></tr>)}</tbody></Table></div>
            <p className="px-5 py-3 text-xs text-muted-foreground">{t("appSnapshotScope")}</p>
          </> : <StateMessage spacious><div>{t("noAppSnapshot")}</div><div className="mt-1 text-xs">{t("noAppSnapshotHint")}</div></StateMessage>}
        </CardContent>
      </Card>
    </div>}
  </div>;
}

const toneDot = {
  teal: "bg-teal-500",
  blue: "bg-blue-500",
  violet: "bg-violet-500",
  amber: "bg-amber-500"
};

function SampleMetric({ label, value, tone }: { label: string; value: string; tone: keyof typeof toneDot }) {
  return <div className="border-r border-border/70 px-5 py-4 last:border-r-0">
    <div className="flex items-center gap-2 text-xs text-muted-foreground"><span className={`h-1.5 w-1.5 rounded-full ${toneDot[tone]}`} />{label}</div>
    <div className="metric-value mt-2 text-lg font-semibold">{value}</div>
  </div>;
}

function StateMessage({ children, tone = "default", spacious = false }: {
  children: React.ReactNode;
  tone?: "default" | "error";
  spacious?: boolean;
}) {
  return <div className={`${spacious ? "py-14" : "py-8"} text-center text-sm ${tone === "error" ? "text-[hsl(var(--danger))]" : "text-muted-foreground"}`}>{children}</div>;
}

function percent(value: number | null, fallback: string) {
  return value == null ? fallback : `${value.toFixed(1)}%`;
}
