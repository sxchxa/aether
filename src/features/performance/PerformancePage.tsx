// Section « Performance & Jeux » (le réglage de l'énergie vit dans la section Alimentation).
import { useEffect, useState } from "react";
import { Check, Gamepad2, HardDriveDownload, History, MemoryStick, Monitor, Server, Sparkles } from "lucide-react";
import { TweakList } from "../../components/TweakCard";
import { Button, Card, CardHeader, cx, Empty, IconBadge, PageHeader } from "../../components/ui";
import { fmt, useI18n } from "../../i18n";
import { api, errorMessage } from "../../lib/ipc";
import { formatBytes } from "../../lib/format";
import type { DiskInfo, DisplayInfo, RamReport } from "../../lib/types";
import { useApp } from "../../store/app";

export const PERFORMANCE_TWEAKS = {
  services: ["svc.spooler", "svc.diagtrack", "svc.wsearch", "svc.sysmain", "svc.mapsBroker", "svc.fax"],
  gaming: [
    "gaming.gameMode",
    "gaming.windowedOpt",
    "gaming.mouse",
    "gaming.xboxDvr",
    "gaming.stickyKeys",
    "gaming.hags",
    "gaming.mmcss",
    "mem.svchostSplit",
    "gaming.gameBarButton",
    "gaming.broadcast",
    "gaming.fsoOff",
  ],
  visual: [
    "ui.menuDelay",
    "ui.startupDelay",
    "ui.endTask",
    "ui.aeroShake",
    "ui.fileExtensions",
    "ui.darkMode",
    "ui.hiddenFiles",
    "ui.launchThisPc",
    "ui.verboseStatus",
    "ui.transparency",
  ],
  fs: ["fs.storageSense", "fs.lastAccess"],
  fx: [
    "fx.minMax",
    "fx.controls",
    "fx.menus",
    "fx.tooltips",
    "fx.combo",
    "fx.selectionFade",
    "fx.dragContents",
    "fx.windowShadow",
    "fx.cursorShadow",
    "fx.taskbarAnim",
    "fx.peek",
    "fx.iconShadow",
    "fx.selectionRect",
    "fx.thumbnails",
  ],
};

