// Tableau de bord (maquette premium) : accueil et score de santé, cartes Processeur / Mémoire / Stockage,
// graphique « Performances système » et carte « Dernière analyse ».
import { useId, useMemo, useRef, useState } from "react";
import {
  Activity,
  ArrowRight,
  CheckCircle2,
  Clock,
  Cpu,
  HardDrive,
  MemoryStick,
  MoreHorizontal,
  RefreshCw,
  Sparkles,
  TriangleAlert,
  Wand2,
} from "lucide-react";
import { Button, Card, cx } from "../../components/ui";
import { fmt, useI18n } from "../../i18n";
import { formatBytes, formatPercent } from "../../lib/format";
import { usePageActive } from "../../lib/pageActive";
import type { MetricsFrame } from "../../lib/types";
import { useApp, type SectionId } from "../../store/app";
import { ramPercent, useMetrics } from "../../store/metrics";
import { downsample, smooth } from "../../lib/series";
import { useHealth } from "./useHealth";

const EMPTY: number[] = [];

/** Courbe fine (une ou plusieurs séries 0–100), aire très discrète sous la ligne. */
function ThinChart({ series, height, grid = false }: { series: { values: number[]; color: string }[]; height: number; grid?: boolean }) {
  const id = useId();
  const width = 600;
  const paths = useMemo(
    () =>
      series.map(({ values }) => {
        if (values.length < 2) return null;
        const step = width / (values.length - 1);
        const y = (v: number) => height - 4 - (Math.max(0, Math.min(100, v)) / 100) * (height - 10);
        const p = values.map((v, i) => [i * step, y(v)] as const);
        let line = `M${p[0]![0].toFixed(1)},${p[0]![1].toFixed(1)}`;
        for (let i = 0; i < p.length - 1; i++) {
          const p0 = p[Math.max(0, i - 1)]!;
          const p1 = p[i]!;
          const p2 = p[i + 1]!;
          const p3 = p[Math.min(p.length - 1, i + 2)]!;
          const c1x = p1[0] + (p2[0] - p0[0]) / 6;
          const c1y = p1[1] + (p2[1] - p0[1]) / 6;
          const c2x = p2[0] - (p3[0] - p1[0]) / 6;
          const c2y = p2[1] - (p3[1] - p1[1]) / 6;
          line += ` C${c1x.toFixed(1)},${c1y.toFixed(1)} ${c2x.toFixed(1)},${c2y.toFixed(1)} ${p2[0].toFixed(1)},${p2[1].toFixed(1)}`;
        }
        return { line, area: `${line} L${width},${height} L0,${height} Z` };
      }),
    [series, height],
  );
  return (
    <svg viewBox={`0 0 ${width} ${height}`} preserveAspectRatio="none" className="block w-full" style={{ height }} aria-hidden>
      <defs>
        {series.map((s, i) => (
          <linearGradient key={i} id={`${id}${i}`} x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor={s.color} stopOpacity="0.16" />
            <stop offset="100%" stopColor={s.color} stopOpacity="0" />
          </linearGradient>
        ))}
      </defs>
      {grid &&
        [0.25, 0.5, 0.75].map((g) => (
          <line
            key={g}
            x1="0"
            x2={width}
            y1={height * g}
            y2={height * g}
            stroke="var(--grid-line)"
            strokeWidth="1"
            vectorEffect="non-scaling-stroke"
          />
        ))}
      {paths.map((p, i) =>
        p ? (
          <g key={i}>
            <path d={p.area} fill={`url(#${id}${i})`} />
            <path
              d={p.line}
              fill="none"
              stroke={series[i]?.color}
              strokeWidth="1.6"
              strokeLinejoin="round"
              vectorEffect="non-scaling-stroke"
            />
          </g>
        ) : null,
      )}
    </svg>
  );
}

/** Fige les données quand la page est masquée (aucun rendu inutile en arrière-plan). */
function useLiveMetrics() {
  const active = usePageActive();
  const frozen = useRef<{ frame: MetricsFrame | null; cpu: number[]; ram: number[] }>({ frame: null, cpu: EMPTY, ram: EMPTY });
  const frame = useMetrics((s) => (active ? s.frame : frozen.current.frame));
  const cpu = useMetrics((s) => (active ? s.cpu : frozen.current.cpu));
  const ram = useMetrics((s) => (active ? s.ram : frozen.current.ram));
  if (active) frozen.current = { frame, cpu, ram };
  return { frame, cpu, ram };
}

