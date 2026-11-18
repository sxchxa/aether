// Commandes de fenêtre flottantes (fenêtre sans cadre, comme la maquette) : historique, réglages,
// réduire, agrandir, fermer. La zone supérieure reste déplaçable. Ctrl+K ouvre la palette.
import { useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { CircleHelp, Copy, History, Minus, ScrollText, Settings, Square, Undo2, X } from "lucide-react";
import { fmt, useI18n } from "../i18n";
import { api, errorMessage } from "../lib/ipc";
import { useApp } from "../store/app";
import { cx } from "./ui";

export function useUndo() {
  const { m } = useI18n();
  const toast = useApp((s) => s.toast);
  const refreshHistory = useApp((s) => s.refreshHistory);
  const loadTweaks = useApp((s) => s.loadTweaks);
  return async () => {
    try {
      const entry = await api.historyUndo();
      if (entry) {
        toast("success", fmt(m.toast.undone, { label: entry.label }));
        await loadTweaks();
      } else {
        toast("info", m.toast.nothingToUndo);
      }
      await refreshHistory();
    } catch (e) {
      toast("error", errorMessage(e));
    }
  };
}

/** Bande supérieure déplaçable + commandes de fenêtre, posées au-dessus du contenu. */
export function TitleBar() {
  const { m, lang } = useI18n();
  const history = useApp((s) => s.history);
  const section = useApp((s) => s.section);
  const goTo = useApp((s) => s.goTo);
  const logsOpen = useApp((s) => s.logsOpen);
  const setLogsOpen = useApp((s) => s.setLogsOpen);
  const undo = useUndo();
  const [maximized, setMaximized] = useState(false);
  const [historyOpen, setHistoryOpen] = useState(false);
  const zoneRef = useRef<HTMLDivElement>(null);
  const win = getCurrentWindow();

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void win.isMaximized().then(setMaximized);
    void win
      .onResized(() => {
        void win.isMaximized().then(setMaximized);
      })
      .then((fn) => {
        unlisten = fn;
      });
    return () => unlisten?.();
  }, [win]);

  useEffect(() => {
    if (!historyOpen) return;
    const onDown = (e: MouseEvent) => {
      if (zoneRef.current && e.target instanceof Node && !zoneRef.current.contains(e.target)) setHistoryOpen(false);
    };
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && setHistoryOpen(false);
    window.addEventListener("mousedown", onDown);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("mousedown", onDown);
      window.removeEventListener("keydown", onKey);
    };
  }, [historyOpen]);

  return (
    <>
      {/* Zone de déplacement de la fenêtre (transparente). */}
      <div className="absolute inset-x-0 top-0 z-30 h-11" data-tauri-drag-region />
      <div ref={zoneRef} className="titlebar-controls absolute top-2 right-2 z-40 flex items-center gap-0.5 rounded-xl p-0.5">
        <WinIcon label={m.titlebar.history} onClick={() => setHistoryOpen((v) => !v)} pressed={historyOpen}>
          <History className="size-[17px]" strokeWidth={1.6} aria-hidden />
          {history.length > 0 && (
            <span className="tabular absolute top-1.5 right-1.5 size-1.5 rounded-full bg-[var(--accent-soft)]" aria-hidden />
          )}
        </WinIcon>
        <WinIcon label={m.logs.title} onClick={() => setLogsOpen(!logsOpen)} pressed={logsOpen}>
          <ScrollText className="size-[17px]" strokeWidth={1.6} aria-hidden />
        </WinIcon>
        <WinIcon label={m.help.title} onClick={() => goTo("help")} pressed={section === "help"}>
          <CircleHelp className="size-[17px]" strokeWidth={1.6} aria-hidden />
        </WinIcon>
        <WinIcon label={m.settings.title} onClick={() => goTo("settings")} pressed={section === "settings"}>
          <Settings className="size-[17px]" strokeWidth={1.6} aria-hidden />
        </WinIcon>
        <div className="mx-1 h-4 w-px bg-white/[0.08]" aria-hidden />
        <WinIcon label={m.titlebar.minimize} onClick={() => void win.minimize()}>
          <Minus className="size-[17px]" strokeWidth={1.6} aria-hidden />
        </WinIcon>
        <WinIcon label={m.titlebar.maximize} onClick={() => void win.toggleMaximize()}>
          {maximized ? (
            <Copy className="size-[15px]" strokeWidth={1.6} aria-hidden />
          ) : (
            <Square className="size-[14px]" strokeWidth={1.6} aria-hidden />
          )}
        </WinIcon>
        <WinIcon label={m.titlebar.close} onClick={() => void win.close()} danger>
          <X className="size-[18px]" strokeWidth={1.6} aria-hidden />
        </WinIcon>

        {historyOpen && (
          <div className="glass-float absolute top-11 right-0 w-80 rounded-[14px] p-1.5 animate-scale-in origin-top-right">
            <div className="flex items-center justify-between px-2.5 pt-1.5 pb-2">
              <span className="text-[12.5px] font-medium text-zinc-200">{m.titlebar.history}</span>
              <button
                type="button"
                disabled={history.length === 0}
                onClick={() => void undo()}
                className="inline-flex h-7 items-center gap-1.5 rounded-[8px] px-2.5 text-[12px] text-zinc-300 transition-colors hover:bg-white/[0.06] hover:text-white disabled:opacity-35"
              >
                <Undo2 className="size-3.5" aria-hidden />
                {m.titlebar.undoLast}
              </button>
            </div>
            {history.length === 0 ? (
              <div className="px-2.5 pb-3 text-[12.5px] text-zinc-500">{m.titlebar.historyEmpty}</div>
            ) : (
              <ul className="max-h-72 overflow-y-auto">
                {history.slice(0, 8).map((h) => (
                  <li key={h.id} className="flex items-center gap-3 rounded-[8px] px-2.5 py-2 text-[12.5px]">
                    <span className="size-1.5 shrink-0 rounded-full bg-[var(--steel)]" aria-hidden />
                    <span className="min-w-0 flex-1 truncate text-zinc-200" title={h.label}>
                      {h.label}
                    </span>
                    <span className="tabular shrink-0 text-[11px] text-zinc-500">{ago(h.ts, lang)}</span>
                  </li>
                ))}
              </ul>
            )}
          </div>
        )}
      </div>
    </>
  );
}

/** Durée écoulée courte et localisée (« il y a 3 min »). */
function ago(ts: number, lang: string): string {
  const s = Math.max(0, (Date.now() - ts) / 1000);
  const rtf = new Intl.RelativeTimeFormat(lang, { numeric: "auto", style: "short" });
  if (s < 60) return rtf.format(-Math.round(s), "second");
  if (s < 3600) return rtf.format(-Math.round(s / 60), "minute");
  if (s < 86400) return rtf.format(-Math.round(s / 3600), "hour");
  return rtf.format(-Math.round(s / 86400), "day");
}

/** Bouton d'icône de fenêtre : même taille, même gris, survol discret. */
function WinIcon({
  children,
  label,
  onClick,
  pressed,
  danger,
}: {
  children: React.ReactNode;
  label: string;
  onClick: () => void;
  pressed?: boolean;
  danger?: boolean;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-label={label}
      aria-pressed={pressed}
      title={label}
      className={cx(
        "relative grid size-9 place-items-center rounded-[10px] transition-colors duration-150",
        pressed ? "bg-white/[0.07] text-zinc-100" : "text-zinc-400",
        danger ? "hover:bg-[#c42b1c] hover:text-white" : "hover:bg-white/[0.06] hover:text-zinc-100",
      )}
    >
      {children}
    </button>
  );
}
