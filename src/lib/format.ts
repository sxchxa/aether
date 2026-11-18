// Formatage localisé des tailles, durées et dates.

export function formatBytes(bytes: number, locale: string, digits = 1): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 o";
  const units = locale.startsWith("fr") ? ["o", "Ko", "Mo", "Go", "To"] : ["B", "KB", "MB", "GB", "TB"];
  const exp = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  const value = bytes / 1024 ** exp;
  const fmt = new Intl.NumberFormat(locale, { maximumFractionDigits: exp === 0 ? 0 : digits });
  return `${fmt.format(value)} ${units[exp] ?? ""}`;
}

export function formatPercent(value: number, locale: string): string {
  return new Intl.NumberFormat(locale, { maximumFractionDigits: 0 }).format(value) + " %";
}

export function formatDuration(seconds: number, locale: string): string {
  const d = Math.floor(seconds / 86400);
  const h = Math.floor((seconds % 86400) / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const nf = new Intl.NumberFormat(locale);
  if (d > 0) return `${nf.format(d)} j ${nf.format(h)} h`;
  if (h > 0) return `${nf.format(h)} h ${nf.format(m)} min`;
  return `${nf.format(m)} min`;
}

export function formatTime(ts: number, locale: string): string {
  return new Intl.DateTimeFormat(locale, { hour: "2-digit", minute: "2-digit", second: "2-digit" }).format(ts);
}

export function formatDate(value: string | number, locale: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return String(value);
  return new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeStyle: "short" }).format(date);
}

export function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}
