import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Database, Languages, Pause, Play, Save, Search, ShieldCheck, Trash2 } from "lucide-react";
import {
  clearCollectedData,
  getAutostartEnabled,
  getCollectionSettings,
  getCollectorStatus,
  getStorageUsage,
  listApps,
  listCrashCases,
  setAppHidden,
  setAutostartEnabled,
  setCollectionPaused,
  setCollectionSettings
} from "../api/tauriApi";
import { Badge } from "../components/ui/Badge";
import { Button } from "../components/ui/Button";
import { Card, CardContent, CardHeader, CardTitle } from "../components/ui/Card";
import { Switch } from "../components/ui/Switch";
import { useI18n, type TranslationKey } from "../i18n";
import type { AppIdentity, CapabilityState, CollectionSettings, CollectorStatus, MetricCategory, ProviderStatus, StorageUsage } from "../types/resource";
import {
  beginSettingsFullRefresh,
  beginSettingsStatusPoll,
  canCommitSettingsFullOnly,
  canCommitSettingsStatusStorage,
  invalidateSettingsRefreshes,
  type SettingsFreshnessState
} from "../utils/settingsFreshness";
import { formatBytes } from "../utils/time";
import { aggregateCategoryCapability, toggleCategory } from "../utils/uiSemantics";

const selectClass = "h-9 rounded-lg border border-input bg-card px-3 text-sm text-foreground outline-none focus:ring-2 focus:ring-ring/35";
const categories: Array<{ id: MetricCategory; label: TranslationKey }> = [
  { id: "cpu", label: "categoryCpu" },
  { id: "gpu", label: "categoryGpu" },
  { id: "memory", label: "categoryMemory" },
  { id: "disk", label: "categoryDisk" },
  { id: "network", label: "categoryNetwork" },
  { id: "power", label: "categoryPower" },
  { id: "battery", label: "categoryBattery" },
  { id: "process", label: "categoryProcess" }
];