/** Accueil : salutation, état du PC, action principale et anneau du score de santé. */
export function Hero({
  busy,
  fixCount,
  findingCount,
  onFix,
  analyzing,
  onAnalyze,
}: {
  busy: boolean;
  fixCount: number;
  findingCount: number;
  onFix: () => void;
  analyzing: boolean;
  onAnalyze: () => void;
}) {
  const { m } = useI18n();
  const health = useHealth();
  // Tant que les données ne sont pas chargées, aucun score provisoire n’est affiché.
  const ready = useApp((s) => s.systemInfo !== null && Object.keys(s.tweaks).length > 0);
  const level = health >= 85 ? "good" : health >= 50 ? "fair" : "poor";
  const title = level === "good" ? m.overview.heroGood : level === "fair" ? m.overview.heroFair : m.overview.heroPoor;
  const badge = level === "good" ? m.overview.badgeGood : level === "fair" ? m.overview.badgeFair : m.overview.badgePoor;
  const greet = new Date().getHours() < 18 ? m.overview.greetMorning : m.overview.greetEvening;
  const size = 220;
  const stroke = 7;
  const r = (size - stroke) / 2 - 4;
  const c = 2 * Math.PI * r;
  const id = useId();
  return (
    <section id="ov-health" className="anchor-card grid grid-cols-1 items-center gap-10 md:grid-cols-[minmax(0,1fr)_auto]">
      <div className="min-w-0">
        <div className="text-[19px] text-zinc-400">{greet}</div>
        <h1 className="mt-1 text-[36px] leading-[1.12] font-medium tracking-[-0.02em] text-zinc-50 xl:text-[44px]">
          {ready ? title : m.scan.analyzing}
        </h1>
        <p className={cx("mt-3 max-w-[620px] text-[15px] text-zinc-400 transition-opacity duration-300", !ready && "opacity-0")}>
          {findingCount > 0
            ? fmt(m.overview.heroSubFindings, { count: findingCount })
            : level === "good"
              ? m.overview.heroSubGood
              : m.overview.heroSubLimited}
        </p>
        <div className="mt-8">
          {fixCount > 0 ? (
            <Button
              variant="primary"
              size="lg"
              loading={busy}
              onClick={onFix}
              icon={<Wand2 className="size-[18px] text-[var(--accent-soft)]" strokeWidth={1.7} />}
              title={m.scan.safetyNote}
              className="rounded-full pr-5"
            >
              {busy ? m.scan.fixing : m.overview.optimizeAll}
              {!busy && <ArrowRight className="size-4 text-zinc-400" aria-hidden />}
            </Button>
          ) : (
            <Button
              variant="primary"
              size="lg"
              loading={analyzing}
              onClick={onAnalyze}
              icon={<RefreshCw className="size-4 text-[var(--accent-soft)]" strokeWidth={1.7} />}
              className="rounded-full pr-5"
            >
              {m.scan.analyze}
              <ArrowRight className="size-4 text-zinc-400" aria-hidden />
            </Button>
          )}
        </div>
      </div>
      <div className="flex flex-col items-center gap-4 justify-self-center">
        <div
          className="relative grid place-items-center"
          style={{ width: size, height: size }}
          role="img"
          aria-label={`${m.overview.health} : ${health}`}
        >
          <svg width={size} height={size} className="-rotate-90">
            <defs>
              <linearGradient id={`${id}g`} x1="0" y1="1" x2="1" y2="0">
                <stop offset="0%" stopColor="#2a8f8a" />
                <stop offset="100%" stopColor="#46cf92" />
              </linearGradient>
            </defs>
            <circle cx={size / 2} cy={size / 2} r={r} fill="none" stroke="var(--ring-track)" strokeWidth={stroke} />
            <circle
              cx={size / 2}
              cy={size / 2}
              r={r}
              fill="none"
              stroke={level === "poor" ? "#d9822b" : `url(#${id}g)`}
              strokeWidth={stroke}
              strokeLinecap="round"
              strokeDasharray={c}
              strokeDashoffset={ready ? c - (Math.max(0, Math.min(100, health)) / 100) * c : c}
              style={{ transition: "stroke-dashoffset 900ms cubic-bezier(0.2,0,0,1)" }}
            />
          </svg>
          <div className="absolute inset-0 grid place-items-center text-center">
            <div>
              <div className="tabular text-[62px] leading-none font-medium tracking-[-0.04em] text-zinc-50">{ready ? health : "—"}</div>
              <div className="mt-2 text-[13px] text-zinc-400">{m.overview.health}</div>
            </div>
          </div>
        </div>
        <span
          className={cx(
            "inline-flex items-center gap-2 rounded-full border px-3.5 py-1.5 text-[13px] transition-opacity duration-300",
            !ready && "opacity-0",
            level === "good"
              ? "border-[var(--good)]/25 bg-[var(--good)]/10 text-[var(--good)]"
              : "border-amber-400/25 bg-amber-400/10 text-amber-300",
          )}
        >
          {level === "good" ? <CheckCircle2 className="size-4" aria-hidden /> : <TriangleAlert className="size-4" aria-hidden />}
          {badge}
        </span>
      </div>
    </section>
  );
}

