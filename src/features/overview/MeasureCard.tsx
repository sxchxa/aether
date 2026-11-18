// Carte « Avant / Après » : compare l'état actuel du PC à la mesure de référence prise au premier lancement.
import { useEffect, useState } from "react";
import { FileText, RefreshCw } from "lucide-react";
import { buildReport } from "../../lib/report";
import { Button, Card, CardHeader, cx } from "../../components/ui";
import { fmt, useI18n } from "../../i18n";
import { formatBytes } from "../../lib/format";
import { api, errorMessage } from "../../lib/ipc";
import type { Measure, Measures } from "../../lib/types";
import { useApp } from "../../store/app";

type Row = { key: string; label: string; value: (m: Measure) => number | null; show: (v: number) => string };

export function MeasureCard() {
  const { m, lang } = useI18n();
  const toast = useApp((s) => s.toast);

  const exportReport = async () => {
    setBusy(true);
    try {
      const s = useApp.getState();
      const html = buildReport(m, lang, { info: s.systemInfo, tweaks: s.tweaks, history: s.history, measures: data });
      const path = await api.reportSave(html, true);
      toast("success", fmt(m.report.saved, { path }));
      void api.storageReveal(path).catch(() => undefined);
    } catch (e) {
      toast("error", errorMessage(e));
    } finally {
      setBusy(false);
    }
  };
  const [data, setData] = useState<Measures | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    void api
      .measuresGet()
      .then(setData)
      .catch(() => setData(null));
  }, []);

  const run = async (fn: () => Promise<Measures>) => {
    setBusy(true);
    try {
      setData(await fn());
    } catch (e) {
      toast("error", errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const ref = data?.reference ?? null;
  const now = data?.history.at(-1) ?? null;
  const rows: Row[] = [
    {
      key: "boot",
      label: m.measure.boot,
      value: (x) => x.bootMs,
      show: (v) => `${(v / 1000).toLocaleString(lang, { maximumFractionDigits: 1 })} s`,
    },
    { key: "ram", label: m.measure.ram, value: (x) => x.ramUsed, show: (v) => formatBytes(v, lang, 1) },
    { key: "proc", label: m.measure.processes, value: (x) => x.processes, show: (v) => String(v) },
    { key: "svc", label: m.measure.services, value: (x) => x.servicesRunning, show: (v) => String(v) },
    { key: "start", label: m.measure.startup, value: (x) => x.startupItems, show: (v) => String(v) },
  ];
  const date = (ts: number) =>
    new Date(ts).toLocaleDateString(lang, { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" });

  return (
    <Card id="ov-measure">
      <CardHeader
        title={m.measure.title}
        description={m.measure.desc}
        actions={
          <>
            {ref && now && ref.ts !== now.ts && (
              <Button size="sm" variant="ghost" disabled={busy} onClick={() => void run(api.measureReset)}>
                {m.measure.setReference}
              </Button>
            )}
            <Button size="sm" variant="ghost" disabled={busy} icon={<FileText className="size-3.5" />} onClick={() => void exportReport()}>
              {m.report.button}
            </Button>
            <Button size="sm" loading={busy} icon={<RefreshCw className="size-3.5" />} onClick={() => void run(api.measureNow)}>
              {m.measure.measureNow}
            </Button>
          </>
        }
      />
      {!ref || !now ? (
        <div className="px-6 pb-5 text-[13px] text-zinc-500">{m.common.loading}</div>
      ) : (
        <div className="px-5 pb-4">
          <div className="grid grid-cols-[minmax(0,1fr)_auto_auto_auto] items-baseline gap-x-6 border-b border-white/[0.05] px-1 pb-2 text-[11.5px] text-zinc-500">
            <span />
            <span className="text-right">{fmt(m.measure.reference, { date: date(ref.ts) })}</span>
            <span className="text-right">{fmt(m.measure.latest, { date: date(now.ts) })}</span>
            <span className="w-16 text-right">{m.measure.change}</span>
          </div>
          <ul className="divide-y divide-white/[0.05]">
            {rows.map((r) => {
              const before = r.value(ref);
              const after = r.value(now);
              const diff = before !== null && after !== null && before > 0 ? ((after - before) / before) * 100 : null;
              return (
                <li key={r.key} className="grid grid-cols-[minmax(0,1fr)_auto_auto_auto] items-baseline gap-x-6 px-1 py-2.5 text-[13.5px]">
                  <span className="truncate text-zinc-200">{r.label}</span>
                  <span className="tabular text-right text-zinc-500">{before === null ? "—" : r.show(before)}</span>
                  <span className="tabular text-right text-zinc-100">{after === null ? "—" : r.show(after)}</span>
                  <span
                    className={cx(
                      "tabular w-16 text-right text-[12.5px]",
                      diff === null || Math.abs(diff) < 1 ? "text-zinc-500" : diff < 0 ? "text-emerald-300" : "text-amber-300",
                    )}
                  >
                    {diff === null || Math.abs(diff) < 1 ? "=" : `${diff > 0 ? "+" : "−"}${Math.abs(Math.round(diff))} %`}
                  </span>
                </li>
              );
            })}
          </ul>
          <p className="px-1 pt-3 text-[11.5px] text-zinc-500">{m.measure.bootHint}</p>
        </div>
      )}
    </Card>
  );
}
