// Fichiers en double (contenu identique) dans les dossiers personnels, envoyés à la Corbeille un par un.
import { useState } from "react";
import { Copy, FolderOpen, ScanSearch, Trash2 } from "lucide-react";
import { Button, Card, CardHeader, Empty } from "../../components/ui";
import { fmt, useI18n } from "../../i18n";
import { formatBytes, formatDate } from "../../lib/format";
import { api, errorMessage } from "../../lib/ipc";
import type { DuplicateGroup } from "../../lib/types";
import { useApp } from "../../store/app";

export function DuplicatesCard() {
  const { m, lang } = useI18n();
  const toast = useApp((s) => s.toast);
  const [groups, setGroups] = useState<DuplicateGroup[] | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  const scan = async () => {
    setBusy("scan");
    try {
      setGroups(await api.storageDuplicates());
    } catch (e) {
      toast("error", errorMessage(e));
    } finally {
      setBusy(null);
    }
  };

  const trash = async (path: string) => {
    setBusy(path);
    try {
      await api.storageTrashFile(path);
      // Le groupe disparaît dès qu'il ne reste qu'un exemplaire.
      setGroups((gs) =>
        (gs ?? [])
          .map((g) => ({ ...g, files: g.files.filter((f) => f.path !== path), wasted: g.wasted - g.size }))
          .filter((g) => g.files.length > 1),
      );
      toast("success", m.diagnostics.dupTrashed);
    } catch (e) {
      toast("error", errorMessage(e));
    } finally {
      setBusy(null);
    }
  };

  const total = (groups ?? []).reduce((s, g) => s + g.wasted, 0);

  return (
    <Card id="st-duplicates">
      <CardHeader
        title={m.diagnostics.dupTitle}
        description={m.diagnostics.dupDesc}
        actions={
          <Button size="sm" loading={busy === "scan"} icon={<ScanSearch className="size-3.5" />} onClick={() => void scan()}>
            {groups ? m.scan.analyze : m.diagnostics.dupScan}
          </Button>
        }
      />
      {groups === null ? (
        <Empty>{m.diagnostics.dupHint}</Empty>
      ) : groups.length === 0 ? (
        <Empty>{m.diagnostics.dupNone}</Empty>
      ) : (
        <div className="border-t border-white/[0.05]">
          <p className="px-6 pt-3 text-[12.5px] text-zinc-400">
            {fmt(m.diagnostics.dupSummary, { count: groups.length, size: formatBytes(total, lang) })}
          </p>
          <div className="scroll-area max-h-[460px] divide-y divide-white/[0.05]">
            {groups.map((g) => (
              <div key={g.files[0]?.path} className="px-6 py-3">
                <div className="mb-1.5 flex items-center gap-2 text-[12.5px] text-zinc-500">
                  <Copy className="size-3.5" aria-hidden />
                  {fmt(m.diagnostics.dupGroup, { count: g.files.length, size: formatBytes(g.size, lang) })}
                </div>
                {g.files.map((f, i) => (
                  <div key={f.path} className="flex min-w-0 items-center gap-3 py-1">
                    <div className="min-w-0 flex-1">
                      <div className="truncate text-[13px] text-zinc-200" title={f.path}>
                        {f.path}
                      </div>
                      <div className="text-[11.5px] text-zinc-500">
                        {formatDate(f.modified, lang)}
                        {i === 0 && ` · ${m.diagnostics.dupNewest}`}
                      </div>
                    </div>
                    <button
                      type="button"
                      onClick={() => void api.storageReveal(f.path)}
                      title={m.diagnostics.dupReveal}
                      aria-label={m.diagnostics.dupReveal}
                      className="grid size-8 place-items-center rounded-[9px] text-zinc-500 transition-colors hover:bg-white/[0.05] hover:text-zinc-200"
                    >
                      <FolderOpen className="size-4" aria-hidden />
                    </button>
                    <Button
                      size="sm"
                      variant="ghost"
                      loading={busy === f.path}
                      disabled={busy !== null && busy !== f.path}
                      icon={<Trash2 className="size-3.5" />}
                      onClick={() => void trash(f.path)}
                    >
                      {m.diagnostics.dupTrash}
                    </Button>
                  </div>
                ))}
              </div>
            ))}
          </div>
        </div>
      )}
    </Card>
  );
}