/** Carte de mesure harmonisée : icône, titre, menu, valeur, graphique fin, détail secondaire. */
function StatCard({
  icon,
  title,
  value,
  footer,
  children,
  target,
}: {
  icon: React.ReactNode;
  title: string;
  value: string;
  footer: string;
  children: React.ReactNode;
  target: SectionId;
}) {
  const { m } = useI18n();
  const goTo = useApp((s) => s.goTo);
  return (
    <Card hover className="flex flex-col p-6">
      <div className="flex items-center gap-3.5">
        <span className="text-[var(--steel)]">{icon}</span>
        <span className="flex-1 truncate text-[15px] text-zinc-100">{title}</span>
        <button
          type="button"
          onClick={() => goTo(target)}
          title={m.overview.openSection}
          aria-label={m.overview.openSection}
          className="grid size-8 place-items-center rounded-[9px] text-zinc-500 transition-colors hover:bg-white/[0.05] hover:text-zinc-200"
        >
          <MoreHorizontal className="size-[18px]" aria-hidden />
        </button>
      </div>
      <div className="tabular mt-3 text-right text-[34px] leading-none font-medium tracking-[-0.02em] text-zinc-50">{value}</div>
      <div className="mt-4 flex-1">{children}</div>
      <div className="mt-3 truncate text-[12.5px] text-zinc-500">{footer}</div>
    </Card>
  );
}

/** Rangée Processeur / Mémoire vive / Stockage. */
export function StatRow() {
  const { m, lang } = useI18n();
  const { frame, cpu, ram } = useLiveMetrics();
  const info = useApp((s) => s.systemInfo);
  const disk = info?.disks.find((d) => d.mount.toUpperCase().startsWith("C")) ?? info?.disks[0];
  const usedPct = disk ? ((disk.total - disk.available) / Math.max(1, disk.total)) * 100 : 0;
  const ramPct = ramPercent(frame);
  return (
    <div id="ov-live" className="anchor-card grid grid-cols-1 gap-5 md:grid-cols-2 xl:grid-cols-3">
      <StatCard
        icon={<Cpu className="size-[22px]" strokeWidth={1.5} />}
        title={m.overview.cpu}
        value={frame ? formatPercent(frame.cpu, lang) : "—"}
        footer={info ? `${info.threads} threads · ${info.cpu}` : ""}
        target="performance"
      >
        <ThinChart series={[{ values: smooth(cpu), color: "#8db4f5" }]} height={56} />
      </StatCard>
      <StatCard
        icon={<MemoryStick className="size-[22px]" strokeWidth={1.5} />}
        title={m.overview.ram}
        value={ramPct === null ? "—" : formatPercent(ramPct, lang)}
        footer={frame ? `${formatBytes(frame.ramUsed, lang, 1)} / ${formatBytes(frame.ramTotal, lang, 0)}` : ""}
        target="performance"
      >
        <ThinChart series={[{ values: smooth(ram), color: "#8db4f5" }]} height={56} />
      </StatCard>
      <StatCard
        icon={<HardDrive className="size-[22px]" strokeWidth={1.5} />}
        title={`${m.overview.storage} ${disk?.mount.replace(/\\$/, "") ?? ""}`}
        value={disk ? formatBytes(disk.available, lang, 1) : "—"}
        footer={disk ? fmt(m.overview.freeOf, { total: formatBytes(disk.total, lang, 1) }) : ""}
        target="storage"
      >
        <div className="flex h-[56px] items-center">
          <div className="h-1.5 w-full overflow-hidden rounded-full bg-white/[0.06]">
            <div
              className="meter-fill h-full rounded-full"
              style={{
                transform: `scaleX(${usedPct / 100})`,
                background: usedPct > 90 ? "#d9822b" : "linear-gradient(90deg, #5d7fae, #9db7dc)",
              }}
            />
          </div>
        </div>
      </StatCard>
    </div>
  );
}

// Un point toutes les 10 secondes (moyenne) : 10 min = 60 points, 30 min = 180, 1 h = 360.
const RANGES = [
  { key: "10m", label: "10 min", points: 60 },
  { key: "30m", label: "30 min", points: 180 },
  { key: "1h", label: "1 h", points: 360 },
] as const;

