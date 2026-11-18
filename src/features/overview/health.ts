// Calcul du score de santé (0–100) à partir de mesures réelles.
import { isPreference } from "../../lib/navigation";
import type { SystemInfo, TweakStatus } from "../../lib/types";

export interface HealthInput {
  info: SystemInfo | null;
  ramPercent: number | null;
  tweaks: TweakStatus[];
  restoreEnabled: boolean | null;
}

/** Score pondéré : disque système 35, RAM 20, optimisations 30, protection 15. */
export function computeHealth({ info, ramPercent, tweaks, restoreEnabled }: HealthInput): number {
  let score = 0;
  const system = info?.disks.find((d) => d.mount.toUpperCase().startsWith("C:"));
  if (system && system.total > 0) {
    const free = system.available / system.total;
    score += 35 * Math.min(1, free / 0.25);
  } else {
    score += 25;
  }
  score += ramPercent === null ? 14 : 20 * Math.min(1, Math.max(0, (100 - ramPercent) / 50));
  // Les effets visuels et les préférences personnelles (jamais appliqués automatiquement) ne comptent pas.
  const relevant = tweaks.filter((t) => t.probe.available && t.risk !== "advanced" && !isPreference(t));
  score += relevant.length === 0 ? 20 : 30 * (relevant.filter((t) => t.probe.applied).length / relevant.length);
  score += restoreEnabled === null ? 10 : restoreEnabled ? 15 : 0;
  return Math.round(Math.min(100, Math.max(0, score)));
}