export function SettingsPage() {
  const { language, setLanguage, t } = useI18n();
  const [settings, setSettings] = useState<CollectionSettings | null>(null);
  const [status, setStatus] = useState<CollectorStatus | null>(null);
  const [storage, setStorage] = useState<StorageUsage | null>(null);
  const [providers, setProviders] = useState<ProviderStatus[]>([]);
  const [apps, setApps] = useState<AppIdentity[]>([]);
  const [activeHolds, setActiveHolds] = useState(0);
  const [autostart, setAutostart] = useState(false);
  const [search, setSearch] = useState("");
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState("");
  const mountedRef = useRef(false);
  // Full-only fields and status/storage have separate freshness domains.
  const freshnessRef = useRef<SettingsFreshnessState>({ fullGeneration: 0, statusStorageGeneration: 0 });
  const savedTimerRef = useRef<number | null>(null);

  const refresh = useCallback(async () => {
    const begun = beginSettingsFullRefresh(freshnessRef.current);
    freshnessRef.current = begun.state;
    const request = begun.request;
    let result: [CollectionSettings, CollectorStatus, StorageUsage, boolean, AppIdentity[], Awaited<ReturnType<typeof listCrashCases>>];
    try {
      result = await Promise.all([
        getCollectionSettings(),
        getCollectorStatus(),
        getStorageUsage(),
        getAutostartEnabled(),
        listApps(),
        listCrashCases()
      ]);
    } catch (error) {
      if (!canCommitSettingsFullOnly(freshnessRef.current, request, mountedRef.current)) return false;
      setLoading(false);
      throw error;
    }
    const commitFullOnly = canCommitSettingsFullOnly(freshnessRef.current, request, mountedRef.current);
    const commitStatusStorage = canCommitSettingsStatusStorage(freshnessRef.current, request, mountedRef.current);
    if (!commitFullOnly && !commitStatusStorage) return false;
    const [nextSettings, nextStatus, nextStorage, nextAutostart, nextApps, nextCases] = result;
    if (commitStatusStorage) {
      setStatus(nextStatus);
      setProviders(nextStatus.providerStatus);
      setStorage(nextStorage);
    }
    if (commitFullOnly) {
      setSettings(nextSettings);
      setAutostart(nextAutostart);
      setApps(nextApps);
      setActiveHolds(nextCases.filter((item) => item.hasActiveHold).length);
      setLoading(false);
    }
    return commitFullOnly;
  }, []);

  useEffect(() => {
    let cancelled = false;
    mountedRef.current = true;
    setLoading(true);
    refresh().catch(() => { if (!cancelled && mountedRef.current) setError(t("settingsErrorMessage")); });
    const interval = window.setInterval(() => {
      const begun = beginSettingsStatusPoll(freshnessRef.current);
      freshnessRef.current = begun.state;
      Promise.all([getCollectorStatus(), getStorageUsage()]).then(([nextStatus, nextStorage]) => {
        if (!cancelled && canCommitSettingsStatusStorage(freshnessRef.current, begun.request, mountedRef.current)) {
          setStatus(nextStatus);
          setProviders(nextStatus.providerStatus);
          setStorage(nextStorage);
        }
      }).catch(() => undefined);
    }, 5_000);
    return () => {
      cancelled = true;
      mountedRef.current = false;
      freshnessRef.current = invalidateSettingsRefreshes(freshnessRef.current);
      window.clearInterval(interval);
      if (savedTimerRef.current != null) {
        window.clearTimeout(savedTimerRef.current);
        savedTimerRef.current = null;
      }
    };
  }, [refresh]);

  const filteredApps = useMemo(() => apps.filter((app) => `${app.displayName} ${app.processName} ${app.exePath ?? ""}`.toLowerCase().includes(search.toLowerCase())), [apps, search]);

  async function savePlan() {
    if (!settings) return;
    setSaving(true);
    setSaved(false);
    setError("");
    try {
      await setCollectionSettings(settings);
      await refresh();
      if (!mountedRef.current) return;
      setSaved(true);
      if (savedTimerRef.current != null) window.clearTimeout(savedTimerRef.current);
      savedTimerRef.current = window.setTimeout(() => {
        savedTimerRef.current = null;
        if (mountedRef.current) setSaved(false);
      }, 2_500);
    } catch {
      if (mountedRef.current) setError(t("settingsActionError"));
    } finally {
      if (mountedRef.current) setSaving(false);
    }
  }

  async function togglePause() {
    if (!status) return;
    try { await setCollectionPaused(!status.paused); await refresh(); } catch { if (mountedRef.current) setError(t("settingsActionError")); }
  }

  async function toggleAutostart(value: boolean) {
    try { await setAutostartEnabled(value); if (mountedRef.current) setAutostart(value); } catch { if (mountedRef.current) setError(t("settingsActionError")); }
  }

  async function toggleHidden(appId: number, hidden: boolean) {
    try { await setAppHidden(appId, hidden); if (mountedRef.current) setApps((items) => items.map((app) => app.id === appId ? { ...app, isHidden: hidden } : app)); } catch { if (mountedRef.current) setError(t("settingsActionError")); }
  }

  async function clearData() {
    if (!window.confirm(t("clearConfirm"))) return;
    if (!window.confirm(t("clearFinalConfirm"))) return;
    try { await clearCollectedData(); await refresh(); } catch { if (mountedRef.current) setError(t("settingsActionError")); }
  }

  return <div className="space-y-5">
    <header><div className="eyebrow">{t("localData")}</div><h1 className="page-title mt-1 text-[28px] font-semibold tracking-[-0.02em]">{t("settingsTitle")}</h1><p className="mt-1 max-w-2xl text-sm text-muted-foreground">{t("settingsSubtitle")}</p></header>
    {error && <div role="alert" className="error-surface rounded-lg border px-4 py-3 text-sm">{error}</div>}
    {loading ? <div className="space-y-3" aria-busy="true" aria-label={t("collectionSettingsLoading")}><div className="skeleton-line h-24 rounded-lg" /><div className="skeleton-line h-72 rounded-lg" /><div className="skeleton-line h-64 rounded-lg" /></div> : <>
      <div className="grid gap-3 xl:grid-cols-[minmax(0,1.1fr)_minmax(330px,0.9fr)]"><Card><CardContent className="flex flex-wrap items-center justify-between gap-4 pt-5"><div className="flex items-start gap-3"><div className="flex h-9 w-9 items-center justify-center rounded-lg bg-muted text-muted-foreground"><Languages size={18} /></div><div><div className="font-medium">{t("language")}</div><div className="text-sm text-muted-foreground">{t("languageDescription")}</div></div></div><select className={`${selectClass} min-w-44`} value={language} onChange={(event) => setLanguage(event.target.value as "en" | "zh-CN")} aria-label={t("language")}><option value="en">{t("english")}</option><option value="zh-CN">{t("simplifiedChinese")}</option></select></CardContent></Card><CollectorCard status={status} autostart={autostart} onPause={togglePause} onAutostart={toggleAutostart} /></div>

      <Card className="overflow-hidden"><CardHeader className="border-b border-border/70"><div className="flex flex-wrap items-start justify-between gap-3"><div><CardTitle>{t("collectionCategories")}</CardTitle><p className="mt-1 text-xs font-normal text-muted-foreground">{t("collectionCategoriesDescription")}</p></div><Button onClick={savePlan} disabled={!settings || saving}><Save size={15} />{saving ? t("savingCollectionPlan") : t("saveCollectionPlan")}</Button></div></CardHeader><CardContent className="pt-4">{settings && <><div className="grid gap-2 sm:grid-cols-2 lg:grid-cols-4">{categories.map((category) => <CategoryControl key={category.id} category={category.id} label={t(category.label)} checked={settings.enabledCategories.includes(category.id)} state={categoryState(providers, settings, category.id)} disabled={categoryState(providers, settings, category.id) === "unsupported"} onChange={() => setSettings(toggleCategory(settings, category.id))} />)}</div><div className="mt-4 grid gap-4 border-t border-border/70 pt-4 md:grid-cols-4"><SettingSelect label={t("foregroundCheck")} value={settings.foregroundPollIntervalMs} onChange={(value) => setSettings({ ...settings, foregroundPollIntervalMs: value })} options={[1, 2, 5, 10].map((count) => [count * 1000, t(count === 1 ? "everySecond" : "everySeconds", { count })])} /><SettingSelect label={t("systemResources")} value={settings.systemSampleIntervalMs} onChange={(value) => setSettings({ ...settings, systemSampleIntervalMs: value })} options={[5, 10, 30, 60].map((count) => [count * 1000, t("everySeconds", { count })])} /><SettingSelect label={t("idleThreshold")} value={settings.idleThresholdSeconds} onChange={(value) => setSettings({ ...settings, idleThresholdSeconds: value })} options={[1, 5, 10, 30, 60].map((count) => [count * 60, t(count === 1 ? "minute" : "minutes", { count })])} /><SettingSelect label={t("rawRetention")} value={settings.systemSampleRetentionDays} onChange={(value) => setSettings({ ...settings, systemSampleRetentionDays: value })} options={[1, 3, 7, 14, 30].map((count) => [count, t(count === 1 ? "day" : "days", { count })])} /></div></>}</CardContent></Card>

      <div className="grid gap-3 xl:grid-cols-2"><ProviderControls providers={providers} settings={settings} onChange={setSettings} /><StorageCard settings={settings} storage={storage} activeHolds={activeHolds} /></div>

      <Card className="overflow-hidden"><CardHeader className="border-b border-border/70"><div className="flex flex-wrap items-start justify-between gap-4"><div><CardTitle>{t("recognizedApps")}</CardTitle><p className="mt-1 text-xs font-normal text-muted-foreground">{t("privacy")}</p></div><label className="flex w-72 items-center gap-2 rounded-lg border border-input bg-card px-3 focus-within:ring-2 focus-within:ring-ring/25"><Search size={16} className="text-muted-foreground" /><input className="h-9 min-w-0 flex-1 border-0 bg-transparent text-sm outline-none" value={search} onChange={(event) => setSearch(event.target.value)} placeholder={t("searchApps")} /></label></div></CardHeader><CardContent className="px-0 pb-0">{filteredApps.length ? <div className="overflow-x-auto"><table className="w-full min-w-[680px] border-collapse text-sm"><caption className="sr-only">{t("recognizedApps")}</caption><thead><tr><th scope="col" className="table-head pl-5">{t("app")}</th><th scope="col" className="table-head">{t("executablePath")}</th><th scope="col" className="table-head">{t("status")}</th><th scope="col" className="table-head pr-5 text-right">{t("hide")}</th></tr></thead><tbody>{filteredApps.map((app) => <tr key={app.id} className="border-b border-border/60 last:border-b-0"><td className="table-cell pl-5 font-medium">{app.displayName}</td><td className="table-cell max-w-[520px] truncate text-muted-foreground" title={app.exePath ?? ""}>{app.exePath || t("unresolved")}</td><td className="table-cell">{app.isHidden ? <Badge>{t("hidden")}</Badge> : <Badge className="border-border bg-card text-muted-foreground">{t("visible")}</Badge>}</td><td className="table-cell pr-5 text-right"><Switch checked={app.isHidden} onCheckedChange={(value) => toggleHidden(app.id, value)} ariaLabel={t("hideApp", { name: app.displayName })} /></td></tr>)}</tbody></table></div> : <div className="px-5 py-12 text-center text-sm text-muted-foreground">{t("noMatchingApps")}</div>}</CardContent></Card>

      <Card className="border-[hsl(var(--danger)/0.22)]"><CardContent className="flex flex-wrap items-center justify-between gap-4 pt-5"><div><div className="font-medium">{t("clearData")}</div><p className="mt-1 text-xs text-muted-foreground">{t("dataRetentionSummary")}</p></div><Button variant="outline" className="text-[hsl(var(--danger))]" onClick={clearData}><Trash2 size={15} />{t("clearData")}</Button></CardContent></Card>
      {saved && <div role="status" aria-live="polite" className="success-surface rounded-lg border px-4 py-3 text-sm">{t("collectionPlanSaved")}</div>}
    </>}
  </div>;
}

