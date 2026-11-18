// Section « Système & Réparation ».
import { useEffect, useRef, useState } from "react";
import {
  Cloud,
  Globe,
  LayoutGrid,
  ListTree,
  PackageX,
  Power,
  RefreshCw,
  ShieldCheck,
  Stethoscope,
  TriangleAlert,
  UserLock,
} from "lucide-react";
import { ConfirmDialog } from "../../components/Overlays";
import { TweakList } from "../../components/TweakCard";
import { Win11Card } from "./Win11Card";
import { Button, Card, CardHeader, cx, Empty, PageHeader, Skeleton, Toggle } from "../../components/ui";
import { fmt, useI18n } from "../../i18n";
import { api, errorMessage } from "../../lib/ipc";
import type { EdgeStatus, InstalledApp, OneDriveStatus, RepairTool, StartupEntry, StartupImpact } from "../../lib/types";
import { useApp } from "../../store/app";

export const SYSTEM_TWEAKS = {
  update: ["wu.restartNotify", "wu.activeHours", "wu.noDrivers", "wu.deferFeature"],
  contextMenu: ["ui.classicMenu"],
  edge: ["edge.neutralize"],
  privacy: [
    "privacy.bingSearch",
    "privacy.startRecommendations",
    "privacy.tips",
    "privacy.advertisingId",
    "privacy.activity",
    "privacy.telemetry",
    "privacy.feedback",
    "privacy.tailored",
    "privacy.appTracking",
    "privacy.inking",
    "privacy.onlineSpeech",
    "privacy.copilot",
    "privacy.widgets",
    "privacy.recall",
    "privacy.backgroundApps",
  ],
};

interface PendingConfirm {
  title: string;
  body: string;
  action: () => Promise<void>;
}

