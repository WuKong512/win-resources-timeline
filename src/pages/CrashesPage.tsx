import { useCallback, useEffect, useMemo, useState } from "react";
import { AlertCircle, CheckCircle2, CircleHelp, Clock3, FileSearch, ShieldCheck } from "lucide-react";
import { getCrashCaseDetail, getCrashDetectorStatus, getSystemTimeline, listCrashCases } from "../api/tauriApi";
import { ResourceTimelineChart } from "../components/ResourceTimelineChart";
import { Badge } from "../components/ui/Badge";
import { Button } from "../components/ui/Button";
import { Card, CardContent, CardHeader, CardTitle } from "../components/ui/Card";
import { useI18n } from "../i18n";
import type { CrashCaseSummary, CrashDetectorStatus, CrashEvidenceDetail, CrashEvidenceMetric, CrashEvidenceProcessEntry, CrashEvidenceWindow, CrashSystemEvent, SystemSample, TimelineQueryResult } from "../types/resource";
import { formatBytes, formatClock } from "../utils/time";
import { evidenceStatusTone } from "../utils/uiSemantics";

const windows: CrashEvidenceWindow[] = ["pre_1m", "pre_5m", "pre_30m", "post_5m"];

export function CrashesPage() {
  const { language, t } = useI18n();
  const [cases, setCases] = useState<CrashCaseSummary[]>([]);
  const [detector, setDetector] = useState<CrashDetectorStatus | null>(null);
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [detail, setDetail] = useState<CrashEvidenceDetail | null>(null);
  const [curveTimeline, setCurveTimeline] = useState<TimelineQueryResult | null>(null);
  const [selectedWindow, setSelectedWindow] = useState<CrashEvidenceWindow>("pre_30m");
  const [loading, setLoading] = useState(true);
  const [detailLoading, setDetailLoading] = useState(false);
  const [curveLoading, setCurveLoading] = useState(false);
  const [detailReload, setDetailReload] = useState(0);
  const [error, setError] = useState("");
  const [detailError, setDetailError] = useState("");

  const loadCases = useCallback(() => {
    let cancelled = false;
    setLoading(true);
    setError("");
    Promise.all([listCrashCases(), getCrashDetectorStatus()])
      .then(([nextCases, nextDetector]) => {
        if (cancelled) return;
        setCases(nextCases);
        setDetector(nextDetector);
        setSelectedId((current) => current && nextCases.some((item) => item.id === current) ? current : nextCases[0]?.id ?? null);
      })
      .catch(() => { if (!cancelled) setError(t("crashCasesErrorMessage")); })
      .finally(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, [t]);

  useEffect(() => loadCases(), [loadCases]);

  useEffect(() => {
    if (selectedId == null) {
      setDetail(null);
      setCurveTimeline(null);
      return;
    }
    let cancelled = false;
    const selectedCase = cases.find((item) => item.id === selectedId);
    setDetail(null);
    setCurveTimeline(null);
    setDetailLoading(true);
    setCurveLoading(true);
    setDetailError("");
    Promise.all([
      getCrashCaseDetail(selectedId),
      selectedCase ? getSystemTimeline(selectedCase.windowStartMs, selectedCase.windowEndMs, 1_000) : Promise.resolve(null)
    ])
      .then(([nextDetail, nextCurves]) => {
        if (cancelled) return;
        setDetail(nextDetail);
        setCurveTimeline(nextCurves);
      })
      .catch(() => {
        if (!cancelled) setDetailError(t("crashDetailErrorMessage"));
      })
      .finally(() => {
        if (cancelled) return;
        setDetailLoading(false);
        setCurveLoading(false);
      });
    return () => { cancelled = true; };
  }, [cases, detailReload, selectedId, t]);

  const selectedCase = detail?.case ?? cases.find((item) => item.id === selectedId) ?? null;
  const visibleMetrics = useMemo(() => (detail?.metrics ?? []).filter((metric) => metric.window === selectedWindow), [detail?.metrics, selectedWindow]);
  const onCurveSelect = useCallback(() => undefined, []);

  return <div className="space-y-5">
    <header className="flex flex-wrap items-end justify-between gap-4"><div><div className="eyebrow">{t("crashCaseList")}</div><h1 className="page-title mt-1 text-[28px] font-semibold tracking-[-0.02em]">{t("crashesTitle")}</h1><p className="mt-1 max-w-2xl text-sm text-muted-foreground">{t("crashesSubtitle")}</p></div><DetectorStatus status={detector} /></header>
    {error ? <InlineError title={t("crashCasesError")} message={error} onRetry={loadCases} /> : loading ? <CrashLoading /> : <>
      <div className="grid gap-3 xl:grid-cols-[310px_minmax(0,1fr)]">
        <Card className="overflow-hidden"><CardHeader className="border-b border-border/70"><div className="flex items-center justify-between gap-3"><CardTitle>{t("crashCaseList")}</CardTitle><Badge className="border-border bg-muted text-muted-foreground">{cases.length}</Badge></div></CardHeader><CardContent className="px-2 pb-2 pt-2">{cases.length ? <div className="space-y-1">{cases.map((item) => <CaseListItem key={item.id} item={item} selected={item.id === selectedId} onClick={() => { setSelectedId(item.id); setSelectedWindow("pre_30m"); }} />)}</div> : <div className="px-3 py-10 text-center"><CircleHelp size={22} className="mx-auto text-muted-foreground" /><div className="mt-3 text-sm font-semibold">{t("noCrashCases")}</div><p className="mt-1 text-xs leading-5 text-muted-foreground">{t("noCrashCasesHint")}</p></div>}</CardContent></Card>
        <CrashDetail detail={detail} selectedCase={selectedCase} detailLoading={detailLoading} detailError={detailError} onRetry={() => setDetailReload((value) => value + 1)} selectedWindow={selectedWindow} setSelectedWindow={setSelectedWindow} curveTimeline={curveTimeline} curveLoading={curveLoading} onCurveSelect={onCurveSelect} visibleMetrics={visibleMetrics} language={language} />
      </div>
    </>}
  </div>;
}

function CaseListItem({ item, selected, onClick }: { item: CrashCaseSummary; selected: boolean; onClick: () => void }) {
  const { language, t } = useI18n();
  return <button type="button" onClick={onClick} className={`w-full rounded-lg border px-3 py-3 text-left transition-[background-color,border-color,transform] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/35 ${selected ? "border-primary/45 bg-accent" : "border-transparent hover:border-border hover:bg-muted/55"}`} aria-pressed={selected}><div className="flex items-start justify-between gap-3"><div className="min-w-0"><div className="truncate text-sm font-semibold">{classificationLabel(item.classification, t)}</div><div className="mt-1 font-mono text-[11px] text-muted-foreground">{new Date(item.anchorTimeMs).toLocaleString(language)}</div></div><EvidenceBadge status={item.evidenceStatus} /></div><div className="mt-2 flex items-center gap-2 text-[10px] text-muted-foreground">{item.hasActiveHold && <><ShieldCheck size={12} />{t("activeHold")}</>}<span className="ml-auto">{t("sampleCount")}: {item.summaryCount}</span></div></button>;
}

function CrashDetail({ detail, selectedCase, detailLoading, detailError, onRetry, selectedWindow, setSelectedWindow, curveTimeline, curveLoading, onCurveSelect, visibleMetrics, language }: { detail: CrashEvidenceDetail | null; selectedCase: CrashCaseSummary | null; detailLoading: boolean; detailError: string; onRetry: () => void; selectedWindow: CrashEvidenceWindow; setSelectedWindow: (window: CrashEvidenceWindow) => void; curveTimeline: TimelineQueryResult | null; curveLoading: boolean; onCurveSelect: (sample: SystemSample) => void; visibleMetrics: CrashEvidenceMetric[]; language: "en" | "zh-CN" }) {
  const { t } = useI18n();
  if (!selectedCase) return <Card className="empty-state min-h-[360px]"><FileSearch size={24} className="text-muted-foreground" /><div className="font-semibold text-foreground">{t("selectCrashCase")}</div></Card>;
  return <div className="space-y-3">{detailError && <InlineError title={t("crashDetailError")} message={detailError} onRetry={onRetry} />}{detailLoading || !detail ? <Card className="min-h-[360px]"><div className="flex h-full min-h-[360px] items-center justify-center text-sm text-muted-foreground">{t("crashDetailLoading")}</div></Card> : <>
    <Card className="overflow-hidden"><CardHeader className="border-b border-border/70"><div className="flex flex-wrap items-start justify-between gap-3"><div><div className="eyebrow">{t("incidentTime")}</div><CardTitle className="mt-1 text-lg">{new Date(detail.case.anchorTimeMs).toLocaleString(language)}</CardTitle><p className="mt-1 text-xs text-muted-foreground">{classificationLabel(detail.case.classification, t)}</p></div><div className="flex flex-wrap items-center justify-end gap-2"><EvidenceBadge status={detail.case.evidenceStatus} /><Badge className={detail.case.hasActiveHold ? "border-[hsl(var(--success)/0.3)] bg-[hsl(var(--success-surface))] text-[hsl(var(--success))]" : "border-border bg-muted text-muted-foreground"}>{detail.case.hasActiveHold ? t("activeHold") : t("noActiveHold")}</Badge></div></div></CardHeader><CardContent className="pt-4"><div className="grid gap-3 text-sm sm:grid-cols-3"><Fact label={t("classification")} value={classificationLabel(detail.case.classification, t)} /><Fact label={t("evidenceStatus")} value={evidenceStatusLabel(detail.case.evidenceStatus, t)} /><Fact label={t("processingVersion")} value={detail.case.processingVersion} mono /></div>{detail.case.hasActiveHold && <div className="success-surface mt-4 flex items-start gap-2 rounded-lg border px-3 py-2.5 text-xs"><ShieldCheck size={14} className="mt-0.5 shrink-0" />{t("holdProtected")}</div>}<p className="mt-4 border-t border-border/70 pt-3 text-xs leading-5 text-muted-foreground">{t("evidenceNotice")}</p></CardContent></Card>
    <Card className="overflow-hidden"><CardHeader className="border-b border-border/70"><div className="flex flex-wrap items-center justify-between gap-3"><CardTitle>{t("resourceCurves")}</CardTitle><span className="text-xs text-muted-foreground">{t("crashCurveHint")}</span></div></CardHeader><CardContent className="pt-4">{curveLoading ? <div className="skeleton-line h-[390px] rounded-lg" aria-busy="true" /> : curveTimeline?.samples.length ? <ResourceTimelineChart samples={curveTimeline.samples} gaps={curveTimeline.gaps} startMs={curveTimeline.startMs} endMs={curveTimeline.endMs} selectedTimestampMs={null} onSampleSelect={onCurveSelect} ariaLabel={t("resourceCurves")} /> : <div className="empty-state min-h-[220px]">{detail.case.evidenceStatus === "pending" || detail.case.evidenceStatus === "post_pending" ? <><Clock3 size={22} className="text-muted-foreground" /><div className="font-semibold text-foreground">{t("evidenceUnavailable")}</div></> : <><CircleHelp size={22} className="text-muted-foreground" /><div className="font-semibold text-foreground">{t("noResourceCurves")}</div></>}</div>}</CardContent></Card>
    <Card className="overflow-hidden"><CardHeader className="border-b border-border/70"><CardTitle>{t("evidenceWindows")}</CardTitle></CardHeader><CardContent className="pt-4"><div className="segmented-control w-full overflow-x-auto">{windows.map((window) => <button key={window} type="button" className={`segmented-control-item min-w-max ${selectedWindow === window ? "segmented-control-active" : ""}`} onClick={() => setSelectedWindow(window)}>{windowLabel(window, t)}</button>)}</div><div className="mt-4">{visibleMetrics.length ? <EvidenceMetricTable metrics={visibleMetrics} language={language} /> : <div className="rounded-lg border border-dashed border-border px-4 py-8 text-center text-sm text-muted-foreground">{detail.case.evidenceStatus === "pending" || detail.case.evidenceStatus === "post_pending" ? t("evidenceUnavailable") : t("noDataForWindow")}</div>}</div></CardContent></Card>
    <div className="grid gap-3 xl:grid-cols-2"><EventTable events={detail.events} language={language} /><ProcessTable processes={detail.processes.filter((item) => item.window === selectedWindow)} language={language} /></div>
  </>}</div>;
}

function EvidenceMetricTable({ metrics, language }: { metrics: CrashEvidenceMetric[]; language: "en" | "zh-CN" }) {
  const { t } = useI18n();
  return <div className="overflow-x-auto"><table className="w-full min-w-[720px] border-collapse text-sm"><caption className="sr-only">{t("evidenceSummary")}</caption><thead><tr><th scope="col" className="table-head pl-4">{t("evidenceMetric")}</th><th scope="col" className="table-head text-right">{t("average")}</th><th scope="col" className="table-head text-right">{t("minimum")}</th><th scope="col" className="table-head text-right">{t("maximum")}</th><th scope="col" className="table-head text-right">{t("delta")}</th><th scope="col" className="table-head text-right">{t("windowCoverage")}</th><th scope="col" className="table-head pr-4 text-right">{t("sampleCount")}</th></tr></thead><tbody>{metrics.map((metric) => <tr key={metric.metricKey} className="border-b border-border/60 last:border-b-0"><td className="table-cell pl-4"><div className="font-medium">{metric.metric}</div>{metric.deviceKey && <div className="mt-1 font-mono text-[10px] text-muted-foreground">{metric.deviceKey}</div>}</td><td className="table-cell text-right font-mono">{metricNumber(metric.avg)}</td><td className="table-cell text-right font-mono">{metricNumber(metric.min)}</td><td className="table-cell text-right font-mono">{metricNumber(metric.max)}</td><td className="table-cell text-right font-mono">{metricNumber(metric.delta)}</td><td className="table-cell text-right font-mono">{Math.round(metric.coverage * 100)}%</td><td className="table-cell pr-4 text-right font-mono">{metric.sampleCount}</td></tr>)}</tbody></table></div>;
}

function EventTable({ events, language }: { events: CrashSystemEvent[]; language: "en" | "zh-CN" }) {
  const { t } = useI18n();
  return <Card className="overflow-hidden"><CardHeader className="border-b border-border/70"><CardTitle>{t("objectiveEvents")}</CardTitle></CardHeader><CardContent className="px-0 pb-0">{events.length ? <div className="overflow-x-auto"><table className="w-full min-w-[560px] border-collapse text-sm"><caption className="sr-only">{t("objectiveEvents")}</caption><thead><tr><th scope="col" className="table-head pl-4">{t("systemSample")}</th><th scope="col" className="table-head">{t("eventProvider")}</th><th scope="col" className="table-head">{t("eventId")}</th><th scope="col" className="table-head pr-4 text-right">{t("recordId")}</th></tr></thead><tbody>{events.map((event) => <tr key={event.id} className="border-b border-border/60 last:border-b-0"><td className="table-cell pl-4"><div className="font-medium">{event.kind}</div><div className="mt-1 font-mono text-[10px] text-muted-foreground">{formatClock(event.eventTimeMs, language)}</div></td><td className="table-cell text-xs text-muted-foreground">{event.provider ?? event.channel}</td><td className="table-cell font-mono text-xs">{event.eventId}</td><td className="table-cell pr-4 text-right font-mono text-xs text-muted-foreground">{event.recordId}</td></tr>)}</tbody></table></div> : <div className="px-4 py-8 text-sm text-muted-foreground">{t("noObjectiveEvents")}</div>}</CardContent></Card>;
}

function ProcessTable({ processes, language }: { processes: CrashEvidenceProcessEntry[]; language: "en" | "zh-CN" }) {
  const { t } = useI18n();
  return <Card className="overflow-hidden"><CardHeader className="border-b border-border/70"><CardTitle>{t("processEvidenceTitle")}</CardTitle></CardHeader><CardContent className="px-0 pb-0">{processes.length ? <div className="overflow-x-auto"><table className="w-full min-w-[560px] border-collapse text-sm"><caption className="sr-only">{t("processEvidenceTitle")}</caption><thead><tr><th scope="col" className="table-head pl-4">{t("processIdentity")}</th><th scope="col" className="table-head text-right">{t("average")}</th><th scope="col" className="table-head text-right">{t("maximum")}</th><th scope="col" className="table-head text-right">{t("metricMemory")}</th><th scope="col" className="table-head pr-4 text-right">{t("windowCoverage")}</th></tr></thead><tbody>{processes.map((process) => <tr key={`${process.processIdentityKey}-${process.window}`} className="border-b border-border/60 last:border-b-0"><td className="table-cell pl-4"><div className="font-medium">{process.processName}</div><div className="mt-1 font-mono text-[10px] text-muted-foreground">{process.pid == null ? process.processIdentityKey : `PID ${process.pid}`}</div></td><td className="table-cell text-right font-mono">{metricNumber(process.cpuAvgPercent, "%")}</td><td className="table-cell text-right font-mono">{metricNumber(process.cpuPeakPercent, "%")}</td><td className="table-cell text-right font-mono">{process.memoryPeakBytes == null ? "—" : formatBytes(process.memoryPeakBytes, language)}</td><td className="table-cell pr-4 text-right font-mono">{Math.round(process.coverage * 100)}%</td></tr>)}</tbody></table></div> : <div className="px-4 py-8 text-sm text-muted-foreground">{t("noCrashProcessEvidence")}</div>}</CardContent></Card>;
}

function DetectorStatus({ status }: { status: CrashDetectorStatus | null }) {
  const { t } = useI18n();
  const value = status?.state === "ready" ? t("detectorReady") : status?.state === "scanning" ? t("detectorScanning") : status?.state === "permission_denied" ? t("detectorPermissionDenied") : status?.state === "failed" ? t("detectorFailed") : t("detectorIdle");
  const failed = status?.state === "failed" || status?.state === "permission_denied";
  return <div className={`flex items-center gap-2 rounded-full border px-3 py-1.5 text-xs ${failed ? "error-surface" : "border-border bg-card text-muted-foreground"}`}><span className={`h-2 w-2 rounded-full ${failed ? "bg-[hsl(var(--danger))]" : status?.state === "ready" ? "bg-[hsl(var(--success))]" : "bg-[hsl(var(--warning))]"}`} />{t("detector")}: <strong>{value}</strong></div>;
}

function EvidenceBadge({ status }: { status: string }) {
  const { t } = useI18n();
  const tone = evidenceStatusTone(status);
  const label = evidenceStatusLabel(status, t);
  const Icon = tone === "complete" ? CheckCircle2 : tone === "failed" ? AlertCircle : tone === "partial" ? FileSearch : Clock3;
  return <span className={`inline-flex items-center gap-1.5 rounded-full border px-2 py-1 text-[10px] font-semibold ${tone === "complete" ? "border-[hsl(var(--success)/0.3)] bg-[hsl(var(--success-surface))] text-[hsl(var(--success))]" : tone === "failed" ? "border-[hsl(var(--danger)/0.35)] bg-[hsl(var(--danger-surface))] text-[hsl(var(--danger))]" : tone === "partial" ? "border-[hsl(var(--warning)/0.35)] bg-[hsl(var(--warning-surface))] text-[hsl(var(--warning))]" : "border-border bg-muted text-muted-foreground"}`}><Icon size={11} />{label}</span>;
}

function Fact({ label, value, mono = false }: { label: string; value: string; mono?: boolean }) {
  return <div className="rounded-lg border border-border/80 bg-muted/15 px-3 py-2.5"><div className="text-[10px] uppercase tracking-[0.08em] text-muted-foreground">{label}</div><div className={`mt-1 text-sm font-medium ${mono ? "font-mono text-xs" : ""}`}>{value}</div></div>;
}

function classificationLabel(classification: string, t: ReturnType<typeof useI18n>["t"]) {
  if (classification === "bsod") return t("classificationBsod");
  if (classification === "unexpected_shutdown") return t("classificationUnexpectedShutdown");
  if (classification === "abnormal_restart") return t("classificationAbnormalRestart");
  return classification;
}

function evidenceStatusLabel(status: string, t: ReturnType<typeof useI18n>["t"]) {
  if (status === "pending") return t("evidencePending");
  if (status === "post_pending") return t("evidencePostPending");
  if (status === "partial") return t("evidencePartial");
  if (status === "complete") return t("evidenceComplete");
  if (status === "failed") return t("evidenceFailed");
  return status;
}

function windowLabel(window: CrashEvidenceWindow, t: ReturnType<typeof useI18n>["t"]) {
  if (window === "pre_1m") return t("evidenceWindowPre1m");
  if (window === "pre_5m") return t("evidenceWindowPre5m");
  if (window === "post_5m") return t("evidenceWindowPost5m");
  return t("evidenceWindowPre30m");
}

function metricNumber(value: number | null, suffix = "") {
  return value == null ? "—" : `${value.toFixed(2)}${suffix}`;
}

function InlineError({ title, message, onRetry }: { title: string; message: string; onRetry: () => void }) {
  const { t } = useI18n();
  return <div role="alert" className="error-surface flex flex-wrap items-center justify-between gap-3 rounded-lg border px-4 py-3 text-sm"><div><div className="font-semibold">{title}</div><div className="mt-1 break-all text-xs opacity-80">{message}</div></div><Button variant="outline" onClick={onRetry}>{t("retry")}</Button></div>;
}

function CrashLoading() {
  const { t } = useI18n();
  return <div className="grid gap-3 xl:grid-cols-[310px_minmax(0,1fr)]" aria-busy="true" aria-label={t("crashDetailLoading")}><div className="skeleton-line h-[520px] rounded-lg" /><div className="space-y-3"><div className="skeleton-line h-44 rounded-lg" /><div className="skeleton-line h-[430px] rounded-lg" /></div></div>;
}
