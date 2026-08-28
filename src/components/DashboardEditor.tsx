import { RotateCcw } from "lucide-react";
import { useI18n } from "../i18n";
import { type DashboardConfig } from "../dashboard/config";
import type { MetricCatalogItem, MetricId } from "../dashboard/metrics";
import type { SystemSample } from "../types/resource";
import { MetricExplorer } from "./MetricExplorer";
import { Button } from "./ui/Button";
import { Card, CardContent, CardHeader, CardTitle } from "./ui/Card";

type DashboardEditorProps = {
  config: DashboardConfig;
  catalog: MetricCatalogItem[];
  samples: SystemSample[];
  trendMetricIds: readonly MetricId[];
  onTogglePin: (metricId: MetricId) => void;
  onToggleTrend: (metricId: MetricId) => void;
  onMoveCard: (cardId: string, direction: -1 | 1) => void;
  onRestoreDefaults: () => void;
  saving: boolean;
  saveError: string;
};

export function DashboardEditor({ config, catalog, samples, trendMetricIds, onTogglePin, onToggleTrend, onMoveCard, onRestoreDefaults, saving, saveError }: DashboardEditorProps) {
  const { t } = useI18n();
  return <Card className="border-[hsl(var(--signal-cyan)/0.35)] bg-card/95">
    <CardHeader className="border-b border-border/70">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <CardTitle>{t("dashboardMetricExplorer")}</CardTitle>
          <p className="mt-1 max-w-2xl text-xs font-normal text-muted-foreground">{t("dashboardMetricExplorerDescription")}</p>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          {saving && <span className="text-xs text-muted-foreground">{t("dashboardSaving")}</span>}
          {!saving && !saveError && <span className="text-xs text-muted-foreground">{t("dashboardSaved")}</span>}
          {saveError && <span role="alert" className="text-xs text-[hsl(var(--danger))]">{saveError}</span>}
          <Button type="button" variant="outline" className="h-8 px-2.5 text-xs" onClick={onRestoreDefaults}><RotateCcw size={13} aria-hidden="true" />{t("dashboardRestoreDefaults")}</Button>
        </div>
      </div>
    </CardHeader>
    <CardContent className="pt-4">
      <MetricExplorer catalog={catalog} samples={samples} config={config} trendMetricIds={trendMetricIds} onTogglePin={onTogglePin} onToggleTrend={onToggleTrend} onMoveCard={onMoveCard} />
    </CardContent>
  </Card>;
}
