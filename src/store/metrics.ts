// Métriques temps réel : un seul flux (tauri::ipc::Channel) pour toute l'application.
// Les valeurs affichées sont des moyennes sur 10 secondes : lecture calme et représentative de
// la charge réelle (pas de chiffres qui sautent à chaque seconde), et quasiment aucun re-rendu.
import { create } from "zustand";
import { api } from "../lib/ipc";
import type { MetricsFrame } from "../lib/types";

/** Période d'actualisation de l'affichage. */
export const METRICS_PERIOD_MS = 10_000;
/** Images par seconde demandées au backend (moyennées sur chaque période). */
const STREAM_FPS = 1;
/** Historique court (mini-graphiques) : 10 minutes. */
const HISTORY = 60;
/** Historique long (graphique « Performances système ») : 1 heure. */
const LONG_HISTORY = 360;

interface MetricsState {
  frame: MetricsFrame | null;
  cpu: number[];
  ram: number[];
  longCpu: number[];
  longRam: number[];
  start: () => void;
  stop: () => void;
}

let pending: MetricsFrame[] = [];
let timer: number | undefined;
let running = false;

/** Moyenne des images reçues pendant la période (la dernière sert pour les totaux). */
function average(frames: MetricsFrame[]): MetricsFrame | null {
  const last = frames.at(-1);
  if (!last) return null;
  const mean = (pick: (f: MetricsFrame) => number) => frames.reduce((sum, f) => sum + pick(f), 0) / frames.length;
  return {
    ...last,
    cpu: mean((f) => f.cpu),
    ramUsed: mean((f) => f.ramUsed),
    cores: last.cores.map((_, i) => mean((f) => f.cores[i] ?? 0)),
  };
}

const ramPct = (f: MetricsFrame) => (f.ramUsed / Math.max(1, f.ramTotal)) * 100;

export const useMetrics = create<MetricsState>((set, get) => {
  const push = (f: MetricsFrame) =>
    set((s) => ({
      frame: f,
      cpu: [...s.cpu.slice(-(HISTORY - 1)), f.cpu],
      ram: [...s.ram.slice(-(HISTORY - 1)), ramPct(f)],
      longCpu: [...s.longCpu.slice(-(LONG_HISTORY - 1)), f.cpu],
      longRam: [...s.longRam.slice(-(LONG_HISTORY - 1)), ramPct(f)],
    }));

  return {
    frame: null,
    cpu: [],
    ram: [],
    longCpu: [],
    longRam: [],
    start: () => {
      if (running) return;
      running = true;
      pending = [];
      void api.monitorStart((f) => {
        // Première valeur affichée immédiatement, puis une moyenne toutes les 10 secondes.
        if (!get().frame) push(f);
        else pending.push(f);
      }, STREAM_FPS);
      timer = window.setInterval(() => {
        const f = average(pending);
        pending = [];
        if (f) push(f);
      }, METRICS_PERIOD_MS);
    },
    stop: () => {
      // Pas d'appel `monitor_stop` ici : il pourrait arriver après un redémarrage immédiat
      // (double montage React) et couper le nouveau flux. Le backend remplace l'ancien flux
      // à chaque `monitor_start` et s'arrête seul quand le canal est fermé.
      running = false;
      if (timer !== undefined) window.clearInterval(timer);
      timer = undefined;
    },
  };
});

export function ramPercent(frame: MetricsFrame | null): number | null {
  return frame ? ramPct(frame) : null;
}
