// Section « Vue d’ensemble » (maquette premium) : accueil, mesures en direct, analyse, suggestions,
// avant/après, protection et profils.
import { useCallback, useEffect, useRef, useState } from "react";
import {
  ArrowRight,
  Briefcase,
  BriefcaseBusiness,
  Crosshair,
  Moon,
  Palette,
  Radio,
  CircleCheck,
  Download,
  Eye,
  Gamepad2,
  Laptop,
  MinusCircle,
  RefreshCw,
  ShieldCheck,
  ShieldOff,
  Sparkles,
  Sun,
  Upload,
  XCircle,
} from "lucide-react";
import { Modal } from "../../components/Overlays";
import { Button, Card, CardHeader, cx, IconBadge, type Tone } from "../../components/ui";
import { fmt, useI18n } from "../../i18n";
import { api, errorMessage } from "../../lib/ipc";
import { formatDate } from "../../lib/format";
import type { GuidedProfile, TweakReport, UserLevel } from "../../lib/types";
import { atLeast } from "../../lib/level";
import { ensureRestorePoint } from "../../lib/safety";
import { useApp } from "../../store/app";
import { Hero, LastScanCard, PerfChart, StatRow } from "./Dashboard";
import { type Finding, useFindings, useFixAll } from "./findings";
import { MeasureCard } from "./MeasureCard";

function countChanged(reports: TweakReport[]): number {
  return reports.filter((r) => r.outcome === "applied").length;
}

const PROFILE_GROUPS = ["daily", "games", "work", "pro"] as const;

interface ProfileDef {
  id: GuidedProfile;
  group: (typeof PROFILE_GROUPS)[number];
  min: UserLevel;
  icon: typeof Sun;
  title: string;
  audience: string;
  desc: string;
  tone: Tone;
}

export function OverviewPage() {
  const loadTweaks = useApp((s) => s.loadTweaks);
  const loadRestore = useApp((s) => s.loadRestore);
  const loadRules = useApp((s) => s.loadRules);
  const runScan = useApp((s) => s.runScan);
  const [startupEnabled, setStartupEnabled] = useState<number | null>(null);
  const [analyzing, setAnalyzing] = useState(false);
  const findings = useFindings(startupEnabled);
  const { fixAll, busy } = useFixAll(findings);

  const loadStartup = useCallback(
    () =>
      api
        .startupList()
        .then((l) => setStartupEnabled(l.filter((e) => e.enabled).length))
        .catch(() => setStartupEnabled(null)),
    [],
  );

  useEffect(() => {
    void loadRules();
    void loadStartup();
  }, [loadRules, loadStartup]);

  const analyze = useCallback(async () => {
    setAnalyzing(true);
    try {
      await Promise.all([runScan(true), loadTweaks(), loadRestore(true), loadStartup()]);
      useApp.getState().markAnalyzed();
    } finally {
      setAnalyzing(false);
    }
  }, [runScan, loadTweaks, loadRestore, loadStartup]);

  const fixEverything = useCallback(async () => {
    await fixAll();
    await analyze();
  }, [fixAll, analyze]);

  // La palette de commandes (Ctrl+K) déclenche la même action.
  useEffect(() => {
    const onExpress = () => void fixEverything();
    window.addEventListener("aether:express", onExpress);
    return () => window.removeEventListener("aether:express", onExpress);
  }, [fixEverything]);

  const autoCount = findings.filter((f) => f.auto).length;

  return (
    <div className="space-y-10">
      <Hero
        busy={busy}
        fixCount={autoCount}
        findingCount={findings.length}
        onFix={() => void fixEverything()}
        analyzing={analyzing}
        onAnalyze={() => void analyze()}
      />
      <StatRow />
      <div className="grid grid-cols-1 gap-5 xl:grid-cols-[minmax(0,2fr)_minmax(0,1fr)]">
        <PerfChart />
        <LastScanCard findings={findings.length} analyzing={analyzing} onAnalyze={() => void analyze()} />
      </div>
      {findings.length > 0 && (
        <FindingsCard
          findings={findings}
          analyzing={analyzing}
          fixing={busy}
          onAnalyze={() => void analyze()}
          onFixed={() => void analyze()}
        />
      )}
      <MeasureCard />
      <ProtectionCard />
      <ProfilesCard />
    </div>
  );
}

