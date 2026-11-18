// Section « Jeux » : bibliothèque des jeux installés et lancement avec Boost.
import { useCallback, useEffect, useMemo, useState } from "react";
import {
  CircleCheck,
  CircleDashed,
  CircleX,
  Cpu,
  Gamepad2,
  Gauge,
  Globe,
  LoaderCircle,
  type LucideIcon,
  MemoryStick,
  MinusCircle,
  Play,
  Radar,
  RefreshCw,
  Rocket,
  Search,
  SlidersHorizontal,
  Square,
  Zap,
} from "lucide-react";
import { Button, Card, CardHeader, cx, Empty, IconBadge, PageHeader, Skeleton, Toggle } from "../../components/ui";
import { Modal } from "../../components/Overlays";
import { fmt, useI18n } from "../../i18n";
import { api, errorMessage } from "../../lib/ipc";
import type { BoostEvent, BoostOptions, BoostStep, Game } from "../../lib/types";
import { useApp } from "../../store/app";

const DEFAULT_OPTIONS: BoostOptions = { purgeRam: true, flushDns: true, performancePlan: true, highPriority: true, noThrottling: true };
const STEPS: BoostStep[] = ["ram", "dns", "plan", "launch", "detect", "priority", "session", "restore"];
const SOURCE_LABEL: Record<Game["source"], string> = { steam: "Steam", epic: "Epic Games", gog: "GOG" };

function loadOptions(): BoostOptions {
  try {
    const raw = window.localStorage.getItem("aether.boost");
    if (!raw) return DEFAULT_OPTIONS;
    const parsed: unknown = JSON.parse(raw);
    if (typeof parsed !== "object" || parsed === null) return DEFAULT_OPTIONS;
    const p = parsed as Partial<Record<keyof BoostOptions, unknown>>;
    const pick = (k: keyof BoostOptions) => (typeof p[k] === "boolean" ? (p[k] as boolean) : DEFAULT_OPTIONS[k]);
    return {
      purgeRam: pick("purgeRam"),
      flushDns: pick("flushDns"),
      performancePlan: pick("performancePlan"),
      highPriority: pick("highPriority"),
      noThrottling: pick("noThrottling"),
    };
  } catch {
    return DEFAULT_OPTIONS;
  }
}

const GAME_KEY = "aether.boost.games";

/** Options Boost propres à certains jeux (les autres utilisent les options par défaut). */
function loadGameOptions(): Record<string, BoostOptions> {
  try {
    const raw = window.localStorage.getItem(GAME_KEY);
    const parsed: unknown = raw ? JSON.parse(raw) : {};
    if (typeof parsed !== "object" || parsed === null) return {};
    const out: Record<string, BoostOptions> = {};
    for (const [id, v] of Object.entries(parsed as Record<string, unknown>)) {
      if (typeof v !== "object" || v === null) continue;
      const p = v as Partial<Record<keyof BoostOptions, unknown>>;
      const pick = (k: keyof BoostOptions) => (typeof p[k] === "boolean" ? (p[k] as boolean) : DEFAULT_OPTIONS[k]);
      out[id] = {
        purgeRam: pick("purgeRam"),
        flushDns: pick("flushDns"),
        performancePlan: pick("performancePlan"),
        highPriority: pick("highPriority"),
        noThrottling: pick("noThrottling"),
      };
    }
    return out;
  } catch {
    return {};
  }
}

function saveGameOptions(value: Record<string, BoostOptions>): void {
  try {
    window.localStorage.setItem(GAME_KEY, JSON.stringify(value));
  } catch {
    /* préférence non mémorisée */
  }
}

// Images et liste conservées en mémoire pour la durée de la session (navigation instantanée).
const imageCache = new Map<string, string | null>();
let gamesCache: Game[] | null = null;

/** Précharge la liste des jeux (écran de chargement). */
export async function preloadGames(): Promise<void> {
  try {
    gamesCache = await api.gamesList(false);
  } catch {
    gamesCache = [];
  }
}