/** Graphique « Performances système » : processeur et mémoire sur 10 min, 30 min ou 1 h. */
export function PerfChart() {
  const { m } = useI18n();
  const active = usePageActive();
  const [range, setRange] = useState<(typeof RANGES)[number]["key"]>("10m");
  const points = RANGES.find((r) => r.key === range)?.points ?? 60;
  const frozen = useRef<{ cpu: number[]; ram: number[] }>({ cpu: EMPTY, ram: EMPTY });
  const longCpu = useMetrics((s) => (active ? s.longCpu : frozen.current.cpu));
  const longRam = useMetrics((s) => (active ? s.longRam : frozen.current.ram));
  if (active) frozen.current = { cpu: longCpu, ram: longRam };
  const cpu = useMemo(() => smooth(downsample(longCpu.slice(-points), 240)), [longCpu, points]);
  const ram = useMemo(() => smooth(downsample(longRam.slice(-points), 240)), [longRam, points]);
  return (
    <Card className="flex flex-col p-6">
      <div className="flex flex-wrap items-center gap-x-4 gap-y-3">
        <Activity className="size-[20px] text-[var(--steel)]" strokeWidth={1.5} aria-hidden />
        <span className="flex-1 text-[15px] text-zinc-100">{m.overview.perfTitle}</span>
        <div className="flex items-center gap-4 text-[12px] text-zinc-500">
          <span className="flex items-center gap-1.5">
            <span className="size-2 rounded-full bg-[#8db4f5]" />
            {m.overview.cpu}
          </span>
          <span className="flex items-center gap-1.5">
            <span className="size-2 rounded-full bg-[#3fbf8f]" />
            {m.overview.ram}
          </span>
        </div>
        <div className="flex gap-1 rounded-[11px] border border-white/[0.05] bg-black/20 p-1" role="tablist">
          {RANGES.map((r) => (
            <button
              key={r.key}
              type="button"
              role="tab"
              aria-selected={range === r.key}
              onClick={() => setRange(r.key)}
              className={cx(
                "h-7 rounded-[8px] px-3 text-[12px] transition-colors",
                range === r.key ? "bg-white/[0.08] text-zinc-100" : "text-zinc-500 hover:text-zinc-200",
              )}
            >
              {r.label}
            </button>
          ))}
        </div>
      </div>
      <div className="mt-5 flex-1">
        <ThinChart
          grid
          height={150}
          series={[
            { values: cpu, color: "#8db4f5" },
            { values: ram, color: "#3fbf8f" },
          ]}
        />
      </div>
    </Card>
  );
}

/** Carte « Dernière analyse » : moment de l'analyse, résultat et relance. */
export function LastScanCard({ findings, analyzing, onAnalyze }: { findings: number; analyzing: boolean; onAnalyze: () => void }) {
  const { m, lang } = useI18n();
  const last = useApp((s) => s.lastAnalysis);
  const when = last
    ? new Date(last).toDateString() === new Date().toDateString()
      ? `${m.overview.today} · ${new Date(last).toLocaleTimeString(lang, { hour: "2-digit", minute: "2-digit" })}`
      : new Date(last).toLocaleString(lang, { dateStyle: "medium", timeStyle: "short" })
    : m.scan.analyzing;
  return (
    <Card className="flex flex-col p-6">
      <div className="flex items-start gap-3.5">
        <Clock className="mt-0.5 size-[20px] text-[var(--steel)]" strokeWidth={1.5} aria-hidden />
        <div className="min-w-0">
          <div className="text-[15px] text-zinc-100">{m.overview.lastScan}</div>
          <div className="mt-0.5 text-[13px] text-zinc-500">{when}</div>
        </div>
      </div>
      <div className="mt-6 flex flex-1 items-start gap-3 text-[14px]">
        {findings === 0 ? (
          <>
            <CheckCircle2 className="mt-0.5 size-[18px] shrink-0 text-[var(--good)]" aria-hidden />
            <span className="text-zinc-200">{m.overview.noIssue}</span>
          </>
        ) : (
          <>
            <Sparkles className="mt-0.5 size-[18px] shrink-0 text-[var(--accent-soft)]" aria-hidden />
            <span className="text-zinc-200">{fmt(m.overview.issues, { count: findings })}</span>
          </>
        )}
      </div>
      <Button className="mt-6 w-full" loading={analyzing} icon={<RefreshCw className="size-4" />} onClick={onAnalyze}>
        {m.scan.analyze}
      </Button>
    </Card>
  );
}