export function SystemPage() {
  const { m } = useI18n();
  const ensureTweaks = useApp((s) => s.ensureTweaks);
  const refreshHistory = useApp((s) => s.refreshHistory);
  const toast = useApp((s) => s.toast);
  const inspect = useApp((s) => s.inspect);
  const expert = useApp((s) => s.expert);
  const [apps, setApps] = useState<InstalledApp[] | null>(null);
  const [startup, setStartup] = useState<StartupEntry[] | null>(null);
  const [onedrive, setOnedrive] = useState<OneDriveStatus | null>(null);
  const [edge, setEdge] = useState<EdgeStatus | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [confirm, setConfirm] = useState<PendingConfirm | null>(null);

  const loadApps = () =>
    void api
      .appxList()
      .then(setApps)
      .catch(() => setApps([]));
  const loadStartup = () =>
    void api
      .startupList()
      .then(setStartup)
      .catch(() => setStartup([]));
  const [impacts, setImpacts] = useState<StartupImpact[]>([]);
  useEffect(() => {
    void api
      .startupImpacts()
      .then(setImpacts)
      .catch(() => setImpacts([]));
  }, []);
  /** Retard au démarrage d'un programme, retrouvé par le nom de son exécutable dans la commande. */
  const impactOf = (command: string) => impacts.find((i) => command.toLowerCase().includes(i.file))?.degradationMs ?? null;
  const loadOneDrive = () =>
    void api
      .onedriveStatus()
      .then(setOnedrive)
      .catch(() => setOnedrive(null));
  const loadEdge = () =>
    void api
      .edgeStatus()
      .then(setEdge)
      .catch(() => setEdge(null));

  useEffect(() => {
    void ensureTweaks([...SYSTEM_TWEAKS.update, ...SYSTEM_TWEAKS.contextMenu, ...SYSTEM_TWEAKS.edge, ...SYSTEM_TWEAKS.privacy]);
    loadApps();
    loadStartup();
    loadOneDrive();
    loadEdge();
  }, [ensureTweaks]);

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

  const ask = (title: string, action: () => Promise<void>) => setConfirm({ title, body: m.system.confirmIrreversible, action });

  return (
    <div className="space-y-10">
      <PageHeader section="system" title={m.system.title} subtitle={m.system.subtitle} />

      <div className="grid grid-cols-1 items-start gap-10">
        <Win11Card />

        <Card id="sy-apps" className="">
          <CardHeader
            icon={<LayoutGrid className="size-[18px]" />}
            tone="rose"
            title={m.system.apps}
            description={m.system.appsDesc}
            onHelp={() => inspect({ kind: "action", id: "appx", technical: "Get-AppxPackage -Name <paquet> | Remove-AppxPackage" })}
          />
          <div className="space-y-1 px-3 pb-4">
            {!apps && [0, 1, 2].map((k) => <Skeleton key={k} className="mx-2 h-10" />)}
            {apps?.length === 0 && <Empty>{m.system.appsNone}</Empty>}
            {apps?.map((a) => (
              <div key={a.package} className="flex min-w-0 items-center gap-3 rounded-xl px-2.5 py-2 hover:bg-white/[0.04]">
                <PackageX className="size-4 text-slate-500" aria-hidden />
                <div className="min-w-0 flex-1">
                  <div className="truncate text-[13px] text-slate-100">
                    {(m.apps as Record<string, string | undefined>)[a.id] ?? a.package}
                  </div>
                  <div className="truncate font-mono text-[10.5px] text-[var(--text-3)]">
                    {a.package} · {a.version}
                  </div>
                </div>
                <Button
                  size="sm"
                  variant="ghost"
                  loading={busy === `appx-${a.id}`}
                  onClick={() =>
                    ask(`${m.system.remove} ${(m.apps as Record<string, string | undefined>)[a.id] ?? a.package}`, () =>
                      run(`appx-${a.id}`, async () => {
                        const done = await api.appxRemove(a.id);
                        if (done) {
                          // Retrait immédiat de la liste ; le backend a vérifié la désinstallation.
                          setApps((list) => (list ?? []).filter((x) => x.package !== a.package));
                          toast("success", `${(m.apps as Record<string, string | undefined>)[a.id] ?? a.package} : ${m.outcome.applied}`);
                        } else {
                          toast("warn", m.dryRun.simulated);
                        }
                      }),
                    )
                  }
                >
                  {m.system.remove}
                </Button>
              </div>
            ))}
          </div>
        </Card>

        <Card id="sy-startup" className="">
          <CardHeader
            icon={<Power className="size-[18px]" />}
            tone="amber"
            title={m.system.startup}
            description={m.system.startupDesc}
            onHelp={() =>
              inspect({
                kind: "action",
                id: "startup",
                technical:
                  "reg.exe export HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run aether_startup_backup.reg\nHKCU\\…\\Explorer\\StartupApproved\\Run\\<nom> = 02/03 …",
              })
            }
          />
          <div className="space-y-1 px-3 pb-4">
            {!startup && [0, 1].map((k) => <Skeleton key={k} className="mx-2 h-10" />)}
            {startup?.length === 0 && <Empty>{m.system.startupNone}</Empty>}
            {startup?.map((s) => (
              <div key={s.name} className="flex min-w-0 items-center gap-3 rounded-xl px-2.5 py-2 hover:bg-white/[0.04]">
                <div className="min-w-0 flex-1">
                  <div className="flex min-w-0 items-center gap-2">
                    <span className="truncate text-[13px] text-slate-100">{s.name}</span>
                    <ImpactBadge ms={impactOf(s.command)} />
                  </div>
                  <div className="truncate font-mono text-[10.5px] text-[var(--text-3)]" title={s.command}>
                    {s.command}
                  </div>
                </div>
                <Toggle
                  checked={s.enabled}
                  busy={busy === `startup-${s.name}`}
                  label={s.name}
                  onChange={(v) =>
                    void run(`startup-${s.name}`, async () => {
                      const changed = await api.startupSet(s.name, v);
                      if (!changed) toast("info", m.toast.previewOnly);
                      loadStartup();
                      await refreshHistory();
                    })
                  }
                />
              </div>
            ))}
          </div>
        </Card>

        <Card id="sy-update" className="">
          <CardHeader title={m.system.update} />
          <TweakList ids={SYSTEM_TWEAKS.update} />
        </Card>

        <Card id="sy-privacy" className="">
          <CardHeader icon={<UserLock className="size-[18px]" />} tone="violet" title={m.system.privacy} />
          <TweakList ids={SYSTEM_TWEAKS.privacy} />
        </Card>

        <Card id="sy-menu" className="">
          <CardHeader
            icon={<ListTree className="size-[18px]" />}
            tone="sky"
            title={m.system.contextMenu}
            actions={
              <Button
                size="sm"
                variant="ghost"
                icon={<RefreshCw className="size-3.5" />}
                loading={busy === "explorer"}
                onClick={() => void run("explorer", () => api.restartExplorer())}
              >
                {m.system.restartExplorer}
              </Button>
            }
          />
          <TweakList ids={SYSTEM_TWEAKS.contextMenu} />
        </Card>

        {/* Carte affichée seulement si OneDrive est installé (disparaît après sa désinstallation). */}
        {onedrive?.installed && (
          <Card id="sy-onedrive" className="">
            <CardHeader
              icon={<Cloud className="size-[18px]" />}
              tone="sky"
              title={m.system.onedrive}
              description={onedrive ? (onedrive.installed ? m.system.onedriveInstalled : m.system.onedriveAbsent) : m.common.loading}
              onHelp={() =>
                inspect({
                  kind: "action",
                  id: "onedrive",
                  technical: "taskkill /F /IM OneDrive.exe\n%WINDIR%\\SysWOW64\\OneDriveSetup.exe /uninstall",
                })
              }
              actions={
                onedrive?.installed ? (
                  <Button
                    size="sm"
                    variant="danger"
                    loading={busy === "onedrive"}
                    onClick={() =>
                      ask(m.system.onedriveUninstall, () =>
                        run("onedrive", async () => {
                          const done = await api.onedriveUninstall();
                          toast(done ? "success" : "info", done ? m.outcome.applied : m.toast.previewOnly);
                          loadOneDrive();
                        }),
                      )
                    }
                  >
                    {m.system.onedriveUninstall}
                  </Button>
                ) : undefined
              }
            />
          </Card>
        )}

        <Card id="sy-edge" className="">
          <CardHeader
            icon={<Globe className="size-[18px]" />}
            tone="indigo"
            title={m.system.edge}
            onHelp={() =>
              inspect({
                kind: "action",
                id: "edgeUninstall",
                technical:
                  "HKLM\\SOFTWARE\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\Microsoft Edge\\UninstallString (--msedge uniquement, WebView2 exclu)",
              })
            }
          />
          <div className="mx-5 mb-3 flex items-center gap-2 rounded-xl border border-emerald/20 bg-emerald/[0.05] px-3.5 py-2.5 text-[12.5px]">
            {edge?.webview2Installed ? (
              <>
                <ShieldCheck className="size-4 text-emerald" aria-hidden />
                <span className="min-w-0 truncate text-emerald-200">{m.system.edgeWebview}</span>
                <span className="ml-auto shrink-0 font-mono text-[11px] text-[var(--text-3)]">{edge.webview2Version}</span>
              </>
            ) : (
              <>
                <TriangleAlert className="size-4 text-amber" aria-hidden />
                <span className="text-amber-200">{edge ? m.system.edgeWebviewMissing : m.common.loading}</span>
              </>
            )}
          </div>
          <TweakList ids={SYSTEM_TWEAKS.edge} />
          {expert && edge?.browserInstalled && (
            <div className="px-5 pb-5">
              <Button
                size="sm"
                variant="danger"
                disabled={!edge.uninstallAllowed}
                title={edge.uninstallAllowed ? undefined : m.system.edgeUninstallBlocked}
                loading={busy === "edge"}
                onClick={() =>
                  ask(m.system.edgeUninstall, () =>
                    run("edge", async () => {
                      const done = await api.edgeUninstall();
                      toast(done ? "success" : "info", done ? m.outcome.applied : m.toast.previewOnly);
                      loadEdge();
                    }),
                  )
                }
              >
                {m.system.edgeUninstall}
              </Button>
              {!edge.uninstallAllowed && <p className="mt-2 text-[11.5px] text-[var(--text-3)]">{m.system.edgeUninstallBlocked}</p>}
            </div>
          )}
        </Card>

        <RepairCard />
      </div>

      <ConfirmDialog
        open={confirm !== null}
        title={confirm?.title ?? ""}
        body={confirm?.body}
        confirmLabel={m.common.confirm}
        danger
        onConfirm={() => {
          if (confirm) void confirm.action();
        }}
        onClose={() => setConfirm(null)}
      />
    </div>
  );
}