export function GamesPage() {
  const { m } = useI18n();
  const toast = useApp((s) => s.toast);
  const [games, setGames] = useState<Game[] | null>(gamesCache);
  const [query, setQuery] = useState("");
  const [options, setOptions] = useState<BoostOptions>(loadOptions);
  const [gameOptions, setGameOptions] = useState<Record<string, BoostOptions>>(loadGameOptions);
  const [editing, setEditing] = useState<Game | null>(null);
  const [autoBoost, setAutoBoost] = useState(false);
  useEffect(() => {
    void api
      .autoboostGet()
      .then(setAutoBoost)
      .catch(() => undefined);
  }, []);
  const [active, setActive] = useState<Game | null>(null);
  const [steps, setSteps] = useState<Partial<Record<BoostStep, BoostEvent>>>({});
  const [refreshing, setRefreshing] = useState(false);

  const load = useCallback(
    async (force: boolean) => {
      setRefreshing(true);
      try {
        const list = await api.gamesList(force);
        gamesCache = list;
        setGames(list);
      } catch (e) {
        toast("error", errorMessage(e));
        setGames([]);
      } finally {
        setRefreshing(false);
      }
    },
    [toast],
  );

  useEffect(() => {
    if (!gamesCache) void load(false);
    // Reprend l'affichage si une session est déjà en cours.
    void api.gameSession().then((id) => {
      if (id && gamesCache) setActive(gamesCache.find((g) => g.id === id) ?? null);
    });
  }, [load]);

  const setOption = (k: keyof BoostOptions, v: boolean) => {
    const next = { ...options, [k]: v };
    setOptions(next);
    try {
      window.localStorage.setItem("aether.boost", JSON.stringify(next));
    } catch {
      /* préférence non mémorisée */
    }
  };

  const play = async (game: Game) => {
    setActive(game);
    setSteps({});
    document.getElementById("aether-scroll")?.scrollTo({ top: 0, behavior: "smooth" });
    try {
      await api.gameLaunch(game.id, gameOptions[game.id] ?? options, (e) => setSteps((s) => ({ ...s, [e.step]: e })));
      toast("success", fmt(m.games.ended, { name: game.name }));
    } catch (e) {
      toast("error", errorMessage(e));
    } finally {
      setActive(null);
    }
  };

  const filtered = useMemo(() => (games ?? []).filter((g) => g.name.toLowerCase().includes(query.trim().toLowerCase())), [games, query]);

  const optionRows: { key: keyof BoostOptions; title: string; desc: string; icon: LucideIcon }[] = [
    { key: "purgeRam", title: m.games.optRam, desc: m.games.optRamDesc, icon: MemoryStick },
    { key: "flushDns", title: m.games.optDns, desc: m.games.optDnsDesc, icon: Globe },
    { key: "performancePlan", title: m.games.optPlan, desc: m.games.optPlanDesc, icon: Zap },
    { key: "highPriority", title: m.games.optPriority, desc: m.games.optPriorityDesc, icon: Gauge },
    { key: "noThrottling", title: m.games.optThrottle, desc: m.games.optThrottleDesc, icon: Cpu },
  ];

  return (
    <div className="space-y-10">
      <PageHeader
        section="games"
        title={m.games.title}
        subtitle={m.games.subtitle}
        actions={
          <Button icon={<RefreshCw className="size-4" />} loading={refreshing} onClick={() => void load(true)}>
            {m.games.refresh}
          </Button>
        }
      />

      {active && <SessionCard game={active} steps={steps} />}

      <Card id="gm-boost" className="">
        <CardHeader icon={<Rocket className="size-[18px]" />} tone="violet" title={m.games.boost} description={m.games.boostDesc} />
        {/* Liste en une colonne, comme les réglages des autres pages. */}
        <div className="mb-2 divide-y divide-white/[0.05] border-t border-white/[0.05]">
          <label className="flex min-w-0 items-center gap-4 px-6 py-3.5 transition-colors hover:bg-white/[0.02]">
            <span
              className={cx(
                "grid size-9 shrink-0 place-items-center rounded-[10px] border transition-colors duration-200",
                autoBoost
                  ? "border-[var(--good)]/20 bg-[var(--good)]/[0.07] text-[var(--good)]"
                  : "border-white/[0.06] bg-white/[0.02] text-[var(--steel)]",
              )}
            >
              <Radar className="size-[17px]" strokeWidth={1.6} aria-hidden />
            </span>
            <div className="min-w-0 flex-1">
              <div className="truncate text-[14px] text-zinc-100">{m.games.autoBoost}</div>
              <div className="mt-0.5 truncate text-[12.5px] text-zinc-500" title={m.games.autoBoostDesc}>
                {m.games.autoBoostDesc}
              </div>
            </div>
            <Toggle
              checked={autoBoost}
              label={m.games.autoBoost}
              onChange={(v) =>
                void api
                  .autoboostSet(v)
                  .then(setAutoBoost)
                  .catch((e: unknown) => toast("error", errorMessage(e)))
              }
            />
          </label>
          {optionRows.map((o) => (
            <label key={o.key} className="flex min-w-0 items-center gap-4 px-6 py-3.5 transition-colors hover:bg-white/[0.02]">
              <span
                className={cx(
                  "grid size-9 shrink-0 place-items-center rounded-[10px] border transition-colors duration-200",
                  options[o.key]
                    ? "border-[var(--good)]/20 bg-[var(--good)]/[0.07] text-[var(--good)]"
                    : "border-white/[0.06] bg-white/[0.02] text-[var(--steel)]",
                )}
              >
                <o.icon className="size-[17px]" strokeWidth={1.6} aria-hidden />
              </span>
              <div className="min-w-0 flex-1">
                <div className="truncate text-[14px] text-zinc-100">{o.title}</div>
                <div className="mt-0.5 truncate text-[12.5px] text-zinc-500">{o.desc}</div>
              </div>
              <Toggle checked={options[o.key]} onChange={(v) => setOption(o.key, v)} label={o.title} disabled={active !== null} />
            </label>
          ))}
        </div>
      </Card>

      <Card id="gm-library" className="">
        <CardHeader
          icon={<Gamepad2 className="size-[18px]" />}
          tone="sky"
          title={m.games.library}
          description={m.games.libraryDesc}
          actions={
            <div className="flex h-9 w-64 items-center gap-2 rounded-[8px] border border-white/[0.08] bg-[#0c0d10] px-3">
              <Search className="size-4 text-zinc-500" aria-hidden />
              <input
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                placeholder={m.games.search}
                className="min-w-0 flex-1 bg-transparent text-[13px] text-white outline-none placeholder:text-zinc-500"
              />
            </div>
          }
        />
        {games === null && (
          <div className="grid grid-cols-1 gap-4 px-5 pb-5 sm:grid-cols-2 lg:grid-cols-3">
            {[0, 1, 2, 3].map((k) => (
              <Skeleton key={k} className="aspect-[460/215] rounded-[10px]" />
            ))}
          </div>
        )}
        {games !== null && filtered.length === 0 && <Empty>{m.games.empty}</Empty>}
        {filtered.length > 0 && (
          <div className="grid grid-cols-1 gap-4 px-5 pb-5 sm:grid-cols-2 lg:grid-cols-3">
            {filtered.map((g) => (
              <GameTile
                key={g.id}
                game={g}
                busy={active !== null}
                playing={active?.id === g.id}
                custom={Boolean(gameOptions[g.id])}
                onPlay={() => void play(g)}
                onSettings={() => setEditing(g)}
              />
            ))}
          </div>
        )}
      </Card>

      <Modal
        open={editing !== null}
        title={editing ? fmt(m.games.profileTitle, { name: editing.name }) : ""}
        onClose={() => setEditing(null)}
        footer={
          <>
            {editing && gameOptions[editing.id] && (
              <Button
                variant="ghost"
                onClick={() => {
                  const next = { ...gameOptions };
                  delete next[editing.id];
                  setGameOptions(next);
                  saveGameOptions(next);
                  setEditing(null);
                }}
              >
                {m.games.profileReset}
              </Button>
            )}
            <Button variant="primary" onClick={() => setEditing(null)}>
              {m.common.close}
            </Button>
          </>
        }
      >
        {editing && (
          <div className="-mx-1 divide-y divide-white/[0.05]">
            <p className="px-1 pb-3 text-[12.5px] text-zinc-400">{m.games.profileHint}</p>
            {optionRows.map((o) => {
              const current = gameOptions[editing.id] ?? options;
              return (
                <label key={o.key} title={o.desc} className="flex min-w-0 items-center gap-3.5 px-1 py-3">
                  <span className="min-w-0 flex-1 truncate text-[13.5px] text-zinc-100">{o.title}</span>
                  <Toggle
                    checked={current[o.key]}
                    label={o.title}
                    onChange={(v) => {
                      const next = { ...gameOptions, [editing.id]: { ...current, [o.key]: v } };
                      setGameOptions(next);
                      saveGameOptions(next);
                    }}
                  />
                </label>
              );
            })}
          </div>
        )}
      </Modal>
    </div>
  );
}

