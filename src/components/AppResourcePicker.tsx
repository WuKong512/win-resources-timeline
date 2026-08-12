import { useEffect, useMemo, useRef, useState } from "react";
import { Check, ChevronDown, Search } from "lucide-react";
import { useI18n } from "../i18n";
import type { ResourceApp } from "../types/resource";
import { FloatingPanel } from "./FloatingPanel";

type AppResourcePickerProps = {
  apps: ResourceApp[];
  value: string;
  onChange: (appKey: string) => void;
  loading?: boolean;
};

export function AppResourcePicker({
  apps,
  value,
  onChange,
  loading = false
}: AppResourcePickerProps) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const [search, setSearch] = useState("");
  const [activeIndex, setActiveIndex] = useState(0);
  const anchorRef = useRef<HTMLDivElement | null>(null);
  const triggerRef = useRef<HTMLButtonElement | null>(null);
  const inputRef = useRef<HTMLInputElement | null>(null);
  const selected = apps.find((app) => app.appKey === value);
  const filteredApps = useMemo(() => {
    const needle = search.trim().toLocaleLowerCase();
    if (!needle) return apps;
    return apps.filter((app) =>
      [app.displayName, app.processName, app.exePath ?? ""]
        .some((text) => text.toLocaleLowerCase().includes(needle))
    );
  }, [apps, search]);

  useEffect(() => {
    if (!open) return;
    setActiveIndex(Math.max(0, filteredApps.findIndex((app) => app.appKey === value)));
    const frame = window.requestAnimationFrame(() => inputRef.current?.focus());
    return () => window.cancelAnimationFrame(frame);
  }, [filteredApps, open, value]);

  function close() {
    setOpen(false);
    setSearch("");
  }

  function choose(appKey: string) {
    onChange(appKey);
    close();
    window.requestAnimationFrame(() => triggerRef.current?.focus());
  }

  function moveActive(delta: number) {
    if (!filteredApps.length) return;
    setActiveIndex((current) => (current + delta + filteredApps.length) % filteredApps.length);
  }

  return <div ref={anchorRef} className="w-full max-w-[460px]">
    <button
      ref={triggerRef}
      type="button"
      className="flex h-11 w-full items-center gap-3 rounded-lg border border-input bg-card px-3 text-left outline-none transition-[background-color,border-color,transform] duration-150 hover:border-foreground/25 hover:bg-muted/60 focus-visible:ring-2 focus-visible:ring-ring/35 disabled:cursor-wait disabled:opacity-60"
      disabled={loading || !apps.length}
      onClick={() => setOpen((current) => !current)}
      onKeyDown={(event) => {
        if (event.key === "ArrowDown" || event.key === "ArrowUp") {
          event.preventDefault();
          setOpen(true);
          moveActive(event.key === "ArrowDown" ? 1 : -1);
        }
      }}
      aria-expanded={open}
      aria-haspopup="listbox"
    >
      <span className="min-w-0 flex-1">
        <span className="block truncate text-sm font-semibold">{selected?.displayName ?? t("chooseApp")}</span>
        {selected && <span className="mt-0.5 block truncate text-[11px] text-muted-foreground">{selected.processName}</span>}
      </span>
      <ChevronDown size={16} className={`shrink-0 text-muted-foreground transition-transform ${open ? "rotate-180" : ""}`} />
    </button>

    <FloatingPanel open={open} anchorRef={anchorRef} onClose={close} width={460} className="overflow-hidden" ariaLabel={t("appPickerDialog")}>
      <div className="border-b border-border p-3">
        <div className="relative">
          <Search size={15} className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground" />
          <input
            ref={inputRef}
            className="h-10 w-full rounded-md border border-input bg-card pl-9 pr-3 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring/35"
            value={search}
            onChange={(event) => setSearch(event.target.value)}
            placeholder={t("appSearch")}
            role="combobox"
            aria-expanded={open}
            aria-controls="resource-app-picker-options"
            aria-activedescendant={filteredApps[activeIndex] ? `resource-app-option-${activeIndex}` : undefined}
            onKeyDown={(event) => {
              if (event.key === "ArrowDown") { event.preventDefault(); moveActive(1); }
              if (event.key === "ArrowUp") { event.preventDefault(); moveActive(-1); }
              if (event.key === "Enter" && filteredApps[activeIndex]) { event.preventDefault(); choose(filteredApps[activeIndex].appKey); }
              if (event.key === "Escape") { event.preventDefault(); close(); triggerRef.current?.focus(); }
            }}
          />
        </div>
      </div>
      <div id="resource-app-picker-options" role="listbox" className="max-h-[340px] overflow-y-auto p-2">
        {filteredApps.length ? filteredApps.map((app) => {
          const index = filteredApps.findIndex((item) => item.appKey === app.appKey);
          const selectedApp = app.appKey === value;
          const active = index === activeIndex;
          return <button
            key={app.appKey}
            type="button"
            id={`resource-app-option-${index}`}
            role="option"
            aria-selected={selectedApp}
            className={`mb-1 flex w-full items-center gap-3 rounded-md px-3 py-2.5 text-left last:mb-0 ${active ? "bg-accent text-accent-foreground" : "hover:bg-muted/60"}`}
            onMouseEnter={() => setActiveIndex(index)}
            onClick={() => choose(app.appKey)}
          >
            <span className="min-w-0 flex-1">
              <span className="block truncate text-sm font-semibold">{app.displayName}</span>
              <span className={`mt-0.5 block truncate text-[11px] ${active ? "text-accent-foreground/70" : "text-muted-foreground"}`}>{app.processName}</span>
            </span>
            {selectedApp && <Check size={15} className="shrink-0" />}
          </button>;
        }) : <div className="py-10 text-center text-sm text-muted-foreground">{t("noMatchingResourceApps")}</div>}
      </div>
    </FloatingPanel>
  </div>;
}