function RepairCard() {
  const { m } = useI18n();
  const toast = useApp((s) => s.toast);
  const inspect = useApp((s) => s.inspect);
  const [lines, setLines] = useState<string[]>([]);
  const [running, setRunning] = useState<RepairTool | null>(null);
  const endRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    endRef.current?.scrollIntoView({ block: "end" });
  }, [lines.length]);

  const start = async (tool: RepairTool) => {
    setRunning(tool);
    setLines([`> ${tool === "sfc" ? "sfc /scannow" : "DISM /Online /Cleanup-Image /RestoreHealth"}`]);
    try {
      await api.repairRun(tool, (e) => {
        if (e.type === "line") {
          // Les barres de progression réécrivent la même ligne : on remplace la précédente.
          setLines((prev) => {
            const last = prev[prev.length - 1] ?? "";
            const isProgress = /\d+(?:[.,]\d+)?\s?%/.test(e.text) && /\d+(?:[.,]\d+)?\s?%/.test(last);
            return isProgress ? [...prev.slice(0, -1), e.text] : [...prev.slice(-400), e.text];
          });
        } else {
          setLines((prev) => [...prev, fmt(m.system.repairDone, { code: e.code ?? "—" })]);
        }
      });
    } catch (err) {
      toast("error", errorMessage(err));
    } finally {
      setRunning(null);
    }
  };

  return (
    <Card id="sy-repair" className="">
      <CardHeader
        icon={<Stethoscope className="size-[18px]" />}
        tone="emerald"
        title={m.system.repair}
        description={m.system.repairDesc}
        onHelp={() => inspect({ kind: "action", id: "sfc", technical: "sfc /scannow\nDISM /Online /Cleanup-Image /RestoreHealth" })}
        actions={
          <>
            <Button size="sm" loading={running === "sfc"} disabled={running !== null} onClick={() => void start("sfc")}>
              {m.system.sfc}
            </Button>
            <Button size="sm" loading={running === "dism"} disabled={running !== null} onClick={() => void start("dism")}>
              {m.system.dism}
            </Button>
          </>
        }
      />
      {lines.length > 0 && (
        <div
          className={cx(
            "scroll-area selectable mx-5 mb-5 max-h-[220px] rounded-xl border border-white/[0.06] bg-black/30 p-3 font-mono text-[11.5px] leading-relaxed text-slate-300",
          )}
        >
          {lines.map((l, i) => (
            <div key={i} className={i === 0 ? "text-cyan" : undefined}>
              {l}
            </div>
          ))}
          <div ref={endRef} />
        </div>
      )}
    </Card>
  );
}

/** Impact au démarrage : élevé (≥ 2 s de retard), moyen (≥ 0,5 s), faible ou non mesuré. */
function ImpactBadge({ ms }: { ms: number | null }) {
  const { m, lang } = useI18n();
  if (ms === null) return <span className="shrink-0 text-[11px] text-zinc-600">{m.diagnostics.impactUnknown}</span>;
  const level = ms >= 2000 ? "high" : ms >= 500 ? "medium" : "low";
  const style =
    level === "high"
      ? "text-rose-300 border-rose-400/25"
      : level === "medium"
        ? "text-amber-300 border-amber-400/25"
        : "text-zinc-400 border-white/[0.08]";
  return (
    <span
      className={cx("shrink-0 rounded-full border px-2 py-0.5 text-[11px]", style)}
      title={fmt(m.diagnostics.impactDelay, { value: (ms / 1000).toLocaleString(lang, { maximumFractionDigits: 1 }) })}
    >
      {m.diagnostics.impact[level]}
    </span>
  );
}
