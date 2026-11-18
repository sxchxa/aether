// Fenêtre de mise à jour au démarrage : un seul bouton télécharge, installe et relance Aether.
import { useEffect, useState } from "react";
import { Download, Sparkles } from "lucide-react";
import type { Update } from "@tauri-apps/plugin-updater";
import { fmt, useI18n } from "../i18n";
import { errorMessage } from "../lib/ipc";
import { useApp } from "../store/app";
import { Modal } from "./Overlays";
import { Button } from "./ui";

const SNOOZE_KEY = "aether.updateSnoozed";

/** Vérifie s'il existe une nouvelle version (silencieux : hors ligne, rien ne s'affiche). */
async function findUpdate(): Promise<Update | null> {
  try {
    const { check } = await import("@tauri-apps/plugin-updater");
    return await check();
  } catch {
    return null;
  }
}

export function UpdatePrompt() {
  const { m } = useI18n();
  const toast = useApp((s) => s.toast);
  const [update, setUpdate] = useState<Update | null>(null);
  const [progress, setProgress] = useState<number | null>(null);

  useEffect(() => {
    // Après le démarrage, pour ne pas ralentir l'affichage de l'accueil.
    const timer = window.setTimeout(() => {
      void findUpdate().then((u) => {
        if (!u) return;
        // « Plus tard » : la même version n'est plus proposée avant le prochain lancement du lendemain.
        const snoozed = window.localStorage.getItem(SNOOZE_KEY);
        if (snoozed === `${u.version}@${new Date().toDateString()}`) return;
        setUpdate(u);
      });
    }, 4000);
    return () => window.clearTimeout(timer);
  }, []);

  if (!update) return null;

  const install = async () => {
    let total = 0;
    let done = 0;
    setProgress(0);
    try {
      await update.downloadAndInstall((e) => {
        if (e.event === "Started") total = e.data.contentLength ?? 0;
        if (e.event === "Progress") {
          done += e.data.chunkLength;
          if (total > 0) setProgress(Math.min(99, Math.round((done / total) * 100)));
        }
        if (e.event === "Finished") setProgress(100);
      });
      // L'installateur ferme Aether, installe la nouvelle version puis la relance automatiquement.
    } catch (e) {
      setProgress(null);
      toast("error", errorMessage(e));
    }
  };

  const later = () => {
    try {
      window.localStorage.setItem(SNOOZE_KEY, `${update.version}@${new Date().toDateString()}`);
    } catch {
      /* rappel au prochain lancement */
    }
    setUpdate(null);
  };

  const busy = progress !== null;
  return (
    <Modal
      open
      title={fmt(m.updatePrompt.title, { version: update.version })}
      onClose={busy ? () => undefined : later}
      footer={
        <>
          {!busy && (
            <Button variant="ghost" onClick={later}>
              {m.updatePrompt.later}
            </Button>
          )}
          <Button variant="primary" icon={<Download className="size-4" />} loading={busy} onClick={() => void install()}>
            {busy ? fmt(m.updatePrompt.installing, { percent: progress ?? 0 }) : m.updatePrompt.install}
          </Button>
        </>
      }
    >
      <div className="flex gap-3">
        <Sparkles className="mt-0.5 size-5 shrink-0 text-[var(--accent-soft)]" aria-hidden />
        <div className="min-w-0 space-y-2">
          <p>{m.updatePrompt.body}</p>
          {update.body && <p className="max-h-40 overflow-y-auto text-[12.5px] whitespace-pre-line text-[var(--text-3)]">{update.body}</p>}
          {busy && (
            <div className="h-1.5 overflow-hidden rounded-full bg-[var(--track)]" aria-hidden>
              <div className="h-full rounded-full bg-[var(--accent)] transition-[width]" style={{ width: `${progress ?? 0}%` }} />
            </div>
          )}
        </div>
      </div>
    </Modal>
  );
}
