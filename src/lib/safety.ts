// Filet de sécurité : point de restauration automatique avant les optimisations groupées
// (« Tout optimiser », profils, performance maximale). Option activée par défaut.
import { api } from "./ipc";
import { useApp } from "../store/app";

const KEY = "aether.autoRestorePoint";

export function autoRestoreEnabled(): boolean {
  try {
    return window.localStorage.getItem(KEY) !== "0";
  } catch {
    return true;
  }
}

export function setAutoRestoreEnabled(on: boolean): void {
  try {
    window.localStorage.setItem(KEY, on ? "1" : "0");
  } catch {
    /* préférence non mémorisée */
  }
}

const DAY_MS = 24 * 60 * 60 * 1000;
let pending: Promise<void> | null = null;

/** Un point de moins de 24 h existe déjà : inutile d'en recréer un (Windows prend 10 à 60 s). */
function hasRecentPoint(): boolean {
  const created = useApp.getState().restore?.lastPoint?.created;
  return !!created && Date.now() - Date.parse(created) < DAY_MS;
}

function createPoint(description: string): Promise<void> {
  pending ??= api
    .restoreCreate(description)
    .then((status) => useApp.getState().setRestore(status))
    .catch(() => {
      /* point non créé : l'historique d'annulation d'Aether reste disponible */
    })
    .finally(() => {
      pending = null;
    });
  return pending;
}

/**
 * Prépare le point du jour en arrière-plan, dès le lancement : au moment d'une optimisation
 * groupée, il existe déjà et le bouton répond immédiatement.
 */
export function prepareRestorePoint(): void {
  const restore = useApp.getState().restore;
  if (!autoRestoreEnabled() || !restore?.enabled || hasRecentPoint()) return;
  void createPoint("Aether : point automatique");
}

/** `ensureRestorePoint` créera un nouveau point (affiché à l'avance dans le détail des actions). */
export function willCreateRestorePoint(): boolean {
  const restore = useApp.getState().restore;
  return autoRestoreEnabled() && !(restore && !restore.enabled) && !hasRecentPoint();
}

/**
 * Garantit un point de restauration récent avant une modification groupée. Instantané lorsqu'un
 * point de moins de 24 h existe ; ne bloque jamais l'optimisation en cas d'échec.
 */
export async function ensureRestorePoint(): Promise<void> {
  if (!autoRestoreEnabled()) return;
  const restore = useApp.getState().restore;
  if (restore && !restore.enabled) return;
  if (pending) return pending;
  if (hasRecentPoint()) return;
  await createPoint("Aether : avant optimisation");
}