function FindingsCard({
  findings,
  analyzing,
  fixing,
  onAnalyze,
  onFixed,
}: {
  findings: Finding[];
  analyzing: boolean;
  fixing: boolean;
  onAnalyze: () => void;
  onFixed: () => void;
}) {
  const { m } = useI18n();
  const toast = useApp((s) => s.toast);
  const dryRun = useApp((s) => s.dryRun);
  const [busyId, setBusyId] = useState<string | null>(null);
  const fixOne = async (f: Finding) => {
    setBusyId(f.id);
    try {
      await f.fix();
      if (f.auto) toast(dryRun ? "warn" : "success", dryRun ? m.dryRun.simulated : m.outcome.applied);
      onFixed();
    } catch (e) {
      toast("error", errorMessage(e));
    } finally {
      setBusyId(null);
    }
  };
  return (
    <Card id="ov-express" className="">
      <CardHeader
        icon={<Sparkles className="size-[18px]" />}
        tone="sky"
        title={m.scan.title}
        description={m.scan.subtitle}
        actions={
          <Button
            size="sm"
            icon={<RefreshCw className={cx("size-3.5", analyzing && "animate-spin")} />}
            disabled={analyzing || fixing}
            onClick={onAnalyze}
          >
            {analyzing ? m.scan.analyzing : m.scan.analyze}
          </Button>
        }
      />
      {findings.length > 0 && (
        <ul className="stagger space-y-2 px-5 pb-5">
          {findings.map((f) => (
            <li key={f.id} title={f.desc} className="surface-2 flex min-w-0 items-center gap-4 rounded-[10px] px-4 py-3 animate-fade-in">
              <IconBadge tone={f.tone}>{f.icon}</IconBadge>
              <div className="min-w-0 flex-1 truncate text-[14px] font-medium text-white">{f.title}</div>
              <Button
                size="sm"
                variant={f.auto ? "indigo" : "secondary"}
                loading={busyId === f.id}
                disabled={fixing || (busyId !== null && busyId !== f.id)}
                onClick={() => void fixOne(f)}
                icon={f.auto ? undefined : <ArrowRight className="size-3.5" />}
              >
                {f.fixLabel}
              </Button>
            </li>
          ))}
        </ul>
      )}
    </Card>
  );
}

