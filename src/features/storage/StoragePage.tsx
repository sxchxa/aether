// Section « Stockage & Caches ».
import { useCallback, useEffect, useMemo, useState } from "react";
import { Brush, FolderOpen, Gamepad2, Globe, HardDrive, Recycle, ScanSearch, Trash2 } from "lucide-react";
import { ConfirmDialog, Modal } from "../../components/Overlays";
import { Button, Card, CardHeader, CheckMark, cx, Empty, PageHeader, RiskBadge, Skeleton } from "../../components/ui";
import { fmt, useI18n } from "../../i18n";
import { api, errorMessage } from "../../lib/ipc";
import { formatBytes, formatDate } from "../../lib/format";
import type { AnalyzerProgress, CleanCategory, CleanRule, LargeFile, LastRun, RunningApp, ScanResult } from "../../lib/types";
import { TweakList } from "../../components/TweakCard";
import { DiskHealthCard } from "./DiskHealthCard";
import { DuplicatesCard } from "./DuplicatesCard";
import { useApp } from "../../store/app";

const CATEGORY_ICON: Record<CleanCategory, typeof Brush> = { system: Brush, browsers: Globe, graphics: Gamepad2 };
const CATEGORIES: CleanCategory[] = ["system", "browsers", "graphics"];

/** Analyse pas encore disponible : référence stable entre deux affichages. */
const NO_SCAN: Record<string, ScanResult> = {};