function CollectorCard({ status, autostart, onPause, onAutostart }: { status: CollectorStatus | null; autostart: boolean; onPause: () => void; onAutostart: (value: boolean) => void }) {
  const { t, language } = useI18n();
  return <Card><CardHeader className="border-b border-border/70"><CardTitle>{t("collector")}</CardTitle></CardHeader><CardContent className="space-y-4"><div className="flex items-center justify-between gap-4"><div><div className="font-medium">{t("backgroundCollection")}</div><div className="text-sm text-muted-foreground">{t("lastHeartbeat")}: {status?.lastHeartbeatAtMs ? new Date(status.lastHeartbeatAtMs).toLocaleString(language) : t("waiting")}</div></div><Button onClick={onPause}>{status?.paused ? <Play size={16} /> : <Pause size={16} />}{status?.paused ? t("resume") : t("pause")}</Button></div><div className="flex items-center justify-between gap-4 border-t border-border pt-4"><div className="min-w-0 flex-1"><div className="font-medium">{t("startWithWindows")}</div><div className="text-sm text-muted-foreground">{t("autostartDescription")}</div></div><Switch checked={autostart} onCheckedChange={onAutostart} ariaLabel={t("startWithWindows")} /></div><div className="rounded-lg bg-muted/70 px-3 py-2.5 text-xs leading-5 text-muted-foreground">{t("backgroundLifecycleDescription")}</div><div className="text-xs text-muted-foreground">{t("droppedSamples", { count: status?.droppedSystemSamples ?? 0 })}</div></CardContent></Card>;
}

