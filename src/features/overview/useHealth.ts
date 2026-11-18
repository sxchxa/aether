// Score de santé partagé (barre latérale + Vue d'ensemble).
import { useMemo } from "react";
import { useApp } from "../../store/app";
import { ramPercent, useMetrics } from "../../store/metrics";
import { computeHealth } from "./health";

export function useHealth(): number {
  const info = useApp((s) => s.systemInfo);
  const tweaks = useApp((s) => s.tweaks);
  const restore = useApp((s) => s.restore);
  // Sélection d'une valeur arrondie à 5 % : le composant ne se redessine que lorsqu'elle change,
  // et non à chaque image du flux de métriques.
  const ramBucket = useMetrics((s) => {
    const ram = ramPercent(s.frame);
    return ram === null ? null : Math.round(ram / 5) * 5;
  });
  return useMemo(
    () => computeHealth({ info, ramPercent: ramBucket, tweaks: Object.values(tweaks), restoreEnabled: restore?.enabled ?? null }),
    [info, ramBucket, tweaks, restore],
  );
}
