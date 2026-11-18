// Points de restauration : liste, retour à un point (au redémarrage) et création manuelle.
import { useCallback, useEffect, useState } from "react";
import { History, RotateCcw, ShieldPlus } from "lucide-react";
import { ConfirmDialog } from "../../components/Overlays";
import { Button, Card, CardHeader, Skeleton } from "../../components/ui";
import { fmt, useI18n } from "../../i18n";
import { api, errorMessage } from "../../lib/ipc";
import type { RestorePointEntry } from "../../lib/types";
import { useApp } from "../../store/app";

export function RestorePointsCard() {
  const { m, lang } = useI18n();
  const toast = useApp((s) => s.toast);
  const restore = useApp((s) => s.restore);
  const setRestore = useApp((s) => s.setRestore);
  const [points, setPoints] = useState<RestorePointEntry[] | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [target, setTarget] = useState<RestorePointEntry | null>(null);
  const [askRestart, setAskRestart] = useState(false);

  const load = useCallback(() => {
    void api
      .restorePoints()
      .then(setPoints)
      .catch(() => setPoints([]));
  }, []);
  useEffect(load, [load]);

  const run = async (key: string, fn: () => Promise<void>) => {
    setBusy(key);
    try {
      await fn();
    } catch (e) {
      toast("error", errorMessage(e));
    } finally {
      setBusy(null);
    }
  };

  const date = (iso: string) => {
    const d = new Date(iso);
    return Number.isNaN(d.getTime()) ? iso : d.toLocaleString(lang, { dateStyle: "medium", timeStyle: "short" });
  };
  const disabled = restore !== null && !restore.enabled;

  return (
    <Card id="set-restore-points">
      <CardHeader
        icon={<History className="size-[18px]" />}
        tone="emerald"
        title={m.restorePoints.title}
        description={m.restorePoints.desc}
        actions={
          disabled ? (
            <Button
              size="sm"
              variant="primary"
              loading={busy === "enable"}
              onClick={() => void run("enable", async () => setRestore(await api.restoreEnable()))}
            >
              {m.overview.enableProtection}
            </Button>
          ) : (
            <Button
              size="sm"
              icon={<ShieldPlus className="size-3.5" />}
              loading={busy === "create"}
              onClick={() =>
                void run("create", async () => {
                  setRestore(await api.restoreCreate("Aether"));
                  toast("success", m.restorePoints.created);
                  load();
                })
              }
            >
              {m.restorePoints.create}
            </Button>
          )
        }
      />
      <div className="border-t border-white/[0.05]">
        {points === null ? (
          <div className="space-y-2 p-5">
            <Skeleton className="h-10" />
            <Skeleton className="h-10" />
          </div>
        ) : points.length === 0 ? (
          <p className="px-6 py-5 text-[13px] text-[var(--text-3)]">{disabled ? m.restorePoints.disabled : m.restorePoints.empty}</p>
        ) : (
          <ul className="divide-y divide-white/[0.05]">
            {points.map((p) => (
              <li key={p.sequence} className="flex flex-wrap items-center gap-3 px-6 py-3">
                <div className="min-w-0 flex-1">
                  <div className="truncate text-[13.5px] text-zinc-100">{p.description || m.restorePoints.noName}</div>
                  <div className="text-[12px] text-[var(--text-3)]">{date(p.created)}</div>
                </div>
                <Button size="sm" variant="ghost" icon={<RotateCcw className="size-3.5" />} onClick={() => setTarget(p)}>
                  {m.restorePoints.restore}
                </Button>
              </li>
            ))}
          </ul>
        )}
      </div>

      <ConfirmDialog
        open={target !== null}
        title={m.restorePoints.confirmTitle}
        body={
          target ? fmt(m.restorePoints.confirmBody, { name: target.description || m.restorePoints.noName, date: date(target.created) }) : ""
        }
        confirmLabel={m.restorePoints.restore}
        danger
        onClose={() => setTarget(null)}
        onConfirm={() => {
          const p = target;
          if (!p) return;
          void run("restore", async () => {
            await api.restoreToPoint(p.sequence);
            setAskRestart(true);
          });
        }}
      />
      <ConfirmDialog
        open={askRestart}
        title={m.restorePoints.restartTitle}
        body={m.restorePoints.restartBody}
        confirmLabel={m.restorePoints.restartNow}
        onClose={() => setAskRestart(false)}
        onConfirm={() => void run("restart", () => api.systemRestart())}
      />
    </Card>
  );
}