function CategoryControl({ category, label, checked, state, disabled, onChange }: { category: MetricCategory; label: string; checked: boolean; state: CapabilityState | "notReported"; disabled: boolean; onChange: () => void }) {
  const { t } = useI18n();
  return <div className={`rounded-lg border px-3 py-3 ${disabled ? "border-dashed border-border/70 opacity-75" : "border-border/80 bg-muted/15"}`}><div className="flex items-center justify-between gap-3"><div className="min-w-0"><div className="truncate text-sm font-medium">{label}</div><div className="mt-1 text-[10px] text-muted-foreground">{state === "notReported" ? t("notReported") : capabilityLabel(state, t)}</div></div><Switch checked={checked} onCheckedChange={onChange} disabled={disabled} ariaLabel={label} /></div><div className="mt-2 font-mono text-[10px] text-muted-foreground">{category}</div></div>;
}

function ProviderControls({ providers, settings, onChange }: { providers: ProviderStatus[]; settings: CollectionSettings | null; onChange: (settings: CollectionSettings) => void }) {
  const { t } = useI18n();
  return <Card className="overflow-hidden"><CardHeader className="border-b border-border/70"><CardTitle>{t("providerControls")}</CardTitle><p className="text-xs font-normal text-muted-foreground">{t("providerControlsDescription")}</p></CardHeader><CardContent className="pt-4">{providers.length && settings ? <div className="space-y-3">{providers.map((provider) => { const disabled = settings.disabledProviders.includes(provider.providerId); return <div key={provider.providerId} className="rounded-lg border border-border/80 bg-muted/15 p-3"><div className="flex items-start justify-between gap-3"><div className="min-w-0"><div className="text-sm font-semibold">{provider.displayName}</div><div className="mt-1 font-mono text-[10px] text-muted-foreground">{provider.providerId}</div></div><Switch checked={!disabled && provider.supported} onCheckedChange={(enabled) => onChange({ ...settings, disabledProviders: enabled ? settings.disabledProviders.filter((id) => id !== provider.providerId) : [...new Set([...settings.disabledProviders, provider.providerId])] })} disabled={!provider.supported} ariaLabel={provider.displayName} /></div><div className="mt-3 flex flex-wrap gap-1.5">{provider.capabilities.map((capability) => <Badge key={`${provider.providerId}-${capability.category}`} className="border-border bg-card text-muted-foreground">{capability.category}: {capabilityLabel(capability.state, t)}</Badge>)}</div></div>; })}</div> : <div className="rounded-lg border border-dashed border-border px-4 py-6 text-sm text-muted-foreground">{t("notReported")}</div>}</CardContent></Card>;
}