function ProtectionCard() {
  const { m, lang } = useI18n();
  const restore = useApp((s) => s.restore);
  const setRestore = useApp((s) => s.setRestore);
  const toast = useApp((s) => s.toast);
  const [busy, setBusy] = useState(false);
  const act = async (fn: () => Promise<void>) => {
    setBusy(true);
    try {
      await fn();
    } catch (e) {
      toast("error", errorMessage(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Card id="ov-protection" className="col-span-12 flex flex-col lg:col-span-5">
      <CardHeader
        icon={restore?.enabled ? <ShieldCheck className="size-[18px]" /> : <ShieldOff className="size-[18px]" />}
        tone={restore?.enabled ? "emerald" : "amber"}
        title={m.overview.protection}
        description={
          restore
            ? restore.blockedByPolicy
              ? m.overview.protectionPolicy
              : fmt(restore.enabled ? m.overview.protectionOn : m.overview.protectionOff, { drive: restore.drive })
            : m.common.loading
        }
      />
      <div className="mt-auto space-y-3 px-5 pb-5">
        <div className="well rounded-[10px] px-4 py-3 text-[12.5px]">
          <div className="text-[11px] font-semibold uppercase tracking-[0.1em] text-[var(--text-3)]">{m.overview.lastPoint}</div>
          <div className="mt-1 text-slate-200">
            {restore?.lastPoint ? `${restore.lastPoint.description} · ${formatDate(restore.lastPoint.created, lang)}` : m.overview.noPoint}
          </div>
        </div>
        {restore && !restore.enabled && !restore.blockedByPolicy ? (
          <Button
            variant="primary"
            className="w-full"
            loading={busy}
            onClick={() => void act(async () => setRestore(await api.restoreEnable()))}
          >
            {m.overview.enableProtection}
          </Button>
        ) : (
          <Button
            className="w-full"
            loading={busy}
            disabled={!restore?.enabled}
            icon={<ShieldCheck className="size-4" />}
            onClick={() =>
              void act(async () => {
                setRestore(await api.restoreCreate("Aether"));
                toast("success", m.onboarding.restoreCreated);
              })
            }
          >
            {m.overview.createPoint}
          </Button>
        )}
      </div>
    </Card>
  );
}

function ProfilesCard() {
  const { m } = useI18n();
  const profile = useApp((s) => s.profile);
  const setProfile = useApp((s) => s.setProfile);
  const loadTweaks = useApp((s) => s.loadTweaks);
  const toast = useApp((s) => s.toast);
  const level = useApp((s) => s.level);
  const tweaks = useApp((s) => s.tweaks);
  const info = useApp((s) => s.systemInfo);
  const hardwareLoaded = useApp((s) => s.hardwareLoaded);
  const [recommended, setRecommended] = useState<string[]>([]);
  const [report, setReport] = useState<TweakReport[] | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const fileRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    void api
      .profileTweaks(profile)
      .then(setRecommended)
      .catch(() => setRecommended([]));
  }, [profile]);

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

  // Profils rangés par usage ; seuls ceux adaptés au niveau de l'utilisateur sont proposés.
  const allProfiles: ProfileDef[] = [
    {
      id: "daily",
      group: "daily",
      min: "beginner",
      icon: Sun,
      title: m.overview.profileDaily,
      audience: m.overview.profileDailyFor,
      desc: m.overview.profileDailyDesc,
      tone: "emerald",
    },
    {
      id: "laptop",
      group: "daily",
      min: "beginner",
      icon: Laptop,
      title: m.overview.profileLaptop,
      audience: m.overview.profileLaptopFor,
      desc: m.overview.profileLaptopDesc,
      tone: "sky",
    },
    {
      id: "silent",
      group: "daily",
      min: "beginner",
      icon: Moon,
      title: m.profiles.silent,
      audience: m.profiles.silentFor,
      desc: m.profiles.silentDesc,
      tone: "slate",
    },
    {
      id: "gamer",
      group: "games",
      min: "gamer",
      icon: Gamepad2,
      title: m.overview.profileGamer,
      audience: m.overview.profileGamerFor,
      desc: m.overview.profileGamerDesc,
      tone: "violet",
    },
    {
      id: "streaming",
      group: "games",
      min: "gamer",
      icon: Radio,
      title: m.profiles.streaming,
      audience: m.profiles.streamingFor,
      desc: m.profiles.streamingDesc,
      tone: "violet",
    },
    {
      id: "competitive",
      group: "games",
      min: "expert",
      icon: Crosshair,
      title: m.profiles.competitive,
      audience: m.profiles.competitiveFor,
      desc: m.profiles.competitiveDesc,
      tone: "rose",
    },
    {
      id: "office",
      group: "work",
      min: "beginner",
      icon: Briefcase,
      title: m.profiles.office,
      audience: m.profiles.officeFor,
      desc: m.profiles.officeDesc,
      tone: "sky",
    },
    {
      id: "creator",
      group: "work",
      min: "gamer",
      icon: Palette,
      title: m.profiles.creator,
      audience: m.profiles.creatorFor,
      desc: m.profiles.creatorDesc,
      tone: "indigo",
    },
    {
      id: "client",
      group: "pro",
      min: "beginner",
      icon: BriefcaseBusiness,
      title: m.overview.profileClient,
      audience: m.overview.profileClientFor,
      desc: m.overview.profileClientDesc,
      tone: "amber",
    },
  ];
  const profiles = allProfiles.filter((p) => atLeast(level, p.min));
  const groups = PROFILE_GROUPS.map((g) => ({ g, items: profiles.filter((p) => p.group === g) })).filter((x) => x.items.length > 0);
  // Profil conseillé : « Portable » si une batterie est détectée, sinon « Essentiel ».
  const suggested: GuidedProfile | null = hardwareLoaded ? (info?.battery ? "laptop" : "daily") : null;
  const titleOf = (id: string) =>
    id === "clean.safe" ? m.overview.profileClientClean : ((m.tweaks as Record<string, { title: string } | undefined>)[id]?.title ?? id);
  const allChanges = recommended.map((id) => ({
    id,
    title: titleOf(id),
    applied: Boolean(tweaks[id]?.probe.applied),
    available: tweaks[id]?.probe.available ?? true,
    reason: tweaks[id]?.probe.current ?? "",
  }));
  // Les réglages absents de ce PC sont listés à part et ne comptent pas dans la progression.
  const changes = allChanges.filter((c) => c.available);
  const missing = allChanges.filter((c) => !c.available);

  return (
    <Card id="ov-profiles" className="">
      <CardHeader
        icon={<Sparkles className="size-[18px]" />}
        tone="violet"
        title={m.overview.profiles}
        description={m.overview.profilesDesc}
        actions={
          <>
            <input
              ref={fileRef}
              type="file"
              accept=".json,application/json"
              className="hidden"
              onChange={(e) => {
                const file = e.target.files?.[0];
                e.target.value = "";
                if (!file) return;
                void run("import", async () => {
                  const reports = await api.profileImport(await file.text());
                  toast("success", fmt(m.toast.imported, { count: countChanged(reports) }));
                  await loadTweaks();
                });
              }}
            />
            <Button size="sm" icon={<Upload className="size-3.5" />} loading={busy === "import"} onClick={() => fileRef.current?.click()}>
              {m.overview.importProfile}
            </Button>
            <Button
              size="sm"
              icon={<Download className="size-3.5" />}
              loading={busy === "export"}
              onClick={() =>
                void run("export", async () => {
                  toast("success", fmt(m.toast.exported, { path: await api.profileExport() }));
                })
              }
            >
              {m.overview.exportProfile}
            </Button>
          </>
        }
      />
      <div className="space-y-5 px-5 pb-4" role="radiogroup" aria-label={m.overview.profiles}>
        {groups.map(({ g, items }) => (
          <div key={g}>
            <div className="mb-2 text-[12px] font-medium tracking-wide text-[var(--text-3)] uppercase">{m.profiles.groups[g]}</div>
            <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 xl:grid-cols-3">
              {items.map(({ id, icon: Icon, title, tone, audience, desc: hint }) => {
                const active = id === profile;
                return (
                  <button
                    key={id}
                    type="button"
                    role="radio"
                    onClick={() => setProfile(id)}
                    aria-checked={active}
                    title={hint}
                    className={cx(
                      "surface-2 relative rounded-[10px] p-5 text-left transition-colors duration-200",
                      active ? "!border-[var(--accent)] !bg-[#141822]" : "hover:!border-white/15",
                    )}
                  >
                    <div className="flex items-start justify-between gap-2">
                      <IconBadge tone={tone} size="lg">
                        <Icon className="size-5" aria-hidden />
                      </IconBadge>
                      <span
                        className={cx(
                          "grid size-5 place-items-center rounded-full border transition-colors",
                          active ? "border-[var(--accent)] bg-[var(--accent)]" : "border-white/20",
                        )}
                        aria-hidden
                      >
                        {active && <span className="size-2 rounded-full bg-white" />}
                      </span>
                    </div>
                    <div className="mt-4 flex flex-wrap items-center gap-2">
                      <span className="text-[16px] font-semibold tracking-[-0.01em] text-white">{title}</span>
                      {suggested === id && (
                        <span className="rounded-full bg-emerald/15 px-2 py-0.5 text-[10.5px] font-semibold text-emerald-300">
                          {m.overview.recommendedBadge}
                        </span>
                      )}
                    </div>
                    <div className="mt-0.5 text-[12px] font-medium text-[var(--text-3)]">{audience}</div>
                  </button>
                );
              })}
            </div>
          </div>
        ))}
      </div>
      <div className="mx-5 mb-5 rounded-[10px] border border-white/[0.06] bg-[#0c0d10] p-5">
        <div className="flex items-center justify-between gap-4">
          <div>
            <div className="text-[13.5px] font-semibold text-white">{m.overview.whatChanges}</div>
            <div className="tabular mt-0.5 text-[12px] text-[var(--text-3)]">
              {fmt(m.overview.activeCount, { done: changes.filter((c) => c.applied).length, total: changes.length })}
            </div>
          </div>
          <div className="h-1.5 w-40 overflow-hidden rounded-full bg-[var(--track)]">
            <div
              className="meter-fill h-full rounded-full bg-[var(--accent)]"
              style={{ transform: `scaleX(${changes.length ? changes.filter((c) => c.applied).length / changes.length : 0})` }}
            />
          </div>
        </div>
        <ul className="mt-4 grid grid-cols-1 gap-x-6 gap-y-2 sm:grid-cols-2">
          {changes.slice(0, 8).map((c) => (
            <li key={c.id} className="flex items-center gap-2.5 text-[12.5px]">
              {c.applied ? (
                <CircleCheck className="size-4 shrink-0 text-emerald" aria-hidden />
              ) : (
                <span className="size-4 shrink-0 rounded-full border border-white/20" aria-hidden />
              )}
              <span className={cx("truncate", c.applied ? "text-[var(--text-3)]" : "text-zinc-200")}>{c.title}</span>
            </li>
          ))}
        </ul>
        {changes.length > 8 && (
          <div className="mt-2 text-[12px] text-[var(--text-3)]">{fmt(m.overview.moreCount, { count: changes.length - 8 })}</div>
        )}
        {missing.length > 0 && (
          <div className="mt-4 border-t border-white/[0.05] pt-3">
            <div className="text-[11px] font-semibold uppercase tracking-[0.1em] text-[var(--text-3)]">{m.profileReport.unavailable}</div>
            <ul className="mt-2 flex flex-wrap gap-1.5">
              {missing.map((c) => (
                <li
                  key={c.id}
                  title={c.reason}
                  className="rounded-full border border-white/[0.06] px-2.5 py-0.5 text-[11.5px] text-[var(--text-3)]"
                >
                  {c.title}
                </li>
              ))}
            </ul>
          </div>
        )}
      </div>
      <div className="flex items-center justify-between border-t border-white/[0.05] px-6 py-4">
        <span />
        <Button
          variant="primary"
          size="sm"
          icon={<ArrowRight className="size-3.5" />}
          loading={busy === "profile"}
          onClick={() =>
            void run("profile", async () => {
              await ensureRestorePoint();
              const reports = await api.profileApply(profile);
              setReport(reports);
              await loadTweaks();
              // Le profil client nettoie aussi les fichiers temporaires : tailles mises à jour.
              if (profile === "client") void useApp.getState().runScan(true);
            })
          }
        >
          {m.overview.profileApply}
        </Button>
      </div>
      <ProfileReportModal reports={report} titleOf={titleOf} onClose={() => setReport(null)} />
    </Card>
  );
}

/** Bilan détaillé d'un profil : chaque réglage avec son résultat et, le cas échéant, la raison. */
function ProfileReportModal({
  reports,
  titleOf,
  onClose,
}: {
  reports: TweakReport[] | null;
  titleOf: (id: string) => string;
  onClose: () => void;
}) {
  const { m } = useI18n();
  if (!reports) return null;
  const count = (o: TweakReport["outcome"]) => reports.filter((r) => r.outcome === o).length;
  const failed = reports.filter((r) => r.error).length;
  const status = (r: TweakReport): { label: string; icon: React.ReactNode; tone: string } => {
    if (r.error) return { label: m.profileReport.failed, icon: <XCircle className="size-4" />, tone: "text-rose-300" };
    switch (r.outcome) {
      case "applied":
        return { label: m.profileReport.applied, icon: <CircleCheck className="size-4" />, tone: "text-emerald-300" };
      case "alreadyDone":
        return { label: m.profileReport.already, icon: <CircleCheck className="size-4" />, tone: "text-zinc-400" };
      case "preview":
        return { label: m.profileReport.preview, icon: <Eye className="size-4" />, tone: "text-cyan" };
      default:
        return { label: m.profileReport.unavailable, icon: <MinusCircle className="size-4" />, tone: "text-zinc-500" };
    }
  };
  return (
    <Modal
      open
      title={m.profileReport.title}
      onClose={onClose}
      footer={
        <Button variant="primary" onClick={onClose}>
          {m.common.close}
        </Button>
      }
    >
      <p className="text-[12.5px] text-[var(--text-2)]">
        {fmt(m.profileReport.summary, {
          applied: count("applied") + count("preview"),
          already: count("alreadyDone"),
          unavailable: count("unavailable"),
          failed,
        })}
      </p>
      <ul className="scroll-area mt-3 max-h-[340px] space-y-1.5 pr-1">
        {reports.map((r) => {
          const s = status(r);
          return (
            <li key={r.id} className="rounded-[8px] border border-white/[0.06] bg-white/[0.02] px-3 py-2">
              <div className="flex items-center gap-2.5">
                <span className={s.tone}>{s.icon}</span>
                <span className="min-w-0 flex-1 truncate text-[13px] text-zinc-100">{titleOf(r.id)}</span>
                <span className={cx("shrink-0 text-[11.5px]", s.tone)}>{s.label}</span>
              </div>
              {(r.error ?? r.detail) && <div className="mt-1 pl-[26px] text-[11.5px] text-[var(--text-3)]">{r.error ?? r.detail}</div>}
            </li>
          );
        })}
      </ul>
    </Modal>
  );
}
