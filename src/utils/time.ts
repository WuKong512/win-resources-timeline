import type { Language } from "../i18n";

export function localDateString(date = new Date()): string {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

export function localDayRange(date: string): { startMs: number; endMs: number } {
  const [year, month, day] = date.split("-").map(Number);
  const start = new Date(year, month - 1, day);
  const end = new Date(year, month - 1, day + 1);
  return { startMs: start.getTime(), endMs: end.getTime() };
}

export function clipInterval(
  startMs: number,
  endMs: number,
  rangeStartMs: number,
  rangeEndMs: number
): { startMs: number; endMs: number } | null {
  const start = Math.max(startMs, rangeStartMs);
  const end = Math.min(endMs, rangeEndMs);
  return end > start ? { startMs: start, endMs: end } : null;
}

export function timelinePercent(timestampMs: number, rangeStartMs: number, rangeEndMs: number): number {
  if (rangeEndMs <= rangeStartMs) return 0;
  return Math.min(100, Math.max(0, (timestampMs - rangeStartMs) * 100 / (rangeEndMs - rangeStartMs)));
}

export function formatDuration(seconds: number, language: Language = "en"): string {
  const value = Math.max(0, Math.round(seconds));
  const hours = Math.floor(value / 3600);
  const minutes = Math.floor((value % 3600) / 60);
  const rest = value % 60;
  if (language === "zh-CN") {
    if (hours > 0) return `${hours} 小时 ${minutes} 分钟`;
    if (minutes > 0) return `${minutes} 分钟 ${rest} 秒`;
    return `${rest} 秒`;
  }
  if (hours > 0) return `${hours}h ${minutes}m`;
  if (minutes > 0) return `${minutes}m ${rest}s`;
  return `${rest}s`;
}

export function formatClock(timestampMs: number, language: Language = "en"): string {
  return new Date(timestampMs).toLocaleTimeString(language === "zh-CN" ? "zh-CN" : "en", { hour: "2-digit", minute: "2-digit", second: "2-digit" });
}

export function formatBytes(value: number | null, language: Language = "en"): string {
  if (value == null) return language === "zh-CN" ? "无采样数据" : "No sample";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let amount = Math.max(0, value);
  let unit = 0;
  while (amount >= 1024 && unit < units.length - 1) { amount /= 1024; unit += 1; }
  return `${amount >= 100 ? amount.toFixed(0) : amount.toFixed(1)} ${units[unit]}`;
}
