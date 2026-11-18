// État global de l'interface (navigation, mode Expert, Mode Aperçu, options, journal, toasts).
import { create } from "zustand";
import { api, errorMessage } from "../lib/ipc";
import { startupDone } from "../lib/startup";
import { ensureRestorePoint } from "../lib/safety";
import { isLevel } from "../lib/level";
import type {
  CleanRule,
  GuidedProfile,
  HistoryEntry,
  LibraryApp,
  LogEntry,
  Outcome,
  RestoreStatus,
  ScanResult,
  SystemInfo,
  TweakStatus,
  UserLevel,
} from "../lib/types";

export type SectionId = "overview" | "games" | "apps" | "storage" | "performance" | "power" | "network" | "system" | "settings" | "help";

export type InspectorTarget = { kind: "tweak"; id: string } | { kind: "action"; id: ActionId; technical: string };

export type ActionId =
  | "ram"
  | "trim"
  | "flushDns"
  | "winsock"
  | "dns"
  | "sfc"
  | "dism"
  | "appx"
  | "onedrive"
  | "edgeUninstall"
  | "startup"
  | "analyzer";

export type ToastKind = "success" | "info" | "warn" | "error";

export interface Toast {
  id: number;
  kind: ToastKind;
  text: string;
}

function readFlag(key: string, fallback: boolean): boolean {
  try {
    const v = window.localStorage.getItem(key);
    return v === null ? fallback : v === "1";
  } catch {
    return fallback;
  }
}

function writeValue(key: string, value: string): void {
  try {
    window.localStorage.setItem(key, value);
  } catch {
    /* préférence non mémorisée */
  }
}

/** Niveau mémorisé ; l'ancien interrupteur « mode avancé » devient le niveau Expérimenté. */
function readLevel(): UserLevel {
  try {
    const v = window.localStorage.getItem("aether.level");
    if (isLevel(v)) return v;
    return window.localStorage.getItem("aether.expert") === "1" ? "expert" : "beginner";
  } catch {
    return "beginner";
  }
}

function readProfile(): GuidedProfile {
  try {
    const v = window.localStorage.getItem("aether.profile");
    const known = ["gamer", "laptop", "client", "silent", "competitive", "streaming", "office", "creator"];
    return v && known.includes(v) ? (v as GuidedProfile) : "daily";
  } catch {
    return "daily";
  }
}

interface AppState {
  section: SectionId;
  expert: boolean;
  level: UserLevel;
  dryRun: boolean;
  onboarded: boolean;
  profile: GuidedProfile;
  paletteOpen: boolean;
  logsOpen: boolean;
  inspector: InspectorTarget | null;
  logs: LogEntry[];
  toasts: Toast[];
  tweaks: Record<string, TweakStatus>;
  tweaksLoading: boolean;
  touched: Record<string, true>;
  busy: Record<string, true>;
  history: HistoryEntry[];
  systemInfo: SystemInfo | null;
  hardwareLoaded: boolean;
  restore: RestoreStatus | null;
  rules: CleanRule[] | null;
  scan: Record<string, ScanResult> | null;
  scanning: boolean;
  /** Moment de la dernière analyse complète (ms). */
  lastAnalysis: number | null;
  /** Temps entre le lancement et l’affichage de l’interface (ms). */
  startupMs: number | null;
  markAnalyzed: () => void;
  library: LibraryApp[] | null;
  libraryError: string | null;
  loadLibrary: (force?: boolean) => Promise<void>;
  markInstalled: (id: string) => void;

