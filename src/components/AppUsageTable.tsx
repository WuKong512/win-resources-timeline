import type { AppUsageSummary } from "../types/resource";
import { useI18n } from "../i18n";
import { formatDuration } from "../utils/time";
import { Table, Td, Th } from "./ui/Table";

export function AppUsageTable({ apps }: { apps: AppUsageSummary[] }) {
  const { language, t } = useI18n();
  return <Table>
    <thead><tr><Th>{t("app")}</Th><Th className="w-32 text-right">{t("active")}</Th><Th className="w-28 text-right">{t("share")}</Th></tr></thead>
    <tbody>{apps.map((app) => <tr key={app.appId} className="transition-colors hover:bg-muted/25">
      <Td className="px-5"><div className="font-medium">{app.displayName || app.appName}</div><div className="mt-2 h-1.5 max-w-md overflow-hidden rounded-full bg-muted"><div className="h-full rounded-full bg-[hsl(var(--signal-blue))]" style={{ width: `${Math.min(100, Math.max(1, app.percentage))}%` }} /></div></Td>
      <Td className="text-right font-semibold">{formatDuration(app.activeSeconds, language)}</Td>
      <Td className="pr-5 text-right text-muted-foreground">{app.percentage.toFixed(1)}%</Td>
    </tr>)}</tbody>
  </Table>;
}