function GameTile({
  game,
  busy,
  playing,
  custom,
  onPlay,
  onSettings,
}: {
  game: Game;
  busy: boolean;
  playing: boolean;
  custom: boolean;
  onPlay: () => void;
  onSettings: () => void;
}) {
  const { m } = useI18n();
  const [image, setImage] = useState<string | null>(imageCache.get(game.id) ?? null);

  useEffect(() => {
    if (!game.hasImage || imageCache.has(game.id)) return;
    let alive = true;
    void api
      .gameImage(game.id)
      .then((src) => {
        imageCache.set(game.id, src);
        if (alive) setImage(src);
      })
      .catch(() => imageCache.set(game.id, null));
    return () => {
      alive = false;
    };
  }, [game]);

  const initials = game.name
    .split(/\s+/)
    .slice(0, 2)
    .map((w) => w[0] ?? "")
    .join("")
    .toUpperCase();

  return (
    <div
      className={cx(
        "group overflow-hidden rounded-[10px] border bg-[#111216] transition-colors duration-200",
        playing ? "border-[var(--accent)]" : "border-white/[0.07] hover:border-white/15",
      )}
    >
      <div className="relative aspect-[460/215] overflow-hidden bg-gradient-to-br from-[#1b1d26] to-[#101116]">
        {image ? (
          <img
            src={image}
            alt=""
            className="size-full object-cover transition-transform duration-500 ease-out group-hover:scale-[1.03]"
            draggable={false}
          />
        ) : (
          <div className="grid size-full place-items-center text-[28px] font-semibold tracking-tight text-zinc-600">{initials}</div>
        )}
        <span className="absolute top-2.5 left-2.5 rounded-md bg-black/75 px-2 py-0.5 text-[10.5px] font-medium text-zinc-200">
          {SOURCE_LABEL[game.source]}
        </span>
      </div>
      <div className="flex items-center gap-3 p-3.5">
        <span className="min-w-0 flex-1 truncate text-[13.5px] font-medium text-white" title={game.name}>
          {game.name}
        </span>
        <button
          type="button"
          onClick={onSettings}
          title={custom ? `${m.games.profileCustom} · ${m.games.profileEdit}` : m.games.profileEdit}
          aria-label={m.games.profileEdit}
          className={cx(
            "grid size-8 shrink-0 place-items-center rounded-[8px] transition-colors hover:bg-white/[0.06]",
            custom ? "text-[var(--accent)]" : "text-zinc-500 hover:text-zinc-200",
          )}
        >
          <SlidersHorizontal className="size-4" aria-hidden />
        </button>
        <Button
          size="sm"
          variant={playing ? "secondary" : "primary"}
          disabled={busy}
          icon={playing ? <LoaderCircle className="size-3.5 animate-spin" /> : <Play className="size-3.5" />}
          onClick={onPlay}
        >
          {playing ? m.games.playing : m.games.launch}
        </Button>
      </div>
    </div>
  );
}

