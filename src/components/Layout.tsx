import { getVersion } from "@tauri-apps/api/app";
import { Activity, Database, Monitor, Moon, PanelLeftClose, PanelLeftOpen, Sun } from "lucide-react";
import { useEffect, useState } from "react";
import { useI18n, type TranslationKey } from "../i18n";
import { mainNavigation } from "../navigation";
import { useUiStore } from "../stores/uiStore";
import { Button } from "./ui/Button";

export function Layout({ children }: { children: React.ReactNode }) {
  const page = useUiStore((state) => state.page);
  const setPage = useUiStore((state) => state.setPage);
  const sidebarCollapsed = useUiStore((state) => state.sidebarCollapsed);
  const toggleSidebar = useUiStore((state) => state.toggleSidebar);
  const themeMode = useUiStore((state) => state.themeMode);
  const setThemeMode = useUiStore((state) => state.setThemeMode);
  const setResolvedTheme = useUiStore((state) => state.setResolvedTheme);
  const { t } = useI18n();
  const [version, setVersion] = useState("0.3.2");
  useEffect(() => { getVersion().then(setVersion).catch(() => undefined); }, []);
  useEffect(() => {
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const applyTheme = () => {
      const dark = themeMode === "dark" || (themeMode === "system" && media.matches);
      document.documentElement.classList.toggle("dark", dark);
      document.documentElement.dataset.theme = dark ? "dark" : "light";
      setResolvedTheme(dark ? "dark" : "light");
    };
    applyTheme();
    media.addEventListener("change", applyTheme);
    return () => media.removeEventListener("change", applyTheme);
  }, [setResolvedTheme, themeMode]);
  const themeOptions = ["system", "light", "dark"] as const;
  const cycleTheme = () => setThemeMode(themeOptions[(themeOptions.indexOf(themeMode) + 1) % themeOptions.length]);
  const ThemeIcon = themeMode === "dark" ? Moon : themeMode === "light" ? Sun : Monitor;
  const themeLabel = t(themeMode === "dark" ? "themeDark" : themeMode === "light" ? "themeLight" : "themeSystem");
  return (
    <div className="app-shell flex min-h-screen bg-background">
      <a className="skip-link" href="#main-content">{t("skipToContent")}</a>
      <aside className={`sticky top-0 flex h-screen shrink-0 flex-col border-r border-border/70 bg-[hsl(var(--sidebar))] p-3 text-[hsl(var(--sidebar-foreground))] transition-[width] duration-200 ease-out ${sidebarCollapsed ? "w-[68px]" : "w-60"}`}>
        <div className={`mb-5 grid h-10 items-center ${sidebarCollapsed ? "grid-cols-1 justify-items-center" : "grid-cols-[36px_minmax(0,1fr)_32px] gap-3 px-1"}`}>
          <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg border border-border bg-[hsl(var(--sidebar-raised))] text-[hsl(var(--sidebar-foreground))] shadow-sm">
            <Activity size={19} strokeWidth={2.2} />
          </div>
          {!sidebarCollapsed && <div className="min-w-0">
            <div className="truncate text-[15px] font-semibold">Resource Timeline</div>
            <div className="mt-0.5 truncate text-[11px] text-[hsl(var(--sidebar-muted))]">{t("brandSubtitle")}</div>
          </div>}
          {!sidebarCollapsed && <Button
            type="button"
            variant="ghost"
            size="icon"
            className="h-8 w-8 text-[hsl(var(--sidebar-muted))] hover:bg-muted hover:text-[hsl(var(--sidebar-foreground))]"
            onClick={toggleSidebar}
            aria-label={t("collapseNavigation")}
            title={t("collapseNavigation")}
          >
            <PanelLeftClose size={16} />
          </Button>}
        </div>
        {sidebarCollapsed && <Button type="button" variant="ghost" size="icon" className="mb-3 h-9 w-full text-[hsl(var(--sidebar-muted))] hover:bg-muted hover:text-[hsl(var(--sidebar-foreground))]" onClick={toggleSidebar} aria-label={t("expandNavigation")} title={t("expandNavigation")}><PanelLeftOpen size={16} /></Button>}
        <nav className="space-y-1">
          {mainNavigation.map((item) => {
            const Icon = item.icon;
            const active = page === item.id;
            const label = t(item.label as TranslationKey);
            return <Button key={item.id} variant="ghost" className={`relative w-full ${sidebarCollapsed ? "justify-center px-0" : "justify-start px-3"} ${active ? "bg-[hsl(var(--sidebar-raised))] text-[hsl(var(--sidebar-foreground))] shadow-sm ring-1 ring-inset ring-border/70 hover:bg-[hsl(var(--sidebar-raised))]" : "text-[hsl(var(--sidebar-muted))] hover:bg-muted hover:text-[hsl(var(--sidebar-foreground))]"}`} onClick={() => setPage(item.id)} aria-current={active ? "page" : undefined} aria-label={sidebarCollapsed ? label : undefined} title={sidebarCollapsed ? label : undefined}>
              <Icon size={16} strokeWidth={active ? 2.2 : 1.8} />{!sidebarCollapsed && label}
            </Button>;
          })}
        </nav>
        <div className="mt-auto space-y-2">
          <Button variant="ghost" className={`w-full text-[hsl(var(--sidebar-muted))] hover:bg-muted hover:text-[hsl(var(--sidebar-foreground))] ${sidebarCollapsed ? "px-0" : "justify-start"}`} onClick={cycleTheme} title={themeLabel} aria-label={themeLabel}>
            <ThemeIcon size={16} />
            {!sidebarCollapsed && <span>{themeLabel}</span>}
          </Button>
          <div className={`rounded-lg border border-border/70 bg-[hsl(var(--sidebar-raised))] px-3 py-3 ${sidebarCollapsed ? "flex justify-center" : ""}`}>
          <div className="flex items-center gap-2 text-xs text-[hsl(var(--sidebar-muted))]">
            <Database size={14} />
            {!sidebarCollapsed && <span className="truncate">{t("localData")}</span>}
            {!sidebarCollapsed && <span className="ml-auto font-mono text-[10px] opacity-55">v{version}</span>}
          </div>
          {!sidebarCollapsed && <div className="mt-2 flex items-center gap-2 text-[10px] uppercase text-[hsl(var(--sidebar-muted))]">
            <span className="h-1.5 w-1.5 rounded-full bg-[hsl(var(--success))]" />
            {t("localOnly")}
          </div>}
          </div>
        </div>
      </aside>
      <main id="main-content" tabIndex={-1} className="workspace-surface min-w-0 flex-1 overflow-auto p-4 outline-none xl:p-5">
        <div className="page-shell">{children}</div>
      </main>
    </div>
  );
}
