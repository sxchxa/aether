// État de la batterie (PC portables) : capacité d'origine et actuelle, usure, cycles, rapport complet.
import { useEffect, useState } from "react";
import { BatteryMedium, ExternalLink } from "lucide-react";
import { Button, Card, CardHeader, cx } from "../../components/ui";
import { fmt, useI18n } from "../../i18n";
import { api, errorMessage } from "../../lib/ipc";
import type { BatteryHealth } from "../../lib/types";
import { useApp } from "../../store/app";

export function BatteryCard() {
  const { m, lang } = useI18n();
  const toast = useApp((s) => s.toast);
  const [battery, setBattery] = useState<BatteryHealth | null | undefined>(undefined);
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    void api
      .batteryHealth()
      .then(setBattery)
      .catch(() => setBattery(null));
  }, []);
  // PC sans batterie (ou état inconnu) : la carte n'est pas affichée.
  if (!battery) return null;
  const health = Math.max(0, 100 - battery.wearPercent);
  const wh = (mwh: number) => `${(mwh / 1000).toLocaleString(lang, { maximumFractionDigits: 1 })} Wh`;
  return (
    <Card id="pw-battery">
      <CardHeader
        title={m.diagnostics.batteryTitle}
        actions={
          <Button
            size="sm"
            variant="ghost"
            loading={busy}
            icon={<ExternalLink className="size-3.5" />}
            onClick={() => {
              setBusy(true);
              void api
                .batteryReportOpen()
                .catch((e: unknown) => toast("error", errorMessage(e)))
                .finally(() => setBusy(false));
            }}
          >
            {m.diagnostics.batteryReport}
          </Button>
        }
      />
      <div className="flex flex-wrap items-center gap-6 border-t border-white/[0.05] px-6 py-5">
        <BatteryMedium className="size-8 text-[var(--steel)]" strokeWidth={1.4} aria-hidden />
        <div className="min-w-[180px] flex-1">
          <div className="flex items-baseline justify-between gap-3">
            <span className="text-[14px] text-zinc-100">{fmt(m.diagnostics.batteryHealth, { value: Math.round(health) })}</span>
            <span className="tabular text-[12.5px] text-zinc-500">
              {wh(battery.fullMwh)} / {wh(battery.designMwh)}
            </span>
          </div>
          <div className="mt-2 h-1.5 overflow-hidden rounded-full bg-white/[0.06]">
            <div
              className={cx("meter-fill h-full rounded-full", health < 60 ? "bg-amber-400" : "bg-[var(--good)]")}
              style={{ transform: `scaleX(${health / 100})` }}
            />
          </div>
          <div className="mt-2 text-[12.5px] text-zinc-500">
            {fmt(m.diagnostics.batteryWear, { value: battery.wearPercent.toLocaleString(lang) })}
            {battery.cycles !== null && ` · ${fmt(m.diagnostics.batteryCycles, { value: battery.cycles })}`}
          </div>
        </div>
      </div>
    </Card>
  );
}