function SessionCard({ game, steps }: { game: Game; steps: Partial<Record<BoostStep, BoostEvent>> }) {
  const { m } = useI18n();
  const icon = (e: BoostEvent | undefined) => {
    if (!e) return <CircleDashed className="size-4 text-zinc-600" />;
    switch (e.status) {
      case "running":
        return <LoaderCircle className="size-4 animate-spin text-cyan" />;
      case "done":
        return <CircleCheck className="size-4 text-emerald" />;
      case "error":
        return <CircleX className="size-4 text-rose" />;
      default:
        return <MinusCircle className="size-4 text-zinc-500" />;
    }
  };
  return (
    <Card className="">
      <CardHeader
        icon={<Gamepad2 className="size-[18px]" />}
        tone="emerald"
        title={`${m.games.progress} · ${game.name}`}
        actions={
          <Button size="sm" icon={<Square className="size-3.5" />} onClick={() => void api.gameSessionStop()}>
            {m.games.stopTracking}
          </Button>
        }
      />
      <ol className="grid gap-2 px-5 pb-5 sm:grid-cols-2 xl:grid-cols-4">
        {STEPS.filter((s) => s !== "restore" || steps.restore).map((s) => {
          const e = steps[s];
          return (
            <li key={s} className={cx("surface-2 flex items-center gap-2.5 rounded-[8px] px-3 py-2.5", !e && "opacity-60")}>
              {icon(e)}
              <div className="min-w-0">
                <div className="truncate text-[12.5px] text-zinc-100">{m.games.steps[s]}</div>
                {e?.detail && <div className="truncate text-[11px] text-[var(--text-3)]">{e.detail}</div>}
              </div>
            </li>
          );
        })}
      </ol>
      <div className="flex items-center gap-2 border-t border-white/[0.05] px-6 py-3 text-[12px] text-[var(--text-3)]">
        <IconBadge tone="slate" size="sm">
          <Gamepad2 className="size-4" />
        </IconBadge>
        <span className="truncate">{game.installDir}</span>
      </div>
    </Card>
  );
}
