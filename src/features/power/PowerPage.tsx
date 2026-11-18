// Section « Alimentation & Énergie ».
import { useEffect, useState } from "react";
import { BatteryWarning, CircleCheck, Monitor, Power, SlidersHorizontal, Zap } from "lucide-react";
import { TweakList } from "../../components/TweakCard";
import { Button, Card, CardHeader, cx, IconBadge, PageHeader } from "../../components/ui";
import { fmt, useI18n } from "../../i18n";
import { api, errorMessage } from "../../lib/ipc";
import { POWER_SETTINGS, SECTION_TWEAKS } from "../../lib/navigation";
import type { PowerScheme } from "../../lib/types";
import { ensureRestorePoint } from "../../lib/safety";
import { BatteryCard } from "./BatteryCard";
import { useApp } from "../../store/app";

export function PowerPage() {
  const { m } = useI18n();
  const ensureTweaks = useApp((s) => s.ensureTweaks);
  const loadTweaks = useApp((s) => s.loadTweaks);
  const refreshHistory = useApp((s) => s.refreshHistory);
  const loadSystemInfo = useApp((s) => s.loadSystemInfo);
  const info = useApp((s) => s.systemInfo);
  const hardwareLoaded = useApp((s) => s.hardwareLoaded);
  const tweaks = useApp((s) => s.tweaks);
  const toast = useApp((s) => s.toast);
  const [schemes, setSchemes] = useState<PowerScheme[]>([]);
  const [maxIds, setMaxIds] = useState<string[]>([]);
  const [busy, setBusy] = useState<string | null>(null);

  const loadSchemes = () =>
    void api
      .powerSchemes()
      .then(setSchemes)
      .catch(() => setSchemes([]));

  useEffect(() => {
    void ensureTweaks(SECTION_TWEAKS.power);
    void loadSystemInfo();
    loadSchemes();
    void api
      .powerMaxIds()
      .then(setMaxIds)
      .catch(() => setMaxIds([]));
  }, [ensureTweaks, loadSystemInfo]);

  const laptop = Boolean(info?.battery);
  const maxStatus = maxIds.map((id) => tweaks[id]).filter((t) => t?.probe.available);
  const maxDone = maxStatus.filter((t) => t?.probe.applied).length;
  const allMax = maxStatus.length > 0 && maxDone === maxStatus.length;

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
      <PageHeader section="power" title={m.power.title} subtitle={m.power.subtitle} />

      <section id="pw-max" className="hero-card anchor-card">
        <div className="relative z-10 flex flex-wrap items-center gap-8 px-9 py-7">
          <IconBadge tone="amber" size="lg">
            <Zap className="size-6" aria-hidden />
          </IconBadge>
          <div className="min-w-0 flex-1">
            <div
              className={cx(
                "inline-flex items-center gap-2 rounded-full border px-3 py-1 text-[11px] font-semibold uppercase tracking-[0.12em]",
                !hardwareLoaded
                  ? "border-white/10 text-slate-400"
                  : laptop
                    ? "border-amber/30 bg-amber/10 text-amber-300"
                    : "border-emerald/30 bg-emerald/10 text-emerald-300",
              )}
            >
              {laptop ? <BatteryWarning className="size-3.5" /> : <Monitor className="size-3.5" />}
              {!hardwareLoaded ? m.power.detecting : laptop ? m.power.laptop : m.power.desktop}
            </div>
            <h2 className="mt-3 text-[26px] font-semibold tracking-[-0.03em] text-white">{m.power.maxTitle}</h2>
            <p className="mt-1 max-w-[640px] text-[13.5px] leading-relaxed text-[var(--text-2)]">
              {hardwareLoaded ? (laptop ? m.power.laptopHint : m.power.desktopHint) : m.power.maxDesc}
            </p>
          </div>
          <div className="flex flex-col items-end gap-2">
            <Button
              variant="primary"
              size="lg"
              icon={allMax ? <CircleCheck className="size-[18px]" /> : <Zap className="size-[18px]" />}
              loading={busy === "max"}
              disabled={allMax}
              onClick={() =>
                void run("max", async () => {
                  await ensureRestorePoint();
                  const reports = await api.tweaksOptimizeMany(maxIds);
                  const applied = reports.filter((r) => r.outcome === "applied").length;
                  const preview = reports.some((r) => r.outcome === "preview");
                  toast(preview ? "warn" : "success", preview ? m.dryRun.simulated : fmt(m.power.maxDone, { count: applied }));
                  await loadTweaks(SECTION_TWEAKS.power);
                  await refreshHistory();
                  loadSchemes();
                })
              }
            >
              {allMax ? m.common.optimized : m.power.maxRun}
            </Button>
            {maxStatus.length > 0 && (
              <span className="tabular text-[12px] text-[var(--text-3)]">
                {maxDone}/{maxStatus.length} {m.common.optimized.toLowerCase()}
              </span>
            )}
          </div>
        </div>
      </section>

      <BatteryCard />

      <div className="grid grid-cols-1 items-start gap-10">
        <Card id="pw-plans" className="">
          <CardHeader icon={<Power className="size-[18px]" />} tone="amber" title={m.power.plans} description={m.power.plansDesc} />
          <TweakList ids={["power.ultimate"]} />
          <div className="grid gap-2.5 px-5 pb-5">
            {schemes.map((s) => (
              <button
                key={s.guid}
                type="button"
                disabled={busy === "plan" || s.active}
                onClick={() =>
                  void run("plan", async () => {
                    const changed = await api.powerSetActive(s.guid);
                    if (!changed) toast("warn", m.dryRun.simulated);
                    loadSchemes();
                    await loadTweaks(SECTION_TWEAKS.power);
                    await refreshHistory();
                  })
                }
                className={cx(
                  "surface-2 lift flex items-center gap-3 rounded-[10px] px-4 py-3 text-left disabled:cursor-default",
                  s.active && "!border-emerald/35",
                )}
              >
                <span
                  className={cx(
                    "grid size-5 place-items-center rounded-full border-2",
                    s.active ? "border-emerald bg-emerald/20" : "border-white/20",
                  )}
                >
                  {s.active && <span className="size-2 rounded-full bg-emerald" />}
                </span>
                <span className={cx("flex-1 text-[13.5px] font-medium", s.active ? "text-white" : "text-slate-300")}>{s.name}</span>
              </button>
            ))}
          </div>
        </Card>

        <Card id="pw-settings" className="">
          <CardHeader
            icon={<SlidersHorizontal className="size-[18px]" />}
            tone="amber"
            title={m.power.settings}
            description={m.power.settingsDesc}
          />
          <TweakList ids={POWER_SETTINGS} />
        </Card>
      </div>
    </div>
  );
}
