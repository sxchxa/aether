// Santé des disques (SMART) : verdict clair par disque, température, usure et erreurs. Lecture seule.
import { useEffect, useState } from "react";
import { CheckCircle2, CircleHelp, HardDrive, OctagonAlert, RefreshCw, TriangleAlert } from "lucide-react";
import { Button, Card, CardHeader, cx, Empty, Skeleton } from "../../components/ui";
import { fmt, useI18n } from "../../i18n";
import { formatBytes } from "../../lib/format";
import { api } from "../../lib/ipc";
import type { DiskHealth, DiskVerdict } from "../../lib/types";

const VERDICT_STYLE: Record<DiskVerdict, string> = {
  good: "border-[var(--good)]/25 bg-[var(--good)]/10 text-[var(--good)]",
  warning: "border-amber-400/25 bg-amber-400/10 text-amber-300",
  critical: "border-rose-400/30 bg-rose-400/10 text-rose-300",
  unknown: "border-white/[0.08] text-zinc-400",
};

const VERDICT_ICON: Record<DiskVerdict, typeof CheckCircle2> = {
  good: CheckCircle2,
  warning: TriangleAlert,
  critical: OctagonAlert,
  unknown: CircleHelp,
};

export function DiskHealthCard() {
  const { m, lang } = useI18n();
  const [disks, setDisks] = useState<DiskHealth[] | null>(null);
  const [busy, setBusy] = useState(false);
  const load = async () => {
    setBusy(true);
    try {
      setDisks(await api.disksHealth());
    } catch {
      setDisks([]);
    } finally {
      setBusy(false);
    }
  };
  useEffect(() => {
    void load();
  }, []);

  return (
    <Card id="st-health">
      <CardHeader
        title={m.diagnostics.diskTitle}
        description={m.diagnostics.diskDesc}
        actions={
          <Button size="sm" variant="ghost" loading={busy} icon={<RefreshCw className="size-3.5" />} onClick={() => void load()}>
            {m.scan.analyze}
          </Button>
        }
      />
      <div className="divide-y divide-white/[0.05] border-t border-white/[0.05]">
        {disks === null && <Skeleton className="mx-6 my-4 h-12" />}
        {disks?.length === 0 && <Empty>{m.diagnostics.diskNone}</Empty>}
        {disks?.map((d, i) => {
          const Icon = VERDICT_ICON[d.verdict];
          const details = [
            d.temperature !== null ? fmt(m.diagnostics.temperature, { value: d.temperature }) : null,
            d.wear !== null ? fmt(m.diagnostics.wear, { value: d.wear }) : null,
            d.powerOnHours !== null ? fmt(m.diagnostics.hours, { value: d.powerOnHours.toLocaleString(lang) }) : null,
            d.uncorrectedErrors > 0 ? fmt(m.diagnostics.errors, { value: d.uncorrectedErrors }) : null,
          ].filter(Boolean);
          return (
            <div key={`${d.name}-${i}`} className="flex min-w-0 flex-wrap items-center gap-x-4 gap-y-2 px-6 py-3.5">
              <span className="grid size-9 shrink-0 place-items-center rounded-[10px] border border-white/[0.06] bg-white/[0.02] text-[var(--steel)]">
                <HardDrive className="size-[17px]" strokeWidth={1.6} aria-hidden />
              </span>
              <div className="min-w-0 flex-1 basis-[220px]">
                <div className="truncate text-[14px] text-zinc-100">{d.name || "—"}</div>
                <div className="mt-0.5 truncate text-[12.5px] text-zinc-500">
                  {[d.media, d.bus, d.size ? formatBytes(d.size, lang, 0) : null, ...details].filter(Boolean).join(" · ")}
                </div>
              </div>
              <span
                className={cx(
                  "inline-flex shrink-0 items-center gap-1.5 rounded-full border px-3 py-1 text-[12.5px]",
                  VERDICT_STYLE[d.verdict],
                )}
              >
                <Icon className="size-3.5" aria-hidden />
                {m.diagnostics.verdict[d.verdict]}
              </span>
            </div>
          );
        })}
      </div>
      {disks?.some((d) => d.verdict === "warning" || d.verdict === "critical") && (
        <p className="border-t border-white/[0.05] px-6 py-3 text-[12.5px] text-amber-300">{m.diagnostics.backupAdvice}</p>
      )}
    </Card>
  );
}
