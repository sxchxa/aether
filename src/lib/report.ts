// Rapport HTML autonome (lisible dans tout navigateur, imprimable en PDF) : PC, mesures avant/après,
// état de chaque option et dernières modifications. Généré dans la langue de l'interface.
import type { Messages } from "../i18n";
import { formatBytes } from "./format";
import type { HistoryEntry, Measures, SystemInfo, TweakStatus } from "./types";

const esc = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

export interface ReportData {
  info: SystemInfo | null;
  tweaks: Record<string, TweakStatus>;
  history: HistoryEntry[];
  measures: Measures | null;
}

export function buildReport(m: Messages, lang: string, d: ReportData): string {
  const date = (ts: number) => new Date(ts).toLocaleString(lang, { dateStyle: "medium", timeStyle: "short" });
  const texts = m.tweaks as Record<string, { title: string } | undefined>;
  const rows = Object.values(d.tweaks)
    .sort((a, b) => a.section.localeCompare(b.section) || (texts[a.id]?.title ?? a.id).localeCompare(texts[b.id]?.title ?? b.id))
    .map((t) => {
      const state = !t.probe.available ? m.common.unavailable : t.probe.applied ? m.common.optimized : m.common.notOptimized;
      const cls = !t.probe.available ? "na" : t.probe.applied ? "ok" : "off";
      return `<tr><td>${esc(texts[t.id]?.title ?? t.id)}</td><td>${esc(m.nav[t.section])}</td><td class="${cls}">${esc(state)}</td></tr>`;
    })
    .join("");
  const available = Object.values(d.tweaks).filter((t) => t.probe.available);
  const applied = available.filter((t) => t.probe.applied).length;
  // Gains possibles : réglages sûrs encore disponibles, avec leur bénéfice attendu.
  const gainTexts = m.tweaks as Record<string, { title: string; gain: string } | undefined>;
  const potential = available
    .filter((t) => !t.probe.applied && t.risk === "safe" && gainTexts[t.id])
    .map((t) => `<tr><td>${esc(gainTexts[t.id]?.title ?? t.id)}</td><td>${esc(gainTexts[t.id]?.gain ?? "")}</td></tr>`)
    .join("");

  const ref = d.measures?.reference;
  const now = d.measures?.history.at(-1);
  const measureRows =
    ref && now
      ? (
          [
            [m.measure.boot, ref.bootMs, now.bootMs, (v: number) => `${(v / 1000).toFixed(1)} s`],
            [m.measure.ram, ref.ramUsed, now.ramUsed, (v: number) => formatBytes(v, lang, 1)],
            [m.measure.processes, ref.processes, now.processes, String],
            [m.measure.services, ref.servicesRunning, now.servicesRunning, String],
            [m.measure.startup, ref.startupItems, now.startupItems, String],
          ] as [string, number | null, number | null, (v: number) => string][]
        )
          .map(
            ([label, a, b, f]) =>
              `<tr><td>${esc(label)}</td><td>${a === null ? "—" : esc(f(a))}</td><td>${b === null ? "—" : esc(f(b))}</td></tr>`,
          )
          .join("")
      : "";

  const info = d.info;
  const history = d.history
    .slice(0, 20)
    .map((h) => `<tr><td>${esc(date(h.ts))}</td><td>${esc(h.label)}</td></tr>`)
    .join("");

  return `<!doctype html>
<html lang="${esc(lang)}"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>${esc(m.report.title)} · ${esc(info?.hostname ?? "")}</title>
<style>
body{font:14px/1.5 "Segoe UI",system-ui,sans-serif;color:#18181b;background:#fafafa;margin:0;padding:40px 16px}
main{max-width:860px;margin:0 auto}h1{font-size:26px;margin:0 0 4px}h2{font-size:16px;margin:36px 0 10px}
.muted{color:#71717a}table{width:100%;border-collapse:collapse;background:#fff;border:1px solid #e4e4e7;border-radius:8px;overflow:hidden}
td,th{text-align:left;padding:8px 12px;border-bottom:1px solid #f0f0f2}th{font-weight:600;color:#52525b;background:#f7f7f8}
.ok{color:#047857}.off{color:#71717a}.na{color:#a1a1aa}.kpi{display:flex;gap:12px;flex-wrap:wrap}
.kpi div{flex:1 1 160px;background:#fff;border:1px solid #e4e4e7;border-radius:8px;padding:12px 14px}.kpi b{display:block;font-size:20px}
@media print{body{background:#fff;padding:0}}
</style></head><body><main>
<h1>${esc(m.report.title)}</h1>
<div class="muted">${esc(m.report.generated.replace("{{date}}", date(Date.now())))}</div>
<h2>${esc(m.report.machine)}</h2>
<div class="kpi">
<div><span class="muted">PC</span><b>${esc(info?.hostname ?? "—")}</b></div>
<div><span class="muted">Windows</span><b>${esc(info ? `${info.osName}` : "—")}</b></div>
<div><span class="muted">CPU</span><b>${esc(info?.cpu ?? "—")}</b></div>
<div><span class="muted">RAM</span><b>${info ? esc(formatBytes(info.totalMemory, lang, 0)) : "—"}</b></div>
<div><span class="muted">${esc(m.report.options)}</span><b>${applied} / ${available.length}</b></div>
</div>
${measureRows ? `<h2>${esc(m.measure.title)}</h2><table><tr><th></th><th>${esc(m.report.before)}</th><th>${esc(m.report.after)}</th></tr>${measureRows}</table>` : ""}
<h2>${esc(m.report.options)}</h2>
<table><tr><th>${esc(m.report.option)}</th><th>${esc(m.report.section)}</th><th>${esc(m.report.state)}</th></tr>${rows}</table>
${potential ? `<h2>${esc(m.report.potential)}</h2><p class="muted">${esc(m.report.potentialDesc)}</p><table><tr><th>${esc(m.report.option)}</th><th>${esc(m.common.gain)}</th></tr>${potential}</table>` : ""}
${history ? `<h2>${esc(m.titlebar.history)}</h2><table>${history}</table>` : ""}
<p class="muted" style="margin-top:32px">Aether</p>
</main></body></html>`;
}
