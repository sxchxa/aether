// Test de débit : latence, téléchargement et envoi, avec valeurs en direct pendant la mesure.
import { useState } from "react";
import { ArrowDown, ArrowUp, Gauge, Timer } from "lucide-react";
import { Button, Card, CardHeader, cx } from "../../components/ui";
import { fmt, useI18n } from "../../i18n";
import { api, errorMessage } from "../../lib/ipc";
import type { SpeedPhase, SpeedResult } from "../../lib/types";
import { useApp } from "../../store/app";

interface Live {
  phase: SpeedPhase;
  mbps: number | null;
}

export function SpeedTestCard() {
  const { m, lang } = useI18n();
  const toast = useApp((s) => s.toast);
  const [live, setLive] = useState<Live | null>(null);
  const [result, setResult] = useState<SpeedResult | null>(null);
  const running = live !== null && live.phase !== "done";
  const nf = new Intl.NumberFormat(lang, { maximumFractionDigits: 0 });

  const start = async () => {
    setResult(null);
    setLive({ phase: "ping", mbps: null });
    try {
      const r = await api.netSpeedtest((e) => setLive({ phase: e.phase, mbps: e.mbps }));
      setResult(r);
    } catch (e) {
      toast("error", errorMessage(e));
    } finally {
      setLive(null);
    }
  };

  // Valeur affichée par tuile : résultat final, sinon mesure en direct de l'étape en cours.
  const value = (phase: "download" | "upload") => {
    if (result) return nf.format(phase === "download" ? result.downloadMbps : result.uploadMbps);
    if (live?.phase === phase && live.mbps !== null) return nf.format(live.mbps);
    return "—";
  };

  const tiles = [
    {
      key: "ping" as const,
      icon: <Timer className="size-4" aria-hidden />,
      label: m.network.speedPing,
      value: result ? nf.format(result.pingMs) : "—",
      unit: "ms",
      hint: result ? fmt(m.network.speedJitter, { value: nf.format(result.jitterMs) }) : "",
    },
    {
      key: "download" as const,
      icon: <ArrowDown className="size-4" aria-hidden />,
      label: m.network.speedDown,
      value: value("download"),
      unit: "Mbit/s",
      hint: "",
    },
    {
      key: "upload" as const,
      icon: <ArrowUp className="size-4" aria-hidden />,
      label: m.network.speedUp,
      value: value("upload"),
      unit: "Mbit/s",
      hint: "",
    },
  ];

  return (
    <Card id="nw-speed">
      <CardHeader
        icon={<Gauge className="size-[18px]" />}
        tone="sky"
        title={m.network.speedTitle}
        description={m.network.speedDesc}
        actions={
          <Button size="sm" variant="primary" loading={running} onClick={() => void start()}>
            {running ? m.network.speedRunning : result ? m.network.speedAgain : m.network.speedRun}
          </Button>
        }
      />
      <div className="grid grid-cols-1 gap-3 px-5 pb-5 sm:grid-cols-3">
        {tiles.map((t) => (
          <div
            key={t.key}
            className={cx(
              "surface-2 min-w-0 rounded-[10px] px-4 py-3.5 transition-colors",
              live?.phase === t.key && "!border-[var(--accent)]/60",
            )}
          >
            <div className="flex items-center gap-2 text-[12px] text-[var(--text-3)]">
              {t.icon}
              <span className="truncate">{t.label}</span>
            </div>
            <div className="mt-1.5 flex items-baseline gap-1.5">
              <span className="tabular text-[24px] font-semibold tracking-[-0.02em] text-white">{t.value}</span>
              <span className="text-[12px] text-[var(--text-3)]">{t.unit}</span>
            </div>
            {t.hint && <div className="mt-0.5 truncate text-[11.5px] text-[var(--text-3)]">{t.hint}</div>}
          </div>
        ))}
      </div>
      <p className="px-5 pb-4 text-[11.5px] text-[var(--text-3)]">{m.network.speedNote}</p>
    </Card>
  );
}