  setSection: (s: SectionId) => void;
  goTo: (s: SectionId, anchor?: string) => void;
  loadSystemInfo: (force?: boolean) => Promise<void>;
  loadRestore: (force?: boolean) => Promise<void>;
  setRestore: (r: RestoreStatus) => void;
  loadRules: () => Promise<void>;
  runScan: (force?: boolean) => Promise<void>;
  ensureTweaks: (ids: string[]) => Promise<void>;
  setExpert: (v: boolean) => void;
  setLevel: (level: UserLevel) => void;
  setOnboarded: () => void;
  replayOnboarding: () => void;
  setProfile: (p: GuidedProfile) => void;
  setPaletteOpen: (v: boolean) => void;
  setLogsOpen: (v: boolean) => void;
  inspect: (t: InspectorTarget | null) => void;
  pushLog: (e: LogEntry) => void;
  setLogs: (e: LogEntry[]) => void;
  toast: (kind: ToastKind, text: string) => void;
  dismissToast: (id: number) => void;
  loadSettings: () => Promise<void>;
  setDryRun: (v: boolean) => Promise<void>;
  loadTweaks: (ids?: string[]) => Promise<void>;
  setTweak: (id: string, optimized: boolean) => Promise<Outcome | null>;
  refreshHistory: () => Promise<void>;
  setBusy: (key: string, on: boolean) => void;
}

let toastSeq = 0;
let fullLoad: Promise<void> | null = null;

