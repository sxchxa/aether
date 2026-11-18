// Menu latéral (maquette « premium dark ») : logo et signature, navigation hiérarchisée avec pastille
// glissante, compteurs alignés, actions rapides, options avancées et version.
import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { Check, LoaderCircle, MemoryStick, RefreshCw, ShieldPlus, Trash2 } from "lucide-react";
import { useI18n } from "../i18n";
import { formatBytes } from "../lib/format";
import { api, errorMessage } from "../lib/ipc";
import { isPreference, SECTION_DEFS } from "../lib/navigation";
import { useMediaQuery } from "../lib/useMediaQuery";
import { useApp, type SectionId } from "../store/app";
import { LEVEL_ICON, LevelSegmented } from "./LevelPicker";
import { LEVELS } from "../lib/level";
import { Logo } from "./Logo";
import { cx } from "./ui";

export const SECTIONS = SECTION_DEFS.map((s) => ({ id: s.id, icon: s.icon }));

const NAV_GROUPS = ["main", "optimize", "maintain"] as const;

/** Version de l'application (lue une fois). */
function useAppVersion(): string {
  const [version, setVersion] = useState("");
  useEffect(() => {
    void getVersion()
      .then(setVersion)
      .catch(() => undefined);
  }, []);
  return version;
}

export function Sidebar() {
  const { m } = useI18n();
  const section = useApp((s) => s.section);
  const goTo = useApp((s) => s.goTo);
  // Fenêtres étroites ou peu hautes : barre d'icônes compacte.
  const compact = useMediaQuery("(max-width: 1100px), (max-height: 640px)");
  // Fenêtre peu haute : éléments resserrés.
  const short = useMediaQuery("(max-height: 820px)");
  const groups = NAV_GROUPS.map((g) => ({ g, items: SECTION_DEFS.filter((s) => s.group === g) }));
  const listRef = useRef<HTMLDivElement>(null);
  const pillRef = useRef<HTMLSpanElement>(null);

  // Repère de la section active : une pastille qui glisse d'un élément à l'autre (transform, GPU).
  useLayoutEffect(() => {
    const list = listRef.current;
    const pill = pillRef.current;
    if (!list || !pill) return;
    const place = () => {
      const item = list.querySelector<HTMLElement>(`[data-nav="${section}"]`);
      if (!item) {
        pill.style.opacity = "0";
        return;
      }
      pill.style.height = `${item.offsetHeight}px`;
      pill.style.transform = `translateY(${item.offsetTop}px)`;
      pill.style.opacity = "1";
    };
    place();
    const ro = new ResizeObserver(place);
    ro.observe(list);
    return () => ro.disconnect();
  }, [section, short, compact]);

  if (compact) {
    return (
      <nav className="sidebar-panel flex w-[76px] shrink-0 flex-col items-center py-4" aria-label="Aether">
        <div className="mb-4 grid size-11 place-items-center" data-tauri-drag-region>
          <Logo size={30} />
        </div>
        <div className="scroll-area flex min-h-0 w-full flex-1 flex-col items-center gap-1 px-2">
          {groups.map(({ g, items }, gi) => (
            <div key={g} className={cx("flex w-full flex-col items-center gap-1", gi > 0 && "mt-2 border-t border-white/[0.05] pt-2")}>
              {items.map((def) => {
                const Icon = def.icon;
                const active = def.id === section;
                return (
                  <button
                    key={def.id}
                    type="button"
                    onClick={() => goTo(def.id)}
                    title={m.nav[def.id]}
                    aria-label={m.nav[def.id]}
                    aria-current={active ? "page" : undefined}
                    className={cx(
                      "grid size-11 place-items-center rounded-[12px] transition-colors",
                      active ? "nav-active text-white" : "text-[var(--steel)] hover:bg-white/[0.04] hover:text-zinc-100",
                    )}
                  >
                    <Icon className="size-[19px]" strokeWidth={1.6} aria-hidden />
                  </button>
                );
              })}
            </div>
          ))}
        </div>
        <CompactSettings />
      </nav>
    );
  }

  return (
    <nav className="sidebar-panel flex w-[272px] shrink-0 flex-col" aria-label="Aether">
      {/* En-tête : logo et signature (zone déplaçable de la fenêtre). */}
      <div className="flex items-center gap-3.5 px-6 pt-7 pb-6" data-tauri-drag-region>
        <Logo size={42} />
        <div className="min-w-0" data-tauri-drag-region>
          <div className="text-[18px] leading-tight font-semibold tracking-[-0.01em] text-zinc-50" data-tauri-drag-region>
            {m.app.name}
          </div>
          <div className="truncate text-[12px] text-zinc-500" data-tauri-drag-region>
            {m.app.tagline}
          </div>
        </div>
      </div>

      <div className="scroll-area min-h-0 flex-1 px-4 pb-3">
        <div ref={listRef} className="relative">
          <span ref={pillRef} className="nav-pill" aria-hidden />
          {groups.map(({ g, items }, gi) => (
            <div
              key={g}
              className={cx(gi > 0 && (short ? "mt-3 border-t border-white/[0.05] pt-3" : "mt-4 border-t border-white/[0.05] pt-4"))}
            >
              {gi > 0 && <div className="mb-1.5 px-3 text-[12.5px] text-zinc-500">{m.navGroups[g]}</div>}
              <div className="space-y-0.5">
                {items.map((def) => (
                  <NavItem
                    key={def.id}
                    id={def.id}
                    large={g === "main"}
                    active={def.id === section}
                    short={short}
                    onGo={() => goTo(def.id)}
                  />
                ))}
              </div>
            </div>
          ))}
        </div>
        <QuickActions />
      </div>
      <SidebarFooter />
    </nav>
  );
}

