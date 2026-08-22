import { useMemo, useState } from "react";
import { useI18n } from "../i18n";
import {
  canAddMetricToCard,
  MAX_METRICS_PER_CARD,
  reorderDashboardCards,
  validateDashboardConfig,
  type DashboardCardConfig,
  type DashboardConfig
} from "../dashboard/config";
import { getAvailableMetricDescriptors, getMetricDescriptor, hasMetricData, metricDisplayName, type MetricDescriptor, type MetricId } from "../dashboard/metrics";
import type { SystemSample } from "../types/resource";
import { Badge } from "./ui/Badge";
import { Button } from "./ui/Button";
import { Card, CardContent, CardHeader, CardTitle } from "./ui/Card";

type DashboardEditorProps = {
  config: DashboardConfig;
  samples: SystemSample[];
  onChange: (config: DashboardConfig) => void;
  onRestoreDefaults: () => void;
  saving: boolean;
  saveError: string;
};

export function DashboardEditor({ config, samples, onChange, onRestoreDefaults, saving, saveError }: DashboardEditorProps) {
  const { t } = useI18n();
  const availableDescriptors = useMemo(() => getAvailableMetricDescriptors(samples), [samples]);
  const [newMetricId, setNewMetricId] = useState<MetricId | "">("");
  const sortedCards = [...config.cards].sort((left, right) => left.order - right.order);

  function update(next: DashboardConfig) {
    const validation = validateDashboardConfig(next);
    if (validation.ok) onChange(validation.config);
  }

  return <Card className="border-[hsl(var(--signal-cyan)/0.35)] bg-card/95">
    <CardHeader className="border-b border-border/70">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <CardTitle>{t("dashboardCustomize")}</CardTitle>
          <p className="mt-1 max-w-2xl text-xs font-normal text-muted-foreground">{t("dashboardCustomizeDescription")}</p>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          {saving && <span className="text-xs text-muted-foreground">{t("dashboardSaving")}</span>}
          {!saving && !saveError && <span className="text-xs text-muted-foreground">{t("dashboardSaved")}</span>}
          {saveError && <span role="alert" className="text-xs text-[hsl(var(--danger))]">{saveError}</span>}
          <Button type="button" variant="outline" className="h-8 px-2.5 text-xs" onClick={onRestoreDefaults}>{t("dashboardRestoreDefaults")}</Button>
        </div>
      </div>
    </CardHeader>
    <CardContent className="space-y-3 pt-4">
      {!sortedCards.length && <div className="rounded-lg border border-dashed border-border px-4 py-5 text-sm text-muted-foreground">{t("dashboardNoCards")}</div>}
      {sortedCards.map((card, index) => <DashboardEditorCard
        key={card.id}
        card={card}
        index={index}
        cardCount={sortedCards.length}
        samples={samples}
        availableDescriptors={availableDescriptors}
        onChange={(nextCard) => update({ ...config, cards: config.cards.map((item) => item.id === card.id ? nextCard : item) })}
        onMove={(direction) => update(reorderDashboardCards(config, card.id, direction))}
        onRemove={() => update({ ...config, cards: sortedCards.filter((item) => item.id !== card.id).map((item, nextOrder) => ({ ...item, order: nextOrder })) })}
      />)}
      <div className="flex flex-wrap items-center gap-2 border-t border-border/70 pt-3">
        <label className="text-xs text-muted-foreground" htmlFor="new-dashboard-metric">{t("dashboardSelectMetric")}</label>
        <select id="new-dashboard-metric" value={newMetricId} className="h-8 max-w-full rounded-md border border-input bg-card px-2 text-xs text-foreground" onChange={(event) => setNewMetricId(event.target.value as MetricId | "")}>
          <option value="">{t("dashboardSelectMetric")}</option>
          {availableDescriptors.map((descriptor) => <option key={descriptor.id} value={descriptor.id}>{metricDisplayName(descriptor, t, samples)}</option>)}
        </select>
        <Button type="button" variant="outline" className="h-8 px-2.5 text-xs" disabled={!newMetricId || config.cards.length >= 12} onClick={() => {
          if (!newMetricId) return;
          const id = `dashboard-card-${Date.now()}`;
          update({ ...config, cards: [...config.cards, { id, metricIds: [newMetricId], hiddenMetricIds: [], order: config.cards.length, visible: true }] });
          setNewMetricId("");
        }}>{t("dashboardAddCard")}</Button>
        {!availableDescriptors.length && <span className="text-xs text-muted-foreground">{t("dashboardNoMetricsAvailable")}</span>}
      </div>
    </CardContent>
  </Card>;
}