export const useApp = create<AppState>((set, get) => ({
  section: "overview",
  level: readLevel(),
  expert: readLevel() === "expert",
  dryRun: false,
  onboarded: readFlag("aether.onboarded", false),
  profile: readProfile(),
  paletteOpen: false,
  logsOpen: false,
  inspector: null,
  logs: [],
  toasts: [],
  tweaks: {},
  tweaksLoading: false,
  touched: {},
  busy: {},
  history: [],
  systemInfo: null,
  hardwareLoaded: false,
  restore: null,
  rules: null,
  scan: null,
  scanning: false,
  lastAnalysis: null,
  startupMs: null,
  markAnalyzed: () => set({ lastAnalysis: Date.now() }),
  library: null,
  libraryError: null,
  loadLibrary: async (force) => {
    if (get().library && !force) return;
    try {
      set({ library: await api.appsLibrary(), libraryError: null });
    } catch (e) {
      set({ library: [], libraryError: errorMessage(e) });
    }
  },
  markInstalled: (id) => set((s) => ({ library: (s.library ?? []).map((a) => (a.id === id ? { ...a, installed: true } : a)) })),

  setSection: (section) => set({ section }),
  goTo: (section, anchor) => {
    set({ section });
    const root = () => document.getElementById("aether-scroll");
    // Sans ancre, l'application restaure la position de défilement mémorisée de la page.
    if (!anchor) return;
    // Défilement explicite de la zone de contenu ; nouvelle tentative tant que la page n'est
    // pas encore montée (changement de section) — jusqu'à ~1 s.
    let attempts = 0;
    const tryScroll = () => {
      const container = root();
      const target = document.getElementById(anchor);
      if (container && target) {
        const offset = target.getBoundingClientRect().top - container.getBoundingClientRect().top + container.scrollTop - 20;
        container.scrollTo({ top: Math.max(0, offset), behavior: "smooth" });
        return;
      }
      if (++attempts < 60) requestAnimationFrame(tryScroll);
    };
    requestAnimationFrame(tryScroll);
  },
  loadSystemInfo: async (force) => {
    if (get().systemInfo && !force) return;
    try {
      set({ systemInfo: await api.systemInfo() });
      // Partie lente (WMI) chargée une fois l'accueil affiché, sans ralentir le démarrage.
      await startupDone;
      const hw = await api.hardwareInfo();
      set((s) => (s.systemInfo ? { systemInfo: { ...s.systemInfo, physicalDisks: hw.disks, battery: hw.battery } } : {}));
      set({ hardwareLoaded: true });
    } catch (e) {
      get().toast("error", errorMessage(e));
    }
  },
  loadRestore: async (force) => {
    if (get().restore && !force) return;
    try {
      set({ restore: await api.restoreStatus() });
    } catch {
      /* statut optionnel */
    }
  },
  setRestore: (restore) => set({ restore }),
  loadRules: async () => {
    if (get().rules) return;
    try {
      set({ rules: await api.storageRules() });
    } catch (e) {
      get().toast("error", errorMessage(e));
    }
  },
  runScan: async (force) => {
    if (get().scanning || (get().scan && !force)) return;
    set({ scanning: true });
    try {
      const results = await api.storageScan();
      set({ scan: Object.fromEntries(results.map((r) => [r.id, r])) });
    } catch (e) {
      get().toast("error", errorMessage(e));
    } finally {
      set({ scanning: false });
    }
  },
  ensureTweaks: async (ids) => {
    // Attend le chargement global en cours plutôt que de relancer les mêmes lectures système.
    if (fullLoad) await fullLoad;
    const missing = ids.filter((id) => !get().tweaks[id]);
    if (missing.length) await get().loadTweaks(missing);
  },
  setExpert: (expert) => get().setLevel(expert ? "expert" : "gamer"),
  setLevel: (level) => {
    writeValue("aether.level", level);
    writeValue("aether.expert", level === "expert" ? "1" : "0");
    set({ level, expert: level === "expert" });
  },
  setOnboarded: () => {
    writeValue("aether.onboarded", "1");
    set({ onboarded: true });
  },
  replayOnboarding: () => {
    writeValue("aether.onboarded", "0");
    set({ onboarded: false });
  },
  setProfile: (profile) => {
    writeValue("aether.profile", profile);
    set({ profile });
  },
  setPaletteOpen: (paletteOpen) => set({ paletteOpen }),
  setLogsOpen: (logsOpen) => set({ logsOpen }),
  inspect: (inspector) => set({ inspector }),
  pushLog: (entry) => set((s) => ({ logs: [...s.logs.slice(-799), entry] })),
  setLogs: (logs) => set({ logs }),
  toast: (kind, text) => {
    const id = ++toastSeq;
    set((s) => ({ toasts: [...s.toasts.slice(-3), { id, kind, text }] }));
    window.setTimeout(() => get().dismissToast(id), kind === "error" ? 7000 : 4200);
  },
  dismissToast: (id) => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),

  loadSettings: async () => {
    try {
      const s = await api.settingsGet();
      set({ dryRun: s.dryRun });
    } catch (e) {
      get().toast("error", errorMessage(e));
    }
  },
  setDryRun: async (value) => {
    try {
      const s = await api.setDryRun(value);
      set({ dryRun: s.dryRun });
    } catch (e) {
      get().toast("error", errorMessage(e));
    }
  },

  loadTweaks: async (ids) => {
    set({ tweaksLoading: true });
    try {
      const request = api.tweaksStatus(ids);
      if (!ids)
        fullLoad = request.then(
          () => undefined,
          () => undefined,
        );
      const list = await request;
      set((s) => {
        const next = { ...s.tweaks };
        for (const t of list) next[t.id] = t;
        return { tweaks: next };
      });
    } catch (e) {
      get().toast("error", errorMessage(e));
    } finally {
      if (!ids) fullLoad = null;
      set({ tweaksLoading: false });
    }
  },

  setTweak: async (id, optimized) => {
    get().setBusy(id, true);
    try {
      if (optimized) await ensureRestorePoint();
      const outcome = optimized ? await api.tweakOptimize(id) : await api.tweakRestore(id);
      if (outcome === "applied") set((s) => ({ touched: { ...s.touched, [id]: true } }));
      await get().loadTweaks([id]);
      await get().refreshHistory();
      return outcome;
    } catch (e) {
      get().toast("error", errorMessage(e));
      return null;
    } finally {
      get().setBusy(id, false);
    }
  },

  refreshHistory: async () => {
    try {
      set({ history: await api.historyList() });
    } catch {
      /* l'historique est secondaire : on ignore une erreur de lecture */
    }
  },

  setBusy: (key, on) =>
    set((s) => {
      const busy = { ...s.busy };
      if (on) busy[key] = true;
      else delete busy[key];
      return { busy };
    }),
}));
