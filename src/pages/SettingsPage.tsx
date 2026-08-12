import { useEffect, useMemo, useState } from "react";
import { Database, Gauge, Languages, Pause, Play, Save, Search, Trash2 } from "lucide-react";
import {
  clearCollectedData,
  getAutostartEnabled,
  getCollectionSettings,
  getCollectorStatus,
  listApps,
  setAppHidden,
  setAutostartEnabled,
  setCollectionPaused,
  setCollectionSettings
} from "../api/tauriApi";
import { Badge } from "../components/ui/Badge";
import { Button } from "../components/ui/Button";
import { Card, CardContent, CardHeader, CardTitle } from "../components/ui/Card";
import { Switch } from "../components/ui/Switch";
import { Table, Td, Th } from "../components/ui/Table";
import { useI18n } from "../i18n";
import type { AppIdentity, CollectionSettings, CollectorStatus } from "../types/resource";
import { formatBytes } from "../utils/time";

const selectClass = "h-9 rounded-lg border border-input bg-card px-3 text-sm text-foreground outline-none focus:ring-2 focus:ring-ring/35";

export function SettingsPage() {
  const { language, setLanguage, t } = useI18n();
  const [apps, setApps] = useState<AppIdentity[]>([]);
  const [status, setStatus] = useState<CollectorStatus | null>(null);
  const [settings, setSettings] = useState<CollectionSettings | null>(null);
  const [autostart, setAutostart] = useState(false);
  const [search, setSearch] = useState("");
  const [error, setError] = useState("");
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);

  const refresh = () => Promise.all([
    listApps(),
    getCollectorStatus(),
    getAutostartEnabled(),
    getCollectionSettings()
  ]).then(([nextApps, nextStatus, nextAutostart, nextSettings]) => {
    setApps(nextApps);
    setStatus(nextStatus);
    setAutostart(nextAutostart);
    setSettings(nextSettings);
  }).catch((reason) => setError(String(reason)));

  useEffect(() => {
    refresh();
    const id = window.setInterval(() => getCollectorStatus().then(setStatus).catch(() => undefined), 5000);
    return () => window.clearInterval(id);
  }, []);

  const filtered = useMemo(() => apps.filter((app) =>
    `${app.displayName} ${app.processName} ${app.exePath ?? ""}`.toLowerCase().includes(search.toLowerCase())
  ), [apps, search]);

  async function togglePause() {
    if (!status) return;
    await setCollectionPaused(!status.paused);
    await refresh();
  }

  async function toggleAutostart(value: boolean) {
    await setAutostartEnabled(value);
    setAutostart(value);
  }

  async function saveSettings() {
    if (!settings) return;
    setSaving(true);
    setSaved(false);
    setError("");
    try {
      await setCollectionSettings(settings);
      setSaved(true);
      window.setTimeout(() => setSaved(false), 2500);
    } catch (reason) {
      setError(String(reason));
    } finally {
      setSaving(false);
    }
  }

  async function toggleHidden(appId: number, hidden: boolean) {
    await setAppHidden(appId, hidden);
    setApps((items) => items.map((app) => app.id === appId ? { ...app, isHidden: hidden } : app));
  }

  async function clearData() {
    if (!window.confirm(t("clearConfirm"))) return;
    if (!window.confirm(t("clearFinalConfirm"))) return;
    await clearCollectedData();
    await refresh();
  }

  return <div className="space-y-5">
    <div>
      <h1 className="page-title text-[26px] font-semibold">{t("settingsTitle")}</h1>
      <p className="mt-1 text-sm text-muted-foreground">{t("settingsSubtitle")}</p>
    </div>

    {error && <div className="error-surface rounded-lg border p-4 text-sm">{error}</div>}

    <Card>
      <CardContent className="flex flex-wrap items-center justify-between gap-4 pt-5">
        <div className="flex items-start gap-3">
          <div className="flex h-9 w-9 items-center justify-center rounded-lg bg-muted text-muted-foreground"><Languages size={18} /></div>
          <div><div className="font-medium">{t("language")}</div><div className="text-sm text-muted-foreground">{t("languageDescription")}</div></div>
        </div>
        <select className={`${selectClass} min-w-44`} value={language} onChange={(event) => setLanguage(event.target.value as "en" | "zh-CN")} aria-label={t("language")}>
          <option value="en">{t("english")}</option>
          <option value="zh-CN">{t("simplifiedChinese")}</option>
        </select>
      </CardContent>
    </Card>

    <div className="grid gap-3 xl:grid-cols-2">
      <Card>
        <CardHeader className="border-b border-border/70"><CardTitle>{t("collector")}</CardTitle></CardHeader>
        <CardContent className="space-y-4">
          <div className="flex items-center justify-between gap-4">
            <div>
              <div className="font-medium">{t("backgroundCollection")}</div>
              <div className="text-sm text-muted-foreground">{t("lastHeartbeat")}: {status?.lastHeartbeatAtMs ? new Date(status.lastHeartbeatAtMs).toLocaleString(language) : t("waiting")}</div>
            </div>
            <Button onClick={togglePause}>{status?.paused ? <Play size={16} /> : <Pause size={16} />}{status?.paused ? t("resume") : t("pause")}</Button>
          </div>
          <div className="flex items-center justify-between gap-4 border-t border-border pt-4">
            <div className="min-w-0 flex-1">
              <div className="font-medium">{t("startWithWindows")}</div>
              <div className="text-sm text-muted-foreground">{t("autostartDescription")}</div>
            </div>
            <Switch checked={autostart} onCheckedChange={toggleAutostart} ariaLabel={t("startWithWindows")} />
          </div>
          <div className="rounded-lg bg-muted/70 px-3 py-2.5 text-xs leading-5 text-muted-foreground">{t("backgroundLifecycleDescription")}</div>
          <div className="text-xs text-muted-foreground">{t("droppedSamples", { count: status?.droppedSystemSamples ?? 0 })}</div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader className="border-b border-border/70"><CardTitle>{t("localData")}</CardTitle></CardHeader>
        <CardContent className="space-y-4 text-sm">
          <div className="flex gap-3"><Database size={18} className="mt-0.5 shrink-0" /><div className="min-w-0"><div className="font-medium">{formatBytes(status?.databaseSizeBytes ?? 0, language)}</div><div className="break-all text-muted-foreground">{status?.databasePath ?? t("unresolved")}</div></div></div>
          <p className="text-muted-foreground">{t("dataRetentionSummary")}</p>
          <Button variant="outline" className="text-[hsl(var(--danger))]" onClick={clearData}><Trash2 size={16} />{t("clearData")}</Button>
        </CardContent>
      </Card>
    </div>

    <Card>
      <CardHeader className="border-b border-border/70"><CardTitle>{t("collectionFrequency")}</CardTitle></CardHeader>
      <CardContent>
        {settings ? <div className="space-y-5">
          <div className="success-surface flex items-start gap-3 rounded-lg border px-4 py-3 text-sm"><Gauge size={18} className="mt-0.5 shrink-0" /><span>{t("lowFrequencyNotice")}</span></div>
          <div className="grid gap-5 md:grid-cols-2 xl:grid-cols-4">
            <SettingSelect label={t("foregroundCheck")} value={settings.foregroundPollIntervalMs} onChange={(value) => setSettings({ ...settings, foregroundPollIntervalMs: value })} options={[1, 2, 5, 10].map((count) => [count * 1000, t(count === 1 ? "everySecond" : "everySeconds", { count })])} />
            <SettingSelect label={t("systemResources")} value={settings.systemSampleIntervalMs} onChange={(value) => setSettings({ ...settings, systemSampleIntervalMs: value })} options={[5, 10, 30, 60].map((count) => [count * 1000, t("everySeconds", { count })])} />
            <SettingSelect label={t("idleThreshold")} value={settings.idleThresholdSeconds} onChange={(value) => setSettings({ ...settings, idleThresholdSeconds: value })} options={[1, 5, 10, 30, 60].map((count) => [count * 60, t(count === 1 ? "minute" : "minutes", { count })])} />
            <SettingSelect label={t("rawRetention")} value={settings.systemSampleRetentionDays} onChange={(value) => setSettings({ ...settings, systemSampleRetentionDays: value })} options={[1, 3, 7, 14, 30].map((count) => [count, t(count === 1 ? "day" : "days", { count })])} />
          </div>
          <div className="rounded-xl border border-border bg-muted/25 p-4">
            <div className="font-medium">{t("samplingTradeoffTitle")}</div>
            <p className="mt-1 text-sm text-muted-foreground">{t("samplingTradeoffIntro")}</p>
            <div className="mt-4 grid gap-3 md:grid-cols-2 xl:grid-cols-4">{[
              [5_000, "sampling5Title", "sampling5Description"],
              [10_000, "sampling10Title", "sampling10Description"],
              [30_000, "sampling30Title", "sampling30Description"],
              [60_000, "sampling60Title", "sampling60Description"]
            ].map(([interval, titleKey, descriptionKey]) => <div key={interval} className={`rounded-lg border p-3 ${settings.systemSampleIntervalMs === interval ? "border-primary/60 bg-accent ring-1 ring-primary/25" : "border-border bg-card"}`}>
              <div className="flex items-center justify-between gap-2"><strong className="text-sm">{t(titleKey as "sampling5Title")}</strong>{settings.systemSampleIntervalMs === interval && <Badge>{t("currentChoice")}</Badge>}</div>
              <p className="mt-2 text-xs leading-5 text-muted-foreground">{t(descriptionKey as "sampling5Description")}</p>
            </div>)}</div>
            <div className="mt-4 space-y-2 border-t border-border pt-3 text-xs leading-5 text-muted-foreground">
              <p>{t("samplingCpuPrinciple")}</p>
              <p>{t("samplingMemoryIoPrinciple")}</p>
              <p>{t("foregroundSamplingPrinciple")}</p>
            </div>
          </div>
          <div className="flex items-center gap-3"><Button onClick={saveSettings} disabled={saving}><Save size={16} />{saving ? t("saving") : t("saveSettings")}</Button>{saved && <span className="text-sm text-[hsl(var(--success))]">{t("settingsApplied")}</span>}</div>
        </div> : <div className="py-8 text-sm text-muted-foreground">{t("loadingLocalData")}</div>}
      </CardContent>
    </Card>

    <Card className="overflow-hidden">
      <CardHeader className="border-b border-border/70"><div className="flex items-center justify-between gap-4"><CardTitle>{t("recognizedApps")}</CardTitle><label className="flex w-72 items-center gap-2 rounded-lg border border-input bg-card px-3 focus-within:ring-2 focus-within:ring-ring/25"><Search size={16} className="text-muted-foreground" /><input className="h-9 min-w-0 flex-1 border-0 bg-transparent text-sm outline-none" value={search} onChange={(event) => setSearch(event.target.value)} placeholder={t("searchApps")} /></label></div></CardHeader>
      <CardContent className="px-0 pb-0">{filtered.length ? <div className="overflow-x-auto"><Table><thead><tr><Th className="pl-5">{t("app")}</Th><Th>{t("executablePath")}</Th><Th className="w-24">{t("status")}</Th><Th className="w-20 pr-5 text-right">{t("hide")}</Th></tr></thead><tbody>{filtered.map((app) => <tr key={app.id}><Td className="pl-5 font-medium">{app.displayName}</Td><Td className="max-w-[520px] truncate text-muted-foreground" title={app.exePath ?? ""}>{app.exePath || t("unresolved")}</Td><Td>{app.isHidden ? <Badge>{t("hidden")}</Badge> : <Badge className="border-border bg-card text-muted-foreground">{t("visible")}</Badge>}</Td><Td className="pr-5 text-right"><Switch checked={app.isHidden} onCheckedChange={(value) => toggleHidden(app.id, value)} ariaLabel={t("hideApp", { name: app.displayName })} /></Td></tr>)}</tbody></Table></div> : <div className="py-12 text-center text-sm text-muted-foreground">{t("noMatchingApps")}</div>}</CardContent>
    </Card>

    <p className="text-xs text-muted-foreground">{t("privacy")}</p>
  </div>;
}

function SettingSelect({ label, value, options, onChange }: { label: string; value: number; options: Array<[number, string]>; onChange: (value: number) => void }) {
  return <label className="space-y-2"><span className="block text-sm font-medium">{label}</span><select className={`${selectClass} w-full`} value={value} onChange={(event) => onChange(Number(event.target.value))}>{options.map(([optionValue, optionLabel]) => <option key={optionValue} value={optionValue}>{optionLabel}</option>)}</select></label>;
}
