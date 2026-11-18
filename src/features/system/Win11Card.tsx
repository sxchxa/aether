// Passage de Windows 10 à Windows 11 : affiché uniquement sous Windows 10.
// PC compatible → Assistant d'installation officiel ; sinon → image ISO officielle de Microsoft.
import { useCallback, useEffect, useState } from "react";
import { CircleCheck, CircleX, Download, MonitorUp, RefreshCw } from "lucide-react";
import { ConfirmDialog } from "../../components/Overlays";
import { Button, Card, CardHeader } from "../../components/ui";
import { fmt, useI18n } from "../../i18n";
import { formatBytes } from "../../lib/format";
import { api, errorMessage } from "../../lib/ipc";
import type { Win11Status } from "../../lib/types";
import { useApp } from "../../store/app";

const ISO_PAGE = "https://www.microsoft.com/software-download/windows11";
const MIN_FREE_BYTES = 25 * 1024 ** 3;

export function Win11Card() {
  const { m, lang } = useI18n();
  const toast = useApp((s) => s.toast);
  const [status, setStatus] = useState<Win11Status | null>(null);
  const [busy, setBusy] = useState(false);
  const [confirm, setConfirm] = useState(false);

  const load = useCallback(() => {
    void api
      .win11Status()
      .then(setStatus)
      .catch(() => setStatus(null));
  }, []);
  useEffect(load, [load]);

  // Option réservée à Windows 10 : rien n'est affiché ailleurs.
  if (!status?.isWindows10) return null;

  const enoughSpace = status.freeBytes >= MIN_FREE_BYTES;
  const checks = [
    { ok: status.tpm?.startsWith("2") ?? false, label: fmt(m.win11.checkTpm, { value: status.tpm ?? m.win11.none }) },
    { ok: status.secureBoot, label: m.win11.checkSecureBoot },
    { ok: status.ramOk, label: m.win11.checkRam },
    { ok: enoughSpace, label: fmt(m.win11.checkSpace, { value: formatBytes(status.freeBytes, lang, 0) }) },
  ];

  const upgrade = async () => {
    setBusy(true);
    try {
      if (status.compatible) await api.win11Upgrade();
      else await api.win11UpgradeIso();
      toast("success", m.win11.started);
    } catch (e) {
      toast("error", errorMessage(e));
    } finally {
      setBusy(false);
      load();
    }
  };

  return (
    <Card id="sy-win11">
      <CardHeader
        icon={<MonitorUp className="size-[18px]" />}
        tone="sky"
        title={m.win11.title}
        description={status.compatible ? m.win11.descCompatible : m.win11.descUnsupported}
      />
      <ul className="grid grid-cols-1 gap-2 px-5 pb-4 sm:grid-cols-2">
        {checks.map((c) => (
          <li key={c.label} className="flex min-w-0 items-center gap-2 text-[13px] text-[var(--text-2)]">
            {c.ok ? (
              <CircleCheck className="size-4 shrink-0 text-emerald" aria-hidden />
            ) : (
              <CircleX className="size-4 shrink-0 text-amber" aria-hidden />
            )}
            <span className="min-w-0 break-words">{c.label}</span>
          </li>
        ))}
      </ul>

      {!status.compatible && (
        <ol className="mx-5 mb-4 space-y-2 rounded-[10px] border border-white/[0.06] bg-white/[0.02] p-4 text-[13px] text-[var(--text-2)]">
          <li className="flex flex-wrap items-center justify-between gap-3">
            <span className="min-w-0 flex-1">{m.win11.step1}</span>
            <Button size="sm" icon={<Download className="size-3.5" />} onClick={() => void api.openLink(ISO_PAGE)}>
              {m.win11.openIsoPage}
            </Button>
          </li>
          <li className="flex flex-wrap items-center justify-between gap-3">
            <span className="min-w-0 flex-1 break-words">
              {status.iso ? fmt(m.win11.isoFound, { file: status.iso.split("\\").pop() ?? status.iso }) : m.win11.step2}
            </span>
            <Button size="sm" variant="ghost" icon={<RefreshCw className="size-3.5" />} onClick={load}>
              {m.win11.recheck}
            </Button>
          </li>
        </ol>
      )}

      <div className="flex flex-wrap items-center justify-between gap-3 border-t border-white/[0.05] px-5 py-4">
        <span className="min-w-0 flex-1 text-[12px] text-[var(--text-3)]">{m.win11.keepsFiles}</span>
        <Button
          variant="primary"
          size="sm"
          icon={<MonitorUp className="size-3.5" />}
          loading={busy}
          disabled={!enoughSpace || (!status.compatible && !status.iso)}
          onClick={() => setConfirm(true)}
        >
          {m.win11.action}
        </Button>
      </div>

      <ConfirmDialog
        open={confirm}
        title={m.win11.confirmTitle}
        body={
          <div className="space-y-2">
            <p>{m.win11.confirmBody}</p>
            {!status.compatible && <p className="text-amber-300">{m.win11.unsupportedWarning}</p>}
          </div>
        }
        confirmLabel={m.win11.action}
        onClose={() => setConfirm(false)}
        onConfirm={() => void upgrade()}
      />
    </Card>
  );
}
