// Carte d'option compacte en relief : badge d'icône 3D + titre + risque + interrupteur (+ inspecteur).
import { memo, useEffect, useRef } from "react";
import { CircleHelp, RotateCcw } from "lucide-react";
import { useI18n } from "../i18n";
import { tweakIcon } from "../lib/catalog";
import { api, errorMessage } from "../lib/ipc";

/** Options dont l'effet nécessite un redémarrage de l'Explorateur. */
const RESTART_EXPLORER = ["ui.classicMenu"];
import { useApp } from "../store/app";
import { riskAllowed } from "../lib/level";
import { cx, RiskBadge, Skeleton, Toggle } from "./ui";

// Mémorisé : une ligne ne se redessine que si SON option change.
export const TweakCard = memo(function TweakCard({ id }: { id: string }) {
  const { m } = useI18n();
  const status = useApp((s) => s.tweaks[id]);
  const busy = useApp((s) => Boolean(s.busy[id]));
  const touched = useApp((s) => Boolean(s.touched[id]));
  const setTweak = useApp((s) => s.setTweak);
  const inspect = useApp((s) => s.inspect);
  const toast = useApp((s) => s.toast);
  const text = (m.tweaks as Record<string, { title: string; role: string; gain: string } | undefined>)[id];
  const Icon = tweakIcon(id);
  // Survol du « ? » : le panneau d’explication s’ouvre et se referme dès que la souris quitte
  // le « ? ». Un clic, lui, l’épingle (il reste ouvert jusqu’à sa fermeture).
  const hoverTimer = useRef<number | undefined>(undefined);
  const openedByHover = useRef(false);
  useEffect(() => () => window.clearTimeout(hoverTimer.current), []);
  const hoverStart = () => {
    window.clearTimeout(hoverTimer.current);
    hoverTimer.current = window.setTimeout(() => {
      openedByHover.current = true;
      inspect({ kind: "tweak", id });
    }, 250);
  };
  const hoverEnd = () => {
    window.clearTimeout(hoverTimer.current);
    if (!openedByHover.current) return;
    openedByHover.current = false;
    const current = useApp.getState().inspector;
    if (current?.kind === "tweak" && current.id === id) inspect(null);
  };

  if (!status) {
    return (
      <div className="surface-2 flex items-center gap-3 rounded-[10px] px-4 py-3.5">
        <Skeleton className="size-9" />
        <Skeleton className="h-4 flex-1" />
        <Skeleton className="h-6 w-11 rounded-full" />
      </div>
    );
  }

  const { probe } = status;
  const applied = probe.applied;
  const alreadyActive = applied && !touched;

  const onToggle = async (value: boolean) => {
    const outcome = await setTweak(id, value);
    if (outcome === "preview") toast("warn", m.dryRun.simulated);
    else if (outcome) toast(outcome === "applied" ? "success" : "info", m.outcome[outcome]);
    // Le menu clic-droit n'est relu qu'au démarrage de l'Explorateur : redémarrage automatique.
    if (outcome === "applied" && RESTART_EXPLORER.includes(id)) {
      try {
        await api.restartExplorer();
        toast("success", m.system.restartExplorer);
      } catch (e) {
        toast("error", errorMessage(e));
      }
    }
  };

  // Ligne sobre, façon Paramètres de Windows : l'état se lit sur l'interrupteur ;
  // seules les informations utiles s'affichent (risque non nul, indisponibilité, redémarrage, erreur).
  const note = !probe.available
    ? probe.current || m.common.unavailable
    : status.error
      ? status.error
      : status.needsReboot && applied && touched
        ? m.common.rebootNeeded
        : null;
  return (
    <div
      title={alreadyActive ? `${text?.gain ?? ""} · ${m.common.alreadyActive}` : text?.gain}
      className={cx(
        "group flex min-w-0 items-center gap-4 px-6 py-3.5 transition-colors duration-150 hover:bg-white/[0.02]",
        !probe.available && "opacity-50",
      )}
    >
      <span
        className={cx(
          "grid size-9 shrink-0 place-items-center rounded-[10px] border transition-colors duration-200",
          applied
            ? "border-[var(--good)]/20 bg-[var(--good)]/[0.07] text-[var(--good)]"
            : "border-white/[0.06] bg-white/[0.02] text-[var(--steel)]",
        )}
      >
        <Icon className="size-[17px]" strokeWidth={1.6} aria-hidden />
      </span>
      <div className="min-w-0 flex-1">
        <div className="flex min-w-0 items-center gap-2">
          <span className="min-w-0 truncate text-[14px] text-zinc-100">{text?.title ?? id}</span>
          {status.risk !== "safe" && (
            <span className="shrink-0">
              <RiskBadge risk={status.risk} />
            </span>
          )}
          <button
            type="button"
            aria-label={m.common.whatIsThis}
            onClick={() => {
              window.clearTimeout(hoverTimer.current);
              // Clic : le panneau reste ouvert, même après le départ de la souris.
              openedByHover.current = false;
              inspect({ kind: "tweak", id });
            }}
            onMouseEnter={hoverStart}
            onMouseLeave={hoverEnd}
            className="shrink-0 text-zinc-600 opacity-0 transition group-hover:opacity-100 hover:text-zinc-200 focus-visible:opacity-100"
          >
            <CircleHelp className="size-3.5" />
          </button>
        </div>
        {/* Description secondaire (rôle du réglage), sur une ligne. */}
        {!note && text?.role && <div className="mt-0.5 truncate text-[12.5px] text-zinc-500">{text.role}</div>}
        {note && (
          <div
            className={cx(
              "mt-0.5 flex min-w-0 items-center gap-1 text-[11.5px]",
              status.error ? "text-rose-300" : probe.available ? "text-amber-300" : "text-zinc-500",
            )}
          >
            {status.needsReboot && applied && touched && <RotateCcw className="size-3 shrink-0" aria-hidden />}
            <span className="truncate">{note}</span>
          </div>
        )}
      </div>
      <Toggle checked={applied} onChange={onToggle} disabled={!probe.available} busy={busy} label={text?.title ?? id} />
    </div>
  );
});

/** Liste d'options séparées par de fines lignes, filtrée selon le niveau de l'utilisateur. */
export function TweakList({ ids }: { ids: string[]; columns?: 1 | 2 }) {
  // Sélecteur à valeur simple (chaîne) : la liste ne se redessine que si les options visibles changent.
  const key = useApp((s) => ids.filter((id) => riskAllowed(s.level, s.tweaks[id]?.risk)).join("|"));
  const visible = key ? key.split("|") : [];
  return (
    <div className="mb-2 divide-y divide-white/[0.05] border-t border-white/[0.05]">
      {visible.map((id) => (
        <TweakCard key={id} id={id} />
      ))}
    </div>
  );
}