function DashboardEditorCard({
  card,
  index,
  cardCount,
  samples,
  availableDescriptors,
  onChange,
  onMove,
  onRemove
}: {
  card: DashboardCardConfig;
  index: number;
  cardCount: number;
  samples: SystemSample[];
  availableDescriptors: MetricDescriptor[];
  onChange: (card: DashboardCardConfig) => void;
  onMove: (direction: -1 | 1) => void;
  onRemove: () => void;
}) {
  const { t } = useI18n();
  const firstDescriptor = getMetricDescriptor(card.metricIds[0]);
  const addable = availableDescriptors.filter((descriptor) => canAddMetricToCard(card, descriptor.id));
  const metricLimitReached = card.metricIds.length >= MAX_METRICS_PER_CARD;
  const incompatibleAvailable = !metricLimitReached && availableDescriptors.some((descriptor) => !card.metricIds.includes(descriptor.id) && !canAddMetricToCard(card, descriptor.id));
  const title = firstDescriptor ? metricDisplayName(firstDescriptor, t, samples) : card.id;
  return <div className="rounded-lg border border-border/80 bg-muted/15 p-3">
    <div className="flex flex-wrap items-start justify-between gap-3">
      <div className="min-w-0">
        <div className="flex flex-wrap items-center gap-2"><span className="text-sm font-semibold">{title}</span><Badge className={card.visible ? "border-[hsl(var(--success)/0.3)] bg-[hsl(var(--success-surface))] text-[hsl(var(--success))]" : "border-border bg-muted text-muted-foreground"}>{card.visible ? t("dashboardShowCard") : t("dashboardHideCard")}</Badge></div>
        <div className="mt-1 text-[11px] text-muted-foreground">{card.id}</div>
      </div>
      <div className="flex flex-wrap items-center gap-1.5">
        <Button type="button" variant="ghost" className="h-8 px-2 text-xs" disabled={index === 0} aria-label={t("dashboardMoveUp")} onClick={() => onMove(-1)}>↑</Button>
        <Button type="button" variant="ghost" className="h-8 px-2 text-xs" disabled={index === cardCount - 1} aria-label={t("dashboardMoveDown")} onClick={() => onMove(1)}>↓</Button>
        <Button type="button" variant="ghost" className="h-8 px-2 text-xs" onClick={() => onChange({ ...card, visible: !card.visible })}>{card.visible ? t("dashboardHideCard") : t("dashboardShowCard")}</Button>
        <Button type="button" variant="ghost" className="h-8 px-2 text-xs text-[hsl(var(--danger))]" onClick={onRemove}>{t("dashboardRemoveCard")}</Button>
      </div>
    </div>
    <div className="mt-3 flex flex-wrap gap-2">
      {card.metricIds.map((metricId) => {
        const descriptor = getMetricDescriptor(metricId);
        if (!descriptor) return null;
        const hidden = card.hiddenMetricIds.includes(metricId);
        const unavailable = !hasMetricData(metricId, samples);
        return <button key={metricId} type="button" className={`rounded-full border px-2.5 py-1 text-[11px] transition-colors ${hidden ? "border-border bg-muted text-muted-foreground" : "border-[hsl(var(--signal-cyan)/0.35)] bg-card text-foreground"}`} onClick={() => onChange({ ...card, hiddenMetricIds: hidden ? card.hiddenMetricIds.filter((id) => id !== metricId) : [...card.hiddenMetricIds, metricId] })} title={hidden ? t("dashboardShowMetric") : t("dashboardHideMetric")}>
          {metricDisplayName(descriptor, t, samples)}{unavailable ? ` · ${t("dashboardMetricUnavailable")}` : ""}{hidden ? ` · ${t("dashboardHideMetric")}` : ""}
        </button>;
      })}
    </div>
    <div className="mt-3 flex flex-wrap items-center gap-2">
      <label className="text-xs text-muted-foreground" htmlFor={`add-metric-${card.id}`}>{t("dashboardAddMetric")}</label>
      <select id={`add-metric-${card.id}`} value="" disabled={metricLimitReached || !addable.length} className="h-8 max-w-full rounded-md border border-input bg-card px-2 text-xs text-foreground" onChange={(event) => {
        const metricId = event.target.value as MetricId;
        if (!metricId) return;
        onChange({ ...card, metricIds: [...card.metricIds, metricId] });
        event.currentTarget.value = "";
      }}>
        <option value="">{t("dashboardSelectMetric")}</option>
        {addable.map((descriptor) => <option key={descriptor.id} value={descriptor.id}>{metricDisplayName(descriptor, t, samples)}</option>)}
      </select>
      {metricLimitReached && <span className="text-[11px] text-muted-foreground">{t("dashboardMetricLimitReached")}</span>}
      {incompatibleAvailable && <span className="text-[11px] text-muted-foreground">{t("dashboardIncompatibleMetric")}</span>}
    </div>
  </div>;
}