export function StoragePage() {
  const { m, lang } = useI18n();
  const toast = useApp((s) => s.toast);
  const inspect = useApp((s) => s.inspect);
  const rules = useApp((s) => s.rules);
  const loadRules = useApp((s) => s.loadRules);
  const scanCache = useApp((s) => s.scan);
  const scanning = useApp((s) => s.scanning);
  const storeScan = useApp((s) => s.runScan);
  const scan: Record<string, ScanResult> = scanCache ?? NO_SCAN;
  const [selected, setSelected] = useState<Set<string> | null>(null);
  const [cleaning, setCleaning] = useState(false);
  const [blockers, setBlockers] = useState<RunningApp[] | null>(null);
  const runScan = useCallback(() => storeScan(true), [storeScan]);

  useEffect(() => {
    void loadRules();
    void storeScan();
  }, [loadRules, storeScan]);

  // Sélection par défaut calculée une fois les règles connues.
  const effective: Set<string> = useMemo(
    () => selected ?? new Set((rules ?? []).filter((r: CleanRule) => r.defaultSelected && r.roots.length > 0).map((r) => r.id)),
    [selected, rules],
  );

  const total = useMemo(() => [...effective].reduce((sum, id) => sum + (scan[id]?.bytes ?? 0), 0), [effective, scan]);

  const doClean = async (closeFirst: string[]) => {
    setBlockers(null);
    setCleaning(true);
    try {
      if (closeFirst.length) await api.storageCloseApps(closeFirst);
      const summary = await api.storageClean([...effective]);
      toast(
        "success",
        fmt(summary.preview ? m.storage.resultPreview : m.storage.result, {
          files: summary.removed,
          size: formatBytes(summary.freed, lang),
          skipped: summary.skipped,
        }),
      );
      if (!summary.preview) await runScan();
    } catch (e) {
      toast("error", errorMessage(e));
    } finally {
      setCleaning(false);
    }
  };

  const startClean = async () => {
    try {
      const running = await api.storageBlockers([...effective]);
      if (running.length) setBlockers(running);
      else await doClean([]);
    } catch (e) {
      toast("error", errorMessage(e));
    }
  };

  const toggle = (id: string) => {
    const next = new Set(effective);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    setSelected(next);
  };

  return (
    <div className="space-y-10">
      <PageHeader
        section="storage"
        title={m.storage.title}
        subtitle={m.storage.subtitle}
        actions={
          <>
            <Button icon={<ScanSearch className="size-4" />} loading={scanning} onClick={() => void runScan()}>
              {scanning ? m.storage.scanning : m.storage.scan}
            </Button>
            <Button
              variant="primary"
              icon={<Recycle className="size-4" />}
              loading={cleaning}
              disabled={effective.size === 0}
              onClick={() => void startClean()}
            >
              {m.storage.cleanSelected} · {formatBytes(total, lang)}
            </Button>
          </>
        }
      />

      <div className="grid grid-cols-1 items-start gap-10">
        {CATEGORIES.map((cat) => {
          const Icon = CATEGORY_ICON[cat];
          const list = (rules ?? []).filter((r) => r.category === cat);
          const catBytes = list.reduce((s, r) => s + (scan[r.id]?.bytes ?? 0), 0);
          return (
            <Card key={cat} id={`st-${cat}`} className="flex flex-col">
              <CardHeader
                icon={<Icon className="size-[18px]" />}
                tone={cat === "system" ? "violet" : cat === "browsers" ? "sky" : "indigo"}
                title={m.storage.categories[cat]}
                description={m.storage.categoryDesc[cat]}
                actions={
                  <span className="tabular text-[15px] font-semibold text-white">
                    {scanning && !catBytes ? "…" : formatBytes(catBytes, lang)}
                  </span>
                }
              />
              <div className="flex-1 space-y-1 px-3 pb-4">
                {!rules && [0, 1, 2].map((k) => <Skeleton key={k} className="mx-2 h-11" />)}
                {list.map((r) => {
                  const res = scan[r.id];
                  const present = r.roots.length > 0;
                  const text = (m.rules as Record<string, { title: string; desc: string } | undefined>)[r.id];
                  return (
                    <div
                      key={r.id}
                      role="checkbox"
                      aria-checked={effective.has(r.id)}
                      aria-disabled={!present}
                      tabIndex={present ? 0 : -1}
                      onClick={() => present && toggle(r.id)}
                      onKeyDown={(e) => {
                        if (present && (e.key === " " || e.key === "Enter")) {
                          e.preventDefault();
                          toggle(r.id);
                        }
                      }}
                      className={cx(
                        "flex min-w-0 items-center gap-3 overflow-hidden rounded-[8px] px-3 py-2.5 transition-colors",
                        present ? "cursor-pointer hover:bg-white/[0.035]" : "cursor-default opacity-45",
                      )}
                    >
                      <CheckMark checked={effective.has(r.id)} disabled={!present} />
                      <div className="min-w-0 flex-1">
                        <div className="flex min-w-0 items-center gap-2">
                          <span className="min-w-0 truncate text-[13px] text-zinc-100">{text?.title ?? r.id}</span>
                          {r.risk !== "safe" && (
                            <span className="shrink-0">
                              <RiskBadge risk={r.risk} />
                            </span>
                          )}
                        </div>
                        {!present && <div className="truncate text-[11.5px] text-[var(--text-3)]">{m.storage.notFound}</div>}
                      </div>
                      <div className="shrink-0 text-right">
                        <div className="tabular text-[12.5px] font-medium text-slate-200">
                          {res ? formatBytes(res.bytes, lang) : present ? "…" : "—"}
                        </div>
                        {res && res.files > 0 && (
                          <div className="tabular text-[10.5px] text-[var(--text-3)]">
                            {res.files} {m.common.files}
                          </div>
                        )}
                      </div>
                    </div>
                  );
                })}
              </div>
            </Card>
          );
        })}
      </div>

      <MaintenanceCard />

      <DiskHealthCard />

      <DuplicatesCard />

      <Analyzer
        onHelp={() => inspect({ kind: "action", id: "analyzer", technical: "walkdir (≥ 100 Mo, hors %WINDIR%) → trash::delete" })}
      />

      <Modal
        open={blockers !== null}
        title={m.storage.blockersTitle}
        onClose={() => setBlockers(null)}
        footer={
          <>
            <Button variant="ghost" onClick={() => setBlockers(null)}>
              {m.common.cancel}
            </Button>
            <Button onClick={() => void doClean([])}>{m.storage.cleanAnyway}</Button>
            <Button variant="primary" onClick={() => void doClean((blockers ?? []).map((b) => b.process))}>
              {m.storage.closeApps}
            </Button>
          </>
        }
      >
        <p>{m.storage.blockersDesc}</p>
        <ul className="mt-3 space-y-1.5">
          {(blockers ?? []).map((b) => (
            <li
              key={b.process}
              className="flex items-center justify-between rounded-lg border border-white/[0.06] bg-white/[0.02] px-3 py-2 font-mono text-[12px] text-slate-200"
            >
              {b.process}
              <span className="text-[var(--text-3)]">×{b.instances}</span>
            </li>
          ))}
        </ul>
      </Modal>
    </div>
  );
}