function StorageCard({ settings, storage, activeHolds }: { settings: CollectionSettings | null; storage: StorageUsage | null; activeHolds: number }) {
  const { language, t } = useI18n();
  return <Card className="overflow-hidden"><CardHeader className="border-b border-border/70"><CardTitle>{t("storageAndRetention")}</CardTitle><p className="text-xs font-normal text-muted-foreground">{t("storageAndRetentionDescription")}</p></CardHeader><CardContent className="space-y-4 pt-4"><div className="grid grid-cols-2 gap-3"><StorageMetric label={t("storageTotal")} value={storage?.totalBytes ?? null} language={language} /><StorageMetric label={t("storageMain")} value={storage?.mainBytes ?? null} language={language} /><StorageMetric label={t("storageWal")} value={storage?.walBytes ?? null} language={language} /><StorageMetric label={t("storageShm")} value={storage?.shmBytes ?? null} language={language} /></div><div className="border-t border-border/70 pt-4 text-sm"><div className="flex items-center justify-between gap-3"><span className="text-muted-foreground">{t("systemRetention")}</span><strong>{settings ? `${settings.systemSampleRetentionDays} ${t(settings.systemSampleRetentionDays === 1 ? "day" : "days", { count: settings.systemSampleRetentionDays })}` : t("unresolved")}</strong></div><div className="mt-3 flex items-center justify-between gap-3"><span className="flex items-center gap-2 text-muted-foreground"><ShieldCheck size={15} />{t("activeCrashHolds")}</span><strong>{activeHolds}</strong></div><p className="mt-3 text-xs leading-5 text-muted-foreground">{t("categoryEstimateUnavailable")}</p></div></CardContent></Card>;
}

function StorageMetric({ label, value, language }: { label: string; value: number | null; language: "en" | "zh-CN" }) {
  const { t } = useI18n();
  return <div className="rounded-lg border border-border/80 bg-muted/15 px-3 py-2.5"><div className="text-[10px] uppercase tracking-[0.08em] text-muted-foreground">{label}</div><div className="mt-1 font-mono text-sm font-semibold">{value == null ? t("missingData") : formatBytes(value, language)}</div></div>;
}

function SettingSelect({ label, value, options, onChange }: { label: string; value: number; options: Array<[number, string]>; onChange: (value: number) => void }) {
  return <label className="space-y-2"><span className="block text-sm font-medium">{label}</span><select className={`${selectClass} w-full`} value={value} onChange={(event) => onChange(Number(event.target.value))}>{options.map(([optionValue, optionLabel]) => <option key={optionValue} value={optionValue}>{optionLabel}</option>)}</select></label>;
}

function categoryState(providers: ProviderStatus[], settings: CollectionSettings, category: MetricCategory): CapabilityState | "notReported" {
  return aggregateCategoryCapability(providers, settings, category) ?? "notReported";
}

function capabilityLabel(state: CapabilityState, t: ReturnType<typeof useI18n>["t"]) {
  if (state === "supportedDisabled") return t("disabledByUser");
  if (state === "unsupported") return t("capabilityUnsupported");
  if (state === "failed") return t("capabilityFailed");
  return t("enabledByUser");
}
