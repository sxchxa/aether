// Analyse intelligente : problèmes détectés en langage clair, chacun avec sa correction.
import { useCallback, useState, type ReactNode } from "react";
import { isPreference } from "../../lib/navigation";
import { HardDrive, Monitor, MemoryStick, Power, ShieldAlert, ShieldPlus, SlidersHorizontal, Trash2 } from "lucide-react";
import type { Tone } from "../../components/ui";
import { fmt, useI18n } from "../../i18n";
import { api, errorMessage } from "../../lib/ipc";
import { ensureRestorePoint } from "../../lib/safety";
import { formatBytes, formatPercent } from "../../lib/format";
import { useApp } from "../../store/app";
import { ramPercent, useMetrics } from "../../store/metrics";

export type FindingId = "protection" | "point" | "junk" | "tweaks" | "disk" | "startup" | "display" | "ram";

export interface Finding {
  id: FindingId;
  icon: ReactNode;
  tone: Tone;
  title: string;
  desc: string;
  fixLabel: string;
  /** Corrigé par « Tout corriger » (sinon : action guidée vers la bonne page). */
  auto: boolean;
  fix: () => Promise<void>;
}

const RECENT_POINT_DAYS = 30;

/** Calcule les problèmes à partir des données déjà chargées (aucune lecture système supplémentaire). */
export function useFindings(startupEnabled: number | null): Finding[] {
  const { m, lang } = useI18n();
  const scan = useApp((s) => s.scan);
  const rules = useApp((s) => s.rules);
  const tweaks = useApp((s) => s.tweaks);
  const restore = useApp((s) => s.restore);
  const info = useApp((s) => s.systemInfo);
  const goTo = useApp((s) => s.goTo);
  const setRestore = useApp((s) => s.setRestore);
  const runScan = useApp((s) => s.runScan);
  const loadTweaks = useApp((s) => s.loadTweaks);
  // Valeur arrondie : la page ne se redessine pas à chaque image du flux de métriques.
  const ram = useMetrics((s) => {
    const r = ramPercent(s.frame);
    return r === null ? null : Math.round(r / 5) * 5;
  });
  const out: Finding[] = [];

  if (restore && !restore.enabled && !restore.blockedByPolicy) {
    out.push({
      id: "protection",
      icon: <ShieldAlert className="size-[18px]" />,
      tone: "rose",
      title: m.scan.protectionTitle,
      desc: m.scan.protectionDesc,
      fixLabel: m.scan.protectionFix,
      auto: true,
      fix: async () => setRestore(await api.restoreEnable()),
    });
  } else if (restore?.enabled) {
    const last = restore.lastPoint ? new Date(restore.lastPoint.created).getTime() : 0;
    if (!last || Date.now() - last > RECENT_POINT_DAYS * 86_400_000) {
      out.push({
        id: "point",
        icon: <ShieldPlus className="size-[18px]" />,
        tone: "amber",
        title: m.scan.pointTitle,
        desc: m.scan.pointDesc,
        fixLabel: m.scan.pointFix,
        auto: true,
        fix: async () => setRestore(await api.restoreCreate("Aether")),
      });
    }
  }

  // Fichiers inutiles : uniquement les règles sûres sélectionnées par défaut, sans application à fermer.
  const safeRules = (rules ?? []).filter((r) => r.risk === "safe" && r.defaultSelected && r.processes.length === 0 && r.roots.length > 0);
  const junk = safeRules.reduce((sum, r) => sum + (scan?.[r.id]?.bytes ?? 0), 0);
  if (junk > 20 * 1024 * 1024) {
    out.push({
      id: "junk",
      icon: <Trash2 className="size-[18px]" />,
      tone: "violet",
      title: fmt(m.scan.junkTitle, { size: formatBytes(junk, lang) }),
      desc: m.scan.junkDesc,
      fixLabel: m.scan.junkFix,
      auto: true,
      fix: async () => {
        await api.storageClean(safeRules.map((r) => r.id));
        await runScan(true);
      },
    });
  }

  // Les effets visuels sont une affaire de goût : jamais appliqués automatiquement.
  const safeTweaks = Object.values(tweaks).filter((t) => t.risk === "safe" && !isPreference(t) && t.probe.available && !t.probe.applied);
  if (safeTweaks.length > 0) {
    const titles = safeTweaks
      .slice(0, 3)
      .map((t) => (m.tweaks as Record<string, { title: string } | undefined>)[t.id]?.title ?? t.id)
      .join(" · ");
    out.push({
      id: "tweaks",
      icon: <SlidersHorizontal className="size-[18px]" />,
      tone: "sky",
      title: fmt(m.scan.tweaksTitle, { count: safeTweaks.length }),
      desc: safeTweaks.length > 3 ? `${titles}…` : titles,
      fixLabel: m.scan.tweaksFix,
      auto: true,
      fix: async () => {
        await api.tweaksOptimizeMany(safeTweaks.map((t) => t.id));
        await loadTweaks();
      },
    });
  }

  const disk = info?.disks.find((d) => d.mount.toUpperCase().startsWith("C:"));
  if (disk && disk.total > 0 && disk.available / disk.total < 0.15) {
    out.push({
      id: "disk",
      icon: <HardDrive className="size-[18px]" />,
      tone: "amber",
      title: fmt(m.scan.diskTitle, { free: formatBytes(disk.available, lang) }),
      desc: m.scan.diskDesc,
      fixLabel: m.scan.diskFix,
      auto: false,
      fix: async () => goTo("storage", "st-analyzer"),
    });
  }

  if (startupEnabled !== null && startupEnabled > 4) {
    out.push({
      id: "startup",
      icon: <Power className="size-[18px]" />,
      tone: "indigo",
      title: fmt(m.scan.startupTitle, { count: startupEnabled }),
      desc: m.scan.startupDesc,
      fixLabel: m.scan.startupFix,
      auto: false,
      fix: async () => goTo("system", "sy-startup"),
    });
  }

  if (info?.display?.upgradeAvailable) {
    out.push({
      id: "display",
      icon: <Monitor className="size-[18px]" />,
      tone: "emerald",
      title: fmt(m.scan.displayTitle, { max: info.display.maxHz }),
      desc: fmt(m.scan.displayDesc, { current: info.display.currentHz }),
      fixLabel: m.scan.displayFix,
      auto: false,
      fix: () => api.openSettingsPage("ms-settings:display-advanced"),
    });
  }

  if (ram !== null && ram > 85) {
    out.push({
      id: "ram",
      icon: <MemoryStick className="size-[18px]" />,
      tone: "sky",
      title: fmt(m.scan.ramTitle, { percent: formatPercent(ram, lang) }),
      desc: m.scan.ramDesc,
      fixLabel: m.scan.ramFix,
      auto: true,
      fix: async () => {
        await api.ramPurge();
      },
    });
  }

  return out;
}