function Analyzer({ onHelp }: { onHelp: () => void }) {
  const { m, lang } = useI18n();
  const toast = useApp((s) => s.toast);
  const [files, setFiles] = useState<LargeFile[] | null>(null);
  const [progress, setProgress] = useState<AnalyzerProgress | null>(null);
  const [running, setRunning] = useState(false);
  const [confirm, setConfirm] = useState<LargeFile | null>(null);

  const run = async () => {
    setRunning(true);
    setProgress(null);
    try {
      setFiles(await api.storageAnalyze(setProgress));
    } catch (e) {
      toast("error", errorMessage(e));
    } finally {
      setRunning(false);
    }
  };

  const trash = async (file: LargeFile) => {
    try {
      const done = await api.storageTrashFile(file.path);
      if (done) setFiles((f) => (f ?? []).filter((x) => x.path !== file.path));
      else toast("info", m.toast.previewOnly);
    } catch (e) {
      toast("error", errorMessage(e));
    }
  };

  const max = files?.[0]?.size ?? 1;

  return (
    <Card id="st-analyzer">
      <CardHeader
        icon={<HardDrive className="size-[18px]" />}
        tone="amber"
        title={m.storage.analyzer}
        description={m.storage.analyzerDesc}
        onHelp={onHelp}
        actions={
          <Button icon={<ScanSearch className="size-4" />} loading={running} onClick={() => void run()}>
            {m.storage.analyzerRun}
          </Button>
        }
      />
      {running && (
        <Empty>
          {progress
            ? fmt(m.storage.analyzerProgress, { files: progress.filesScanned.toLocaleString(lang), found: progress.found })
            : m.common.running}
        </Empty>
      )}
      {files && files.length === 0 && !running && <Empty>{m.storage.noLargeFiles}</Empty>}
      {files && files.length > 0 && (
        <div className="scroll-area max-h-[360px] px-3 pb-4">
          {files.map((f) => (
            <div key={f.path} className="group flex items-center gap-3 rounded-xl px-2.5 py-2 hover:bg-white/[0.04]">
              <div className="min-w-0 flex-1">
                <div className="selectable truncate text-[12.5px] text-slate-100" title={f.path}>
                  {f.path}
                </div>
                <div className="mt-1 flex items-center gap-3">
                  <div className="h-1 flex-1 overflow-hidden rounded-full bg-[var(--track)]">
                    <div className="h-full rounded-full bg-sky" style={{ width: `${(f.size / max) * 100}%` }} />
                  </div>
                  <span className="text-[10.5px] text-[var(--text-3)]">{formatDate(f.modified, lang)}</span>
                </div>
              </div>
              <span className="tabular w-20 text-right text-[12.5px] font-medium text-white">{formatBytes(f.size, lang)}</span>
              <div className="flex gap-1 opacity-0 transition-opacity group-hover:opacity-100">
                <Button
                  size="sm"
                  variant="ghost"
                  icon={<FolderOpen className="size-3.5" />}
                  onClick={() => void api.storageReveal(f.path).catch((e: unknown) => toast("error", errorMessage(e)))}
                >
                  {m.storage.reveal}
                </Button>
                <Button size="sm" variant="danger" icon={<Trash2 className="size-3.5" />} onClick={() => setConfirm(f)}>
                  {m.storage.trash}
                </Button>
              </div>
            </div>
          ))}
        </div>
      )}
      <ConfirmDialog
        open={confirm !== null}
        title={m.storage.trash}
        body={<span className="selectable break-all font-mono text-[12px]">{confirm?.path}</span>}
        confirmLabel={m.storage.trash}
        danger
        onConfirm={() => {
          if (confirm) void trash(confirm);
        }}
        onClose={() => setConfirm(null)}
      />
    </Card>
  );
}

/** Entretien automatique hebdomadaire (tâche planifiée) et résultat du dernier passage. */
function MaintenanceCard() {
  const { m, lang } = useI18n();
  const ensureTweaks = useApp((s) => s.ensureTweaks);
  const [last, setLast] = useState<LastRun | null>(null);
  useEffect(() => {
    void ensureTweaks(["maint.weekly"]);
    void api
      .maintenanceLast()
      .then(setLast)
      .catch(() => setLast(null));
  }, [ensureTweaks]);
  return (
    <Card id="st-maintenance">
      <CardHeader title={m.storage.maintenance} description={m.storage.maintenanceDesc} />
      <TweakList ids={["maint.weekly"]} />
      <p className="px-6 pb-4 text-[12px] text-zinc-500">
        {last ? fmt(m.storage.lastRun, { date: formatDate(last.ts, lang), size: formatBytes(last.freed, lang) }) : m.storage.neverRun}
      </p>
    </Card>
  );
}