export function PerformancePage() {
  const { m, lang } = useI18n();
  const loadTweaks = useApp((s) => s.loadTweaks);
  const ensureTweaks = useApp((s) => s.ensureTweaks);
  const refreshHistory = useApp((s) => s.refreshHistory);
  const toast = useApp((s) => s.toast);
  const inspect = useApp((s) => s.inspect);
  const info = useApp((s) => s.systemInfo);
  const loadSystemInfo = useApp((s) => s.loadSystemInfo);
  const [ram, setRam] = useState<RamReport | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const display: DisplayInfo | null = info?.display ?? null;
  const systemDisk: DiskInfo | null = info?.disks.find((d) => d.mount.toUpperCase().startsWith("C:")) ?? null;

  useEffect(() => {
    void ensureTweaks([
      ...PERFORMANCE_TWEAKS.services,
      ...PERFORMANCE_TWEAKS.gaming,
      ...PERFORMANCE_TWEAKS.visual,
      ...PERFORMANCE_TWEAKS.fx,
      ...PERFORMANCE_TWEAKS.fs,
    ]);
    void loadSystemInfo();
  }, [ensureTweaks, loadSystemInfo]);

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

  return (
    <div className="space-y-10">
      <PageHeader section="performance" title={m.performance.title} subtitle={m.performance.subtitle} />

      <div className="grid grid-cols-1 items-start gap-10">
        <Card id="pf-services" className="">
          <CardHeader
            icon={<Server className="size-[18px]" />}
            tone="indigo"
            title={m.performance.services}
            description={m.performance.servicesDesc}
            actions={
              <Button
                size="sm"
                icon={<History className="size-3.5" />}
                loading={busy === "services"}
                onClick={() =>
                  void run("services", async () => {
                    const restored = await api.servicesRestoreAll();
                    toast("success", fmt(m.performance.restoredServices, { count: restored.length }));
                    await loadTweaks(PERFORMANCE_TWEAKS.services);
                    await refreshHistory();
                  })
                }
              >
                {m.performance.restoreServices}
              </Button>
            }
          />
          <TweakList ids={PERFORMANCE_TWEAKS.services} />
        </Card>

        <div className="space-y-10">
          <Card id="pf-gaming" className="">
            <CardHeader icon={<Gamepad2 className="size-[18px]" />} tone="violet" title={m.performance.gaming} />
            <TweakList ids={PERFORMANCE_TWEAKS.gaming} />
            <div className="surface-2 mx-5 mb-5 flex items-center gap-3.5 rounded-[10px] px-4 py-3">
              <IconBadge tone={display?.upgradeAvailable ? "amber" : "sky"}>
                <Monitor className="size-[18px]" aria-hidden />
              </IconBadge>
              <div className="min-w-0 flex-1">
                <div className="text-[13.5px] font-semibold text-white">{m.performance.refreshRate}</div>
                <div className={cx("text-[11.5px]", display?.upgradeAvailable ? "text-amber-300" : "text-[var(--text-3)]")}>
                  {display
                    ? display.upgradeAvailable
                      ? fmt(m.performance.refreshUpgrade, { max: display.maxHz })
                      : `${fmt(m.performance.refreshCurrent, { current: display.currentHz })} · ${m.performance.refreshOk}`
                    : m.common.loading}
                </div>
              </div>
              {display?.upgradeAvailable && (
                <Button size="sm" onClick={() => void api.openSettingsPage("ms-settings:display-advanced")}>
                  {m.performance.openDisplay}
                </Button>
              )}
            </div>
          </Card>

          <Card id="pf-visual" className="">
            <CardHeader title={m.performance.visual} />
            <TweakList ids={PERFORMANCE_TWEAKS.visual} />
          </Card>

          <Card id="pf-fs" className="">
            <CardHeader title={m.performance.fs} />
            <TweakList ids={PERFORMANCE_TWEAKS.fs} />
          </Card>

          <Card id="pf-fx" className="">
            <CardHeader
              title={m.performance.fx}
              actions={
                <Button
                  size="sm"
                  icon={<Sparkles className="size-3.5" />}
                  loading={busy === "fx"}
                  onClick={() =>
                    void run("fx", async () => {
                      await api.tweaksOptimizeMany(PERFORMANCE_TWEAKS.fx);
                      await loadTweaks(PERFORMANCE_TWEAKS.fx);
                      await refreshHistory();
                      toast("success", m.performance.fxDone);
                    })
                  }
                >
                  {m.performance.fxAll}
                </Button>
              }
            />
            <TweakList ids={PERFORMANCE_TWEAKS.fx} />
          </Card>

          <Card id="pf-ram" className="">
            <CardHeader
              icon={<MemoryStick className="size-[18px]" />}
              tone="sky"
              title={m.performance.ram}
              description={m.performance.ramDesc}
              onHelp={() =>
                inspect({
                  kind: "action",
                  id: "ram",
                  technical: "OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_SET_QUOTA)\nK32EmptyWorkingSet(hProcess)",
                })
              }
              actions={
                <Button
                  variant="primary"
                  size="sm"
                  loading={busy === "ram"}
                  onClick={() => void run("ram", async () => setRam(await api.ramPurge()))}
                >
                  {m.performance.ramRun}
                </Button>
              }
            />
            {ram && (
              <Empty>
                <span className="inline-flex items-center gap-1.5 rounded-full border border-emerald/30 bg-emerald/10 px-3 py-1 text-emerald-300">
                  <Check className="size-3.5" aria-hidden />
                  {fmt(m.performance.ramResult, { size: formatBytes(ram.freed, lang), count: ram.trimmed })}
                </span>
              </Empty>
            )}
          </Card>

          <Card id="pf-trim" className="">
            <CardHeader
              icon={<HardDriveDownload className="size-[18px]" />}
              tone="emerald"
              title={m.performance.trim}
              description={m.performance.trimDesc}
              onHelp={() => inspect({ kind: "action", id: "trim", technical: "Optimize-Volume -DriveLetter C -ReTrim -Verbose" })}
              actions={
                systemDisk?.kind === "ssd" ? (
                  <Button
                    size="sm"
                    loading={busy === "trim"}
                    onClick={() =>
                      void run("trim", async () => {
                        const out = await api.ssdTrim("C");
                        toast("success", out.split("\n").slice(-1)[0] || m.common.done);
                      })
                    }
                  >
                    {fmt(m.performance.trimRun, { drive: "C:" })}
                  </Button>
                ) : undefined
              }
            />
            {systemDisk && systemDisk.kind !== "ssd" && <Empty>{m.performance.trimNoSsd}</Empty>}
          </Card>
        </div>
      </div>
    </div>
  );
}
