import { useEffect, useMemo, useRef, useState } from "react";
import { CalendarDays, ChevronLeft, ChevronRight, LoaderCircle } from "lucide-react";
import { useI18n } from "../i18n";
import { FloatingPanel } from "./FloatingPanel";

type DateRangePickerProps = {
  value: string;
  onChange: (date: string) => void;
  availableDates?: string[];
  loading?: boolean;
};

export function DateRangePicker({ value, onChange, availableDates, loading = false }: DateRangePickerProps) {
  const { language, t } = useI18n();
  const [open, setOpen] = useState(false);
  const [visibleMonth, setVisibleMonth] = useState(value.slice(0, 7));
  const containerRef = useRef<HTMLDivElement | null>(null);
  const available = useMemo(() => new Set(availableDates), [availableDates]);
  const months = useMemo(() => {
    if (!availableDates?.length) return null;
    return { min: availableDates[0].slice(0, 7), max: availableDates[availableDates.length - 1].slice(0, 7) };
  }, [availableDates]);

  useEffect(() => { if (open) setVisibleMonth(value.slice(0, 7)); }, [open, value]);
  const [year, month] = visibleMonth.split("-").map(Number);
  const firstWeekday = new Date(year, month - 1, 1, 12).getDay();
  const daysInMonth = new Date(year, month, 0, 12).getDate();
  const cells = Array.from({ length: 42 }, (_, index) => {
    const day = index - firstWeekday + 1;
    return day >= 1 && day <= daysInMonth ? day : null;
  });
  const weekdayLabels = Array.from({ length: 7 }, (_, day) =>
    new Intl.DateTimeFormat(language, { weekday: "short" }).format(new Date(2026, 0, 4 + day, 12))
  );

  function moveMonth(delta: number) {
    const next = new Date(year, month - 1 + delta, 1, 12);
    setVisibleMonth(`${next.getFullYear()}-${String(next.getMonth() + 1).padStart(2, "0")}`);
  }

  return <div ref={containerRef} className="relative">
    <button type="button" disabled={loading} className="flex h-9 min-w-40 items-center justify-between gap-2 rounded-lg border border-input bg-card px-3.5 text-sm font-medium outline-none transition-[background-color,border-color,transform] duration-150 hover:border-foreground/25 hover:bg-muted/60 focus-visible:ring-2 focus-visible:ring-ring/35 disabled:cursor-wait disabled:opacity-60" onClick={() => setOpen((current) => !current)} aria-expanded={open} aria-haspopup="dialog" aria-label={t("selectDateLabel", { date: new Date(`${value}T12:00:00`).toLocaleDateString(language) })}>
      <span>{new Date(`${value}T12:00:00`).toLocaleDateString(language, { year: "numeric", month: "short", day: "numeric" })}</span>{loading ? <LoaderCircle size={15} className="animate-spin text-muted-foreground" /> : <CalendarDays size={16} className="text-muted-foreground" />}
    </button>
    <FloatingPanel open={open} anchorRef={containerRef} onClose={() => setOpen(false)} width={320} align="end" className="p-3" ariaLabel={t("datePickerDialog")}>
      <div className="mb-3 flex items-center justify-between">
        <button type="button" className="rounded-md p-1.5 transition-colors hover:bg-muted disabled:cursor-not-allowed disabled:opacity-30" disabled={!!months && visibleMonth <= months.min} onClick={() => moveMonth(-1)} aria-label={t("previousMonth")}><ChevronLeft size={17} /></button>
        <strong className="text-sm">{new Date(year, month - 1, 1, 12).toLocaleDateString(language, { year: "numeric", month: "long" })}</strong>
        <button type="button" className="rounded-md p-1.5 transition-colors hover:bg-muted disabled:cursor-not-allowed disabled:opacity-30" disabled={!!months && visibleMonth >= months.max} onClick={() => moveMonth(1)} aria-label={t("nextMonth")}><ChevronRight size={17} /></button>
      </div>
      <div className="grid grid-cols-7 text-center text-xs text-muted-foreground">{weekdayLabels.map((label) => <div key={label} className="py-1">{label}</div>)}</div>
      <div className="grid grid-cols-7 gap-1">{cells.map((day, index) => {
        if (day == null) return <div key={`blank-${index}`} />;
        const date = `${visibleMonth}-${String(day).padStart(2, "0")}`;
        const enabled = availableDates == null || available.has(date);
        const selected = date === value;
        return <button key={date} type="button" disabled={!enabled || loading} onClick={() => { onChange(date); setOpen(false); }} title={enabled ? t("dateHasData") : t("dateHasNoData")} aria-current={selected ? "date" : undefined}
          className={`relative h-9 rounded-md text-sm transition-[background-color,color,transform] duration-150 ${selected ? "bg-primary font-semibold text-primary-foreground" : enabled ? "text-foreground hover:bg-accent" : "cursor-not-allowed bg-muted/40 text-muted-foreground/45"}`}>
          {day}{enabled && !selected && <span className="absolute bottom-1 left-1/2 h-1 w-1 -translate-x-1/2 rounded-full bg-primary" />}
        </button>;
      })}</div>
      <div className="mt-3 flex items-center gap-2 border-t border-border pt-2 text-xs text-muted-foreground"><span className="h-2 w-2 rounded-full bg-primary" />{t("dateAvailabilityLegend")}</div>
    </FloatingPanel>
  </div>;
}
