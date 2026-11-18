// Choix du niveau (Débutant / Joueur / Expérimenté) : premier lancement, Réglages et menu latéral.
import { Gamepad2, GraduationCap, Wrench } from "lucide-react";
import { useI18n } from "../i18n";
import { LEVELS } from "../lib/level";
import type { UserLevel } from "../lib/types";
import { useApp } from "../store/app";
import { cx } from "./ui";

export const LEVEL_ICON: Record<UserLevel, typeof Gamepad2> = { beginner: GraduationCap, gamer: Gamepad2, expert: Wrench };

/** Grandes cartes avec description (premier lancement). */
export function LevelCards() {
  const { m } = useI18n();
  const level = useApp((s) => s.level);
  const setLevel = useApp((s) => s.setLevel);
  return (
    <div className="space-y-2" role="radiogroup" aria-label={m.level.title}>
      {LEVELS.map((id) => {
        const Icon = LEVEL_ICON[id];
        const active = id === level;
        return (
          <button
            key={id}
            type="button"
            role="radio"
            aria-checked={active}
            onClick={() => setLevel(id)}
            className={cx(
              "flex w-full items-center gap-4 rounded-xl border p-4 text-left transition",
              active ? "border-indigo/50 bg-indigo/10" : "border-white/[0.07] bg-white/[0.02] hover:border-white/15",
            )}
          >
            <Icon className={cx("size-5 shrink-0", active ? "text-cyan" : "text-slate-400")} aria-hidden />
            <div className="min-w-0">
              <div className="text-[13.5px] font-semibold text-white">{m.level[id]}</div>
              <div className="text-[12px] text-[var(--text-3)]">{m.level[`${id}Desc`]}</div>
            </div>
          </button>
        );
      })}
    </div>
  );
}

/** Sélecteur compact à trois boutons (Réglages, menu latéral). */
export function LevelSegmented({ compactLabels = false }: { compactLabels?: boolean }) {
  const { m } = useI18n();
  const level = useApp((s) => s.level);
  const setLevel = useApp((s) => s.setLevel);
  return (
    <div className="flex rounded-[10px] border border-white/[0.07] bg-white/[0.02] p-0.5" role="radiogroup" aria-label={m.level.title}>
      {LEVELS.map((id) => {
        const Icon = LEVEL_ICON[id];
        const active = id === level;
        return (
          <button
            key={id}
            type="button"
            role="radio"
            aria-checked={active}
            title={m.level[`${id}Desc`]}
            onClick={() => setLevel(id)}
            className={cx(
              "flex min-w-0 flex-1 items-center justify-center gap-1.5 rounded-[8px] px-2 py-1.5 text-[12px] transition-colors",
              active ? "bg-white/[0.08] text-white" : "text-zinc-400 hover:text-zinc-100",
            )}
          >
            <Icon className="size-3.5 shrink-0" aria-hidden />
            {!compactLabels && <span className="truncate">{m.level[id]}</span>}
          </button>
        );
      })}
    </div>
  );
}