function NavItem({ id, active, short, large, onGo }: { id: SectionId; active: boolean; short: boolean; large: boolean; onGo: () => void }) {
  const { m } = useI18n();
  const def = SECTION_DEFS.find((s) => s.id === id);
  if (!def) return null;
  const Icon = def.icon;
  return (
    <button
      type="button"
      onClick={onGo}
      data-nav={id}
      aria-current={active ? "page" : undefined}
      className={cx(
        "group relative flex w-full items-center gap-3.5 rounded-[12px] px-3.5 text-left transition-colors duration-200",
        large ? (short ? "py-2.5" : "py-3") : short ? "py-2" : "py-2.5",
        active ? "text-white" : "text-zinc-400 hover:bg-white/[0.03] hover:text-zinc-100",
      )}
    >
      <Icon
        className={cx(
          "shrink-0 transition-colors",
          large ? "size-[20px]" : "size-[19px]",
          active ? "text-[var(--accent-soft)]" : "text-[var(--steel)] group-hover:text-zinc-200",
        )}
        strokeWidth={1.6}
        aria-hidden
      />
      <span className={cx("flex-1 truncate", large ? "text-[14.5px]" : "text-[14px]")}>{m.nav[id]}</span>
    </button>
  );
}

/** Bas du menu : niveau de l'utilisateur et version. */
function SidebarFooter() {
  const { m } = useI18n();
  const level = useApp((s) => s.level);
  const version = useAppVersion();
  return (
    <div className="px-4 pt-2 pb-4">
      <div className="rounded-[14px] border border-white/[0.06] bg-white/[0.02] px-3 py-2.5">
        <div className="mb-2 text-[11.5px] text-[var(--text-3)]">{m.level.title}</div>
        <LevelSegmented compactLabels />
        <div className="mt-1.5 truncate text-center text-[12px] text-zinc-200">{m.level[level]}</div>
      </div>
      {version && <div className="tabular mt-3 text-center text-[11.5px] text-zinc-600">Aether v{version}</div>}
    </div>
  );
}

type QuickKey = "ram" | "temp" | "dns" | "point";

