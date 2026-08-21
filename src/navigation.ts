import { Activity, BarChart3, Settings, ShieldAlert, type LucideIcon } from "lucide-react";
import type { TranslationKey } from "./i18n";
import type { Page } from "./stores/uiStore";

export type NavigationItem = {
  id: Page;
  label: TranslationKey;
  icon: LucideIcon;
};

export const mainNavigation: readonly NavigationItem[] = [
  { id: "timeline", label: "navTimeline", icon: Activity },
  { id: "usage", label: "navUsage", icon: BarChart3 },
  { id: "crashes", label: "navCrashes", icon: ShieldAlert },
  { id: "settings", label: "navSettings", icon: Settings }
];

export function isMainPage(value: string): value is Page {
  return mainNavigation.some((item) => item.id === value);
}