/** Corrige tous les problèmes corrigeables automatiquement, avec un point de restauration préalable. */
export function useFixAll(findings: Finding[]) {
  const { m } = useI18n();
  const toast = useApp((s) => s.toast);
  const refreshHistory = useApp((s) => s.refreshHistory);
  const dryRun = useApp((s) => s.dryRun);
  const [busy, setBusy] = useState(false);

  const fixAll = useCallback(async () => {
    const auto = findings.filter((f) => f.auto);
    if (auto.length === 0) return;
    setBusy(true);
    let done = 0;
    try {
      // Filet de sécurité : point de restauration avant toute modification groupée.
      const hasPointFinding = auto.some((f) => f.id === "point" || f.id === "protection");
      if (!dryRun && !hasPointFinding) await ensureRestorePoint();
      for (const f of auto) {
        try {
          await f.fix();
          done += 1;
        } catch (e) {
          toast("error", `${f.title} : ${errorMessage(e)}`);
        }
      }
      await refreshHistory();
      toast(dryRun ? "warn" : "success", dryRun ? m.dryRun.simulated : fmt(m.scan.fixAllDone, { count: done }));
    } finally {
      setBusy(false);
    }
  }, [findings, dryRun, refreshHistory, toast, m]);

  return { fixAll, busy };
}