/** Actions en un clic, avec retour immédiat du résultat. */
function QuickActions() {
  const { m, lang } = useI18n();
  const toast = useApp((s) => s.toast);
  const runScan = useApp((s) => s.runScan);
  const restore = useApp((s) => s.restore);
  const setRestore = useApp((s) => s.setRestore);
  const [state, setState] = useState<Partial<Record<QuickKey, { busy: boolean; result?: string }>>>({});

  const run = async (key: QuickKey, action: () => Promise<string>) => {
    setState((s) => ({ ...s, [key]: { busy: true } }));
    try {
      const result = await action();
      setState((s) => ({ ...s, [key]: { busy: false, result } }));
      window.setTimeout(() => setState((s) => ({ ...s, [key]: undefined })), 4000);
    } catch (e) {
      setState((s) => ({ ...s, [key]: undefined }));
      toast("error", errorMessage(e));
    }
  };

  const actions: { key: QuickKey; label: string; icon: ReactNode; disabled?: boolean; action: () => Promise<string> }[] = [
    {
      key: "ram",
      label: m.quick.ram,
      icon: <MemoryStick className="size-[18px]" strokeWidth={1.6} />,
      action: async () => formatBytes((await api.ramPurge()).freed, lang),
    },
    {
      key: "temp",
      label: m.quick.temp,
      icon: <Trash2 className="size-[18px]" strokeWidth={1.6} />,
      action: async () => {
        const r = await api.storageClean(["sys.userTemp", "sys.windowsTemp"]);
        void runScan(true);
        return r.preview ? m.dryRun.label : formatBytes(r.freed, lang);
      },
    },
    {
      key: "dns",
      label: m.quick.dns,
      icon: <RefreshCw className="size-[18px]" strokeWidth={1.6} />,
      action: async () => {
        await api.flushDns();
        return m.quick.done;
      },
    },
    {
      key: "point",
      label: m.quick.point,
      icon: <ShieldPlus className="size-[18px]" strokeWidth={1.6} />,
      disabled: !restore?.enabled,
      action: async () => {
        setRestore(await api.restoreCreate("Aether"));
        return m.quick.done;
      },
    },
  ];

  return (
    <div className="mt-5 border-t border-white/[0.05] px-1 pt-4">
      <div className="mb-2.5 px-2 text-[12.5px] text-zinc-500">{m.quick.title}</div>
      <div className="grid grid-cols-4 gap-2">
        {actions.map((a) => {
          const st = state[a.key];
          return (
            <button
              key={a.key}
              type="button"
              title={st?.result ? `${a.label} · ${st.result}` : a.label}
              aria-label={a.label}
              disabled={a.disabled || st?.busy}
              onClick={() => void run(a.key, a.action)}
              className={cx(
                "grid aspect-square place-items-center rounded-[12px] border border-white/[0.06] bg-white/[0.025] transition-colors hover:border-white/[0.12] hover:bg-white/[0.05] disabled:opacity-40",
                st?.result ? "text-[var(--good)]" : "text-[var(--steel)] hover:text-zinc-100",
              )}
            >
              {st?.busy ? (
                <LoaderCircle className="size-[18px] animate-spin" aria-hidden />
              ) : st?.result ? (
                <Check className="size-[18px]" aria-hidden />
              ) : (
                a.icon
              )}
            </button>
          );
        })}
      </div>
    </div>
  );
}

function CompactSettings() {
  const { m } = useI18n();
  const level = useApp((s) => s.level);
  const setLevel = useApp((s) => s.setLevel);
  const Icon = LEVEL_ICON[level];
  const next = LEVELS[(LEVELS.indexOf(level) + 1) % LEVELS.length] ?? "beginner";
  return (
    <div className="mt-2 flex flex-col items-center gap-1.5 border-t border-white/[0.05] pt-3">
      <button
        type="button"
        onClick={() => setLevel(next)}
        title={`${m.level.title} : ${m.level[level]}`}
        className="grid size-11 place-items-center rounded-[12px] text-[var(--accent-soft)] hover:bg-white/[0.04]"
      >
        <Icon className="size-[18px]" strokeWidth={1.6} aria-hidden />
      </button>
    </div>
  );
}

export function useOptimizedCount(): { done: number; total: number } {
  // Valeur primitive « fait/total » : pas de nouveau rendu tant que le compte ne change pas.
  const key = useApp((s) => {
    const list = Object.values(s.tweaks).filter((t) => t.probe.available && t.risk !== "advanced" && !isPreference(t));
    return `${list.filter((t) => t.probe.applied).length}/${list.length}`;
  });
  const [done, total] = key.split("/").map(Number);
  return { done: done ?? 0, total: total ?? 0 };
}
