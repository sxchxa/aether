// Préchargement au lancement (pendant l'écran de chargement) : toutes les lectures système sont
// lancées en parallèle, puis l'écran disparaît dès que l'essentiel est prêt. La barre de
// l'écran de chargement avance réellement à chaque étape terminée.
import { preloadGames } from "../features/games/GamesPage";
import { useApp } from "../store/app";
import { fmt, isLanguage, messagesFor } from "../i18n";
import { formatBytes } from "./format";
import { api } from "./ipc";
import { prepareRestorePoint } from "./safety";
import { markStartupDone } from "./startup";

const MIN_SPLASH_MS = 700;
const MAX_SPLASH_MS = 3000;

const wait = (ms: number) => new Promise<void>((resolve) => window.setTimeout(resolve, ms));

/** Positionne la barre de progression de l'écran de chargement (0 à 1). */
export function setSplashProgress(value: number): void {
  const bar = document.getElementById("splash-progress");
  if (!bar) return;
  // La barre n'avance jamais à reculons (les étapes se terminent dans le désordre).
  const current = Number(bar.dataset.p ?? "0");
  const next = Math.max(current, Math.min(1, value));
  bar.dataset.p = String(next);
  bar.style.transform = `scaleX(${next})`;
}

/** Annonce une seule fois le résultat du dernier entretien automatique. */
async function notifyMaintenance(): Promise<void> {
  try {
    const last = await api.maintenanceLast();
    if (!last) return;
    const seen = Number(window.localStorage.getItem("aether.maintenanceSeen") ?? "0");
    if (last.ts <= seen) return;
    window.localStorage.setItem("aether.maintenanceSeen", String(last.ts));
    const lang = document.documentElement.lang || "fr-FR";
    const m = messagesFor(isLanguage(lang) ? lang : "fr-FR");
    useApp.getState().toast("success", fmt(m.storage.maintenanceToast, { size: formatBytes(last.freed, lang), files: last.removed }));
  } catch {
    /* information facultative */
  }
}

export async function preloadAll(): Promise<void> {
  const s = useApp.getState();
  setSplashProgress(0.12);
  // Indispensables à l'affichage de l'accueil : chacune fait avancer la barre en se terminant.
  // Le reste (protection système, matériel, logiciels, jeux) se charge après l'affichage.
  const steps = [s.loadSettings(), s.refreshHistory(), s.loadTweaks(), s.loadRules(), s.runScan()];
  let done = 0;
  const essential = Promise.allSettled(
    steps.map((p) =>
      p.finally(() => {
        done += 1;
        setSplashProgress(0.12 + 0.78 * (done / steps.length));
      }),
    ),
  );
  // Lectures lourdes (PowerShell, winget) lancées une fois l'essentiel prêt : elles ne
  // ralentissent plus l'affichage de l'accueil sur les PC à peu de cœurs.
  void s.loadSystemInfo();
  void essential.then(() => {
    markStartupDone();
    void s.loadRestore().then(prepareRestorePoint);
    void preloadGames();
    window.setTimeout(() => void s.loadLibrary(), 1500);
  });
  // L’analyse est « faite » dès que les options et le nettoyage sont lus (sans attendre la protection système).
  void Promise.allSettled([steps[2], steps[4]]).then(() => useApp.getState().markAnalyzed());
  void notifyMaintenance();
  await Promise.race([Promise.all([essential, wait(MIN_SPLASH_MS)]), wait(MAX_SPLASH_MS)]);
  markStartupDone();
}

/** Termine la barre puis fait disparaître l'écran de chargement en fondu. */
export function hideSplash(): void {
  const el = document.getElementById("splash");
  if (!el) return;
  setSplashProgress(1);
  // Temps de démarrage réel (depuis le chargement de la page), affiché dans Réglages.
  useApp.setState({ startupMs: Math.round(performance.now()) });
  // Laisse la barre atteindre la fin avant le fondu de sortie.
  window.setTimeout(() => {
    el.classList.add("hide");
    window.setTimeout(() => el.remove(), 520);
  }, 280);
}
